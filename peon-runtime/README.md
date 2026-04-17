# Peon Runtime

[🇨🇳 简体中文](README.zh.md) | [🇬🇧 English](README.md)

**Peon Runtime** is a standalone, zero-trust agent execution engine that replaces `rig`'s orchestration layer with a purpose-built loop designed for request-scoped identity injection, multimodal messaging, and pluggable LLM providers.

---

## 🧠 Why This Exists

The `rig-core` library's `Tool` trait has no mechanism for passing request-scoped context (like a user ID) into tool execution. Its `ToolServer` dispatches tools via `Arc<RwLock>` inside a background `tokio::spawn`, which physically severs `tokio::task_local!` propagation.

This makes it **architecturally impossible** to enforce per-request identity checks in a concurrent, multi-user environment — a deal-breaker for zero-trust systems like Peon.

> [!IMPORTANT]
> This is not a theoretical concern. The exact same limitation was raised in [rig Discussions #1298](https://github.com/0xPlaygrounds/rig/discussions/1298) ("How to manage agent state/context in tool execution?") — currently **Unanswered**.

**Peon Runtime** solves this by:
1. Passing `&RequestContext` directly to every `PeonTool::call()` invocation.
2. Executing tools inline within the calling task — no background actors, no context loss.
3. Making the user identity (`uid`) unforgeable by the LLM — it only controls the `args`, never the `ctx`.

---

## 🚀 Quick Start

### 1. Environment Setup

```bash
cd peon-runtime
cp .env.example .env
# Edit .env with your API key
```

```dotenv
PROVIDER=openai
MODEL=gpt-4o-mini
API_KEY=sk-...
```

### 2. Run the Examples

```bash
# Simple one-shot chat
cargo run -p peon-runtime --example simple_chat

# Chat with a custom prompt
cargo run -p peon-runtime --example simple_chat -- "Explain Rust lifetimes in one sentence"

# Tool calling with RequestContext injection
cargo run -p peon-runtime --example tool_call

# Enable debug logging
RUST_LOG=debug cargo run -p peon-runtime --example tool_call
```

### 3. Use in Your Code

```rust
use peon_runtime::providers::openai::OpenAiProvider;
use peon_runtime::{AgentLoop, RequestContext};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let provider = OpenAiProvider::new("gpt-4o-mini", "sk-...");

    let agent = AgentLoop::builder(provider)
        .system_prompt("You are a helpful assistant.")
        .tool(my_tool)     // implements PeonTool
        .max_turns(10)
        .build();

    // The UID is set here — unforgeable by the LLM
    let ctx = RequestContext::new("user_12345");
    let response = agent.run("Hello!", &[], &ctx).await?;

    println!("{}", response.output);
    Ok(())
}
```

---

## 🏗 Architecture

```text
┌─────────────────────────────────────────────┐
│ CompletionProvider                           │
│ OpenAI · Anthropic · Gemini · OpenRouter    │
└──────────────┬──────────────────────────────┘
               │
┌──────────────▼──────────────────────────────┐
│ AgentLoop                                    │
│  prompt → LLM → tool_call? → execute → loop  │
│  Every tool.call() receives &RequestContext   │
└──────────────┬──────────────────────────────┘
               │
┌──────────────▼──────────────────────────────┐
│ PeonTool                                     │
│  fn call(&self, args, &ctx) → Result         │
│  ctx.uid() ← unforgeable by LLM             │
└─────────────────────────────────────────────┘
```

### Module Layout

| Module | Purpose |
|:---|:---|
| `context` | `RequestContext` — immutable, request-scoped identity |
| `message` | Multimodal messages: text, images, audio, video, files |
| `tool` | `PeonTool` trait with `call(args, &ctx)` |
| `provider` | `CompletionProvider` trait + request/response types |
| `agent` | `AgentLoop` — the core execution engine |
| `error` | `ToolError`, `CompletionError`, `AgentError` |
| `providers/` | Built-in LLM provider implementations |

---

## 🔌 Supported Providers

All providers are accessed through a unified `.env` configuration:

| `PROVIDER=` | Provider | API Format | Auth |
|:---|:---|:---|:---|
| `openai` | OpenAI | Chat Completions | `Authorization: Bearer` |
| `anthropic` | Anthropic (Claude) | Messages API | `x-api-key` + version header |
| `gemini` | Google Gemini | generateContent | API key as query param |
| `openrouter` | OpenRouter | OpenAI-compatible | `Authorization: Bearer` |

> [!TIP]
> OpenRouter gives you access to all major models (Claude, Gemini, Llama, etc.) through a single API key — ideal for development and testing.

---

## 🔐 The PeonTool Trait

The key architectural difference from `rig`:

```rust
pub trait PeonTool: Send + Sync {
    fn name(&self) -> &str;
    fn definition(&self, ctx: &RequestContext) -> BoxFuture<'_, ToolDefinition>;
    fn call(&self, args: &str, ctx: &RequestContext) -> BoxFuture<'_, Result<String, ToolError>>;
}
```

- The LLM controls `args` (the JSON arguments string).
- The LLM **cannot** control `ctx` — it's injected by the host application.
- `definition()` also receives `ctx`, enabling per-user tool schema customization.

---

## ⚙ Feature Flags

| Flag | Default | Description |
|:---|:---|:---|
| `native` | ✅ | Enables HTTP-based providers via `reqwest` + `.env` loading via `dotenvy` |
| `wasm` | — | Reserved for future WebAssembly support (no `reqwest`, no `tokio`) |

---

## 📄 License

This project is dual-licensed under either the [MIT license](../LICENSE-MIT) or the [Apache License, Version 2.0](../LICENSE-APACHE), at your option.
