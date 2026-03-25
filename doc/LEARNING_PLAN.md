# atombot 学习计划

> 目标：通过重写 nanobot，学习 Rust 异步 + Agent 开发
> 创建时间：2026-03-25
> 前提：已熟练掌握 Rust 基础，重点补 Rust 异步 + Agent 架构

---

## 整体路线

```
阶段 0：Rust 异步基础【补课】
   ↓
阶段 1：Provider —— 能跟 LLM 说话
   ↓
阶段 2：Tool —— 能执行动作
   ↓
阶段 3：Agent Loop —— 把 LLM + Tool 跑起来
   ↓
阶段 4：Bus 集成 —— 多渠道输入输出
   ↓
阶段 5：进阶 —— Streaming、并发 Tool、多 Agent
```

---

## 阶段 0 — Rust 异步补课

| 步骤 | 内容 | 状态 |
|------|------|------|
| 0.1 | `async/await` 执行模型：Future 是惰性的，不 await 不执行 | ⬜ |
| 0.2 | `tokio::main` 宏做了什么，Runtime 概念 | ⬜ |
| 0.3 | `tokio::spawn` — 并发任务 | ⬜ |
| 0.4 | `tokio::sync::mpsc` — 理解背压（现有 bus 代码已用到） | ⬜ |
| 0.5 | `tokio::select!` — 多路复用（agent loop 里用到） | ⬜ |

---

## 阶段 1 — Provider（能真正跟 LLM 聊天）

| 步骤 | 内容 | 状态 |
|------|------|------|
| 1.1 | 定义 `Message`、`Role` 等核心数据结构（`serde` 序列化） | ⬜ |
| 1.2 | 定义 `Provider` trait（`async_trait`） | ⬜ |
| 1.3 | 实现 `OpenAIProvider`：`reqwest` 发 HTTP，拿回 completion | ⬜ |
| 1.4 | 跑通：`cargo run` → 终端打一句话，收到 AI 回复 ✅ | ⬜ |
| 1.5 | 加 Tool 支持：把 tools schema 传给 LLM，解析 `tool_calls` 响应 | ⬜ |

---

## 阶段 2 — Tool

| 步骤 | 内容 | 状态 |
|------|------|------|
| 2.1 | 定义 `Tool` trait | ⬜ |
| 2.2 | `ToolRegistry`：`HashMap<String, Arc<dyn Tool>>` | ⬜ |
| 2.3 | 实现第一个 Tool：`ShellTool`（执行 shell 命令） | ⬜ |
| 2.4 | 实现第二个 Tool：`ReadFileTool` | ⬜ |
| 2.5 | 自动生成 JSON Schema 供 LLM 使用 | ⬜ |

---

## 阶段 3 — Agent Loop（核心）

| 步骤 | 内容 | 状态 |
|------|------|------|
| 3.1 | Agent 结构体设计：持有 provider + tools + history | ⬜ |
| 3.2 | 实现 ReAct 循环：LLM → tool_call → 执行 → 回传结果 → 再问 LLM | ⬜ |
| 3.3 | 错误处理：tool 执行失败怎么告诉 LLM | ⬜ |
| 3.4 | 最大轮次限制，防止死循环 | ⬜ |

---

## 阶段 4 — Bus 集成

| 步骤 | 内容 | 状态 |
|------|------|------|
| 4.1 | CLI 渠道：从标准输入读消息 → 塞进 bus | ⬜ |
| 4.2 | Agent 从 bus 收消息 → 跑 loop → 发回 bus | ⬜ |
| 4.3 | 渠道订阅出站消息 → 打印到终端 | ⬜ |
| 4.4 | 完整跑通：在终端和 AI 多轮对话，AI 会用工具 ✅ | ⬜ |

---

## 阶段 5 — 进阶（按兴趣选做）

- [ ] Streaming 输出（SSE 流式响应）
- [ ] 并发执行多个 tool（`tokio::join!` / `FuturesUnordered`）
- [ ] 多 Agent 协作（通过 bus 互相发消息）
- [ ] 持久化 history

---

## 当前状态

### 已完成 ✅
- `MessageBus` + `InboundMessage` / `OutboundMessage` 数据结构
- `mpsc`（入站）+ `broadcast`（出站）正确分离
- 模块结构：`agent` / `provider` / `tools` / `bus`

### 下一步
**阶段 0.2 + 1.1**：把 `main.rs` 改成 async main，同时定义 `Message` 数据结构。

---

## 推荐依赖

```toml
[dependencies]
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.12", features = ["json", "stream"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
async-trait = "0.1"
anyhow = "1"       # 简单错误处理，学习阶段首选
tracing = "0.1"    # 日志/调试
```
