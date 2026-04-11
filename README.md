# Atombot 🤖

> A Rust-native AI agent framework with tool calling superpowers. The seed of something **big**.

[![Rust](https://img.shields.io/badge/Rust-1.75+-orange.svg)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

## What is Atombot?

Atombot is a **pure Rust** AI agent framework that brings intelligent tool-calling agents to the Rust ecosystem. It's lightweight, fast, and designed for developers who want the power of AI agents without leaving the Rust world.

Think of it as **your personal AI coding assistant** — read files, explore codebases, execute tasks — all powered by Large Language Models with the reliability and speed Rust is known for.

## Why Rust?

- **Blazing fast** — No GC pauses, no startup delay, instant response
- **Memory safe** — Fearless concurrency, no segfaults
- **Cross-platform** — Runs everywhere Rust runs
- **First-class async** — Built on Tokio for maximum performance
- **Zero dependencies at runtime** — Statically compiled binary

## Project Structure

```
atombot/
├── src/              # Core atombot library
├── examples/         # CLI example
├── web_ui/          # Web browser UI (Axum-based)
├── tauri_ui/        # Desktop UI (Tauri 2.0)
└── Cargo.toml       # Workspace manifest
```

## Three Ways to Run

### 1. Web UI

```bash
cd web_ui
echo "OPENAI_API_KEY=your_key" > ../.env
cargo run
```
Then open http://127.0.0.1:8080

### 2. Desktop App (Tauri)

```bash
cd tauri_ui
echo "OPENAI_API_KEY=your_key" > ../.env
./dev.sh   # Starts HTTP server + Tauri dev
```

### 3. CLI Example

```bash
echo "OPENAI_API_KEY=your_key" > .env
cargo run --example talk_to_openai
```

## Features

### Core Agent
- **Multi-turn conversations** with automatic context management
- **Tool calling** — Extensible tool system with `Tool` trait
- **Message windowing** — Smart context pruning to handle long conversations
- **Error handling** — Graceful degradation with typed errors

### Built-in Tools
- 📁 **File Reader** — Read files with path sandboxing and pagination support

## Configuration

Create a `.env` file in the project root:

```bash
OPENAI_API_KEY=your_api_key_here
# Optional:
OPENAI_API_BASE=https://api.minimax.chat/v1
OPENAI_MODEL=MiniMax-M2.7
```

| Environment Variable | Default | Description |
|---------------------|---------|-------------|
| `OPENAI_API_KEY` | **required** | Your API key |
| `OPENAI_API_BASE` | `https://api.minimax.chat/v1` | API endpoint |
| `OPENAI_MODEL` | `MiniMax-M2.7` | Model to use |

## Architecture

```
┌─────────────────────────────────────┐
│              atombot                │
├─────────────────────────────────────┤
│  Agent     │  Tools   │  API Client │
├────────────┼──────────┼─────────────┤
│ Message    │ Registry │   Config    │
│  Window    │          │             │
└─────────────────────────────────────┘
```

## Adding Custom Tools

Implement the `Tool` trait to create your own tools:

```rust
use async_trait::async_trait;
use async_openai::types::chat::{ChatCompletionTools, FunctionObject};

pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn parameters_schema(&self) -> serde_json::Value;
    async fn execute(&self, args: serde_json::Value) -> Result<String, ToolError>;
}
```

Register your tool:

```rust
let mut registry = ToolRegistry::new();
registry.register(MyCustomTool::new());
```

## Roadmap 🗺️

This is just the beginning. The roadmap toward **OpenClaw**:

- [ ] **Memory System** — Persistent conversation history with summarization
- [ ] **More Tools** — Shell execution, web search, git operations
- [ ] **Streaming** — Real-time token streaming in Web UI
- [ ] **Multi-agent** — Agent coordination and delegation
- [ ] **Plugin System** — Dynamic tool loading at runtime
- [ ] **Persistence** — Session management and recovery
- [ ] **Observability** — Tracing, metrics, and debugging tools

## The Vision: OpenClaw 🦞

Atombot is the foundation of something larger: **OpenClaw** — a full-featured Rust AI agent framework inspired by Claude's capabilities. We want to build:

- A **powerful** agent that can tackle complex, multi-step tasks
- A **safe** agent with proper sandboxing and permissions
- A **portable** agent that runs anywhere Rust does
- A **extensible** agent that welcomes contributions

The journey from Atombot to OpenClaw will be documented as the project evolves.

## Contributing

Contributions are welcome! This is a young project with lots of room for improvement.

```bash
# Clone and start coding
cargo build
cargo test
```

## Acknowledgments

- Built with [async-openai](https://github.com/et系ason/async-openai) for OpenAI API compatibility
- Powered by [Tokio](https://tokio.rs) for async excellence

---

<div align="center">

**Atombot** — *The tiny agent with infinite potential*

⭐ Star us on GitHub if you find this interesting!

</div>
