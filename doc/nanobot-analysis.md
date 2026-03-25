# nanobot 架构与技术分析

> 参考仓库：`/Users/hhl/Documents/projects/nanobot`
> 目标：为 atombot（Rust 重写版）提供设计参考

---

## 一、整体架构

```
渠道层 (Channels)
  Telegram / WhatsApp / CLI
       ↓ publish_inbound
  消息总线 (MessageBus)
       ↓ consume_inbound
  Agent Loop（核心引擎）
    ├── ContextBuilder（构建 prompt）
    ├── LLMProvider（调 LLM）
    ├── ToolRegistry（注册 & 执行 tool）
    └── SessionManager（持久化对话历史）
       ↓ publish_outbound
  消息总线 (MessageBus)
       ↓ dispatch_outbound
  渠道层（发送回复）
```

---

## 二、核心模块详解

### 2.1 MessageBus (`bus/`)

**职责**：解耦渠道和 Agent，所有消息通过 bus 中转。

| 方向 | 队列类型 | Python 实现 | 说明 |
|------|----------|-------------|------|
| 入站 | `asyncio.Queue` | `inbound` | 渠道 → Agent，单消费者 |
| 出站 | `asyncio.Queue` + 订阅回调 | `outbound` | Agent → 渠道，支持多订阅 |

**关键设计**：
- 出站支持按 channel 名称注册回调（`subscribe_outbound`）
- `dispatch_outbound` 作为后台 Task 持续分发出站消息
- `asyncio.wait_for(timeout=1.0)` 避免死锁，支持 graceful stop

**Rust 对应**：atombot 已用 `mpsc`（入站）+ `broadcast`（出站）实现，思路一致。broadcast 注意：慢消费者会丢消息，nanobot 用回调模式不会丢。

---

### 2.2 LLMProvider (`providers/`)

**职责**：统一的 LLM 调用接口，屏蔽底层 provider 差异。

```python
class LLMProvider(ABC):
    async def chat(messages, tools, model, ...) -> LLMResponse
    def get_default_model() -> str
```

**LLMResponse 关键字段**：
```python
content: str | None                    # 文字回复
tool_calls: list[ToolCallRequest]      # 工具调用列表
has_tool_calls: bool                   # 是否需要执行 tool
```

**实现**：`LiteLLMProvider` 用 litellm 库统一适配 Anthropic / OpenAI / OpenRouter。

**Rust 对应**：
- `Provider` trait = `LLMProvider` ABC
- 用 `reqwest` + `serde_json` 手写 HTTP 请求
- `tool_calls` 里的 `arguments` 是 JSON string，需要二次反序列化

---

### 2.3 Tool 系统 (`agent/tools/`)

**Tool 抽象**：
```python
class Tool(ABC):
    name: str           # tool 名称（LLM 调用时用）
    description: str    # 描述（传给 LLM）
    parameters: dict    # JSON Schema（传给 LLM）
    async execute(**kwargs) -> str   # 返回字符串结果
    to_schema() -> dict              # 转成 OpenAI function schema
```

**ToolRegistry**：
- `HashMap<name, Tool>`
- `get_definitions()` → 返回所有 tool 的 schema 列表，直接传给 LLM
- `execute(name, params)` → 统一执行入口，错误时返回错误字符串，不 panic

**内置 Tool**：

| Tool | 功能 |
|------|------|
| `ReadFileTool` | 读文件内容 |
| `WriteFileTool` | 写文件 |
| `EditFileTool` | 精确替换文件片段 |
| `ListDirTool` | 列目录 |
| `ExecTool` | 执行 shell 命令 |
| `WebSearchTool` | Brave 搜索 |
| `WebFetchTool` | 抓取网页内容 |
| `MessageTool` | 向渠道发消息（跨渠道通知） |

---

### 2.4 Agent Loop (`agent/loop.py`) ⭐ 核心

**ReAct 循环**：
```
while iteration < max_iterations:
    1. 调用 LLM（带 tools schema + 历史消息）
    2. 如果有 tool_calls：
       - 把 assistant 消息（含 tool_calls）追加到 messages
       - 逐个执行 tool，把结果以 role=tool 追加到 messages
       - continue 下一轮
    3. 如果没有 tool_calls：
       - final_content = response.content
       - break
```

**关键细节**：
- tool_calls 必须作为 assistant 消息的一部分追加，不能单独发
- tool 结果用 `role: "tool"` + `tool_call_id` 对应，顺序不能乱
- 最大迭代 20 轮防死循环
- 执行完保存到 Session，再 publish_outbound

---

### 2.5 ContextBuilder (`agent/context.py`)

**组装顺序**：
```
system prompt = [身份] + [AGENTS.md/SOUL.md/USER.md 等] + [Memory] + [Skills]
messages      = [system] + [history] + [current user message]
```

**Skills 分层加载**：
- Always-loaded：每次注入完整内容
- Available：只注入摘要，agent 用 `read_file` 按需加载（节省 token）

---

### 2.6 SessionManager (`session/manager.py`)

**会话持久化**：
- Session key = `{channel}:{chat_id}`（如 `telegram:12345`）
- 存储格式：JSONL（每行一条消息），第一行是 metadata
- 内存 `_cache` 避免频繁磁盘 IO
- `get_history(max_messages=50)` 控制 context 窗口长度

---

### 2.7 MemoryStore (`agent/memory.py`)

**两层记忆**：
- **短期**：`memory/YYYY-MM-DD.md`，每天的原始记录
- **长期**：`memory/MEMORY.md`，精炼的长期记忆

`get_memory_context()` 合并返回，注入 system prompt。

---

### 2.8 Channel 层 (`channels/`)

```python
class BaseChannel(ABC):
    async def start()        # 监听消息（长期运行）
    async def stop()         # 清理资源
    async def send(msg)      # 发送消息
```

- `_handle_message()` 做 `allow_from` 白名单校验，通过后 `publish_inbound`
- Telegram / WhatsApp 各自实现 `start` 监听平台 API

---

## 三、数据流全景

```
用户发消息
    ↓
Channel.start() 接收
    ↓
BaseChannel._handle_message() → 权限校验
    ↓
bus.publish_inbound(InboundMessage)
    ↓
AgentLoop.run() 消费
    ↓
SessionManager.get_or_create() → 取历史
ContextBuilder.build_messages() → 拼 prompt
    ↓
LLMProvider.chat(messages, tools)
    ↓
[有 tool_call] → ToolRegistry.execute() → 追加结果 → 再问 LLM
[无 tool_call] → 得到 final_content
    ↓
SessionManager.save() → 存历史
bus.publish_outbound(OutboundMessage)
    ↓
MessageBus.dispatch_outbound() → 找订阅的 channel
    ↓
Channel.send() → 发给用户
```

---

## 四、atombot 对照表

| nanobot (Python) | atombot (Rust) | 状态 |
|-----------------|---------------|------|
| `InboundMessage` dataclass | `InboundMessage` struct | ✅ 已完成 |
| `OutboundMessage` dataclass | `OutboundMessage` struct | ✅ 已完成 |
| `MessageBus` (asyncio.Queue) | `MessageBus` (mpsc + broadcast) | ✅ 已完成 |
| `LLMProvider` ABC | `Provider` trait | ⬜ 阶段 1.2 |
| `LLMResponse` | `LLMResponse` struct | ⬜ 阶段 1.1 |
| `Tool` ABC | `Tool` trait | ⬜ 阶段 2.1 |
| `ToolRegistry` | `ToolRegistry` | ⬜ 阶段 2.2 |
| `AgentLoop` (ReAct) | `AgentLoop` | ⬜ 阶段 3 |
| `ContextBuilder` | `ContextBuilder` | ⬜ 阶段 3 |
| `SessionManager` (JSONL) | `SessionManager` | ⬜ 阶段 4 |
| `MemoryStore` | `MemoryStore` | ⬜ 阶段 4+ |
| `BaseChannel` | Channel trait | ⬜ 阶段 4 |

---

## 五、重点技术笔记

### ReAct 消息格式（必须严格遵守）

tool_call 执行后，messages 追加顺序：

```json
// Step 1: assistant 消息（含 tool_calls，content 可为 null）
{
  "role": "assistant",
  "content": null,
  "tool_calls": [{"id": "call_abc", "type": "function", "function": {"name": "shell", "arguments": "{\"cmd\":\"ls\"}"}}]
}

// Step 2: tool 结果消息（role 必须是 "tool"）
{
  "role": "tool",
  "tool_call_id": "call_abc",
  "name": "shell",
  "content": "file1.txt\nfile2.txt"
}
```

`arguments` 是 JSON string，不是 object，序列化时要 `json.dumps()`。

### asyncio vs tokio 对照

| asyncio | tokio |
|---------|-------|
| `asyncio.Queue` | `mpsc::channel` |
| `asyncio.wait_for(coro, timeout)` | `tokio::time::timeout(dur, fut).await` |
| `asyncio.create_task(coro)` | `tokio::spawn(async { ... })` |
| `async def foo()` | `async fn foo()` |
| `await foo()` | `foo().await` |

### Tool 错误处理策略

nanobot 的 tool 不 panic，返回错误字符串让 LLM 感知：
```python
return f"Error executing {name}: {str(e)}"
```

Rust 版建议：
```rust
tool.execute(params).unwrap_or_else(|e| format!("Error: {}", e))
```

---

*生成时间：2026-03-25 | 基于 nanobot 源码分析*
