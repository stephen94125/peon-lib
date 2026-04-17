# Peon Runtime

[🇨🇳 简体中文](README.zh.md) | [🇬🇧 English](README.md)

**Peon Runtime** 是一个独立的零信任 Agent 执行引擎，用来取代 `rig` 框架的编排层。它专为请求级别的身份注入、多模态消息传递以及可插拔的 LLM 后端而设计。

---

## 🧠 为什么要造这个轮子

`rig-core` 的 `Tool` trait 没有任何机制能将请求级别的上下文（例如用户 ID）传递到工具执行层。它的 `ToolServer` 使用 `Arc<RwLock>` 在后台 `tokio::spawn` 中调度工具，在物理层面切断了 `tokio::task_local!` 的传播路径。

这使得在并发、多用户环境下强制执行**按请求身份检查**在架构上**完全不可能**——对于 Peon 这样的零信任系统而言，这是致命的。

> [!IMPORTANT]
> 这不是理论上的担忧。完全相同的限制已在 [rig Discussions #1298](https://github.com/0xPlaygrounds/rig/discussions/1298)（"How to manage agent state/context in tool execution?"）中被提出——目前**无人回答**。

**Peon Runtime** 的解决方案：
1. 将 `&RequestContext` 直接传递给每一次 `PeonTool::call()` 调用。
2. 工具在调用方任务中内联执行——不使用后台 Actor，没有上下文丢失。
3. 让 LLM 无法伪造用户身份（`uid`）——它只能控制 `args` 参数，永远无法控制 `ctx`。

---

## 🚀 快速起步

### 1. 环境配置

```bash
cd peon-runtime
cp .env.example .env
# 编辑 .env 填入您的 API Key
```

```dotenv
PROVIDER=openai
MODEL=gpt-4o-mini
API_KEY=sk-...
```

### 2. 运行范例

```bash
# 简单的一次性对话
cargo run -p peon-runtime --example simple_chat

# 指定提示文字
cargo run -p peon-runtime --example simple_chat -- "用一句话解释 Rust 的生命周期"

# 工具调用 + RequestContext 身份注入展示
cargo run -p peon-runtime --example tool_call

# 开启详细日志
RUST_LOG=debug cargo run -p peon-runtime --example tool_call
```

### 3. 在代码中使用

```rust
use peon_runtime::providers::openai::OpenAiProvider;
use peon_runtime::{AgentLoop, RequestContext};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let provider = OpenAiProvider::new("gpt-4o-mini", "sk-...");

    let agent = AgentLoop::builder(provider)
        .system_prompt("你是一个友善的助手。")
        .tool(my_tool)     // 实现 PeonTool trait
        .max_turns(10)
        .build();

    // UID 在这里设定——LLM 绝对无法伪造
    let ctx = RequestContext::new("user_12345");
    let response = agent.run("你好！", &[], &ctx).await?;

    println!("{}", response.output);
    Ok(())
}
```

---

## 🏗 架构总览

```text
┌─────────────────────────────────────────────┐
│ CompletionProvider                           │
│ OpenAI · Anthropic · Gemini · OpenRouter    │
└──────────────┬──────────────────────────────┘
               │
┌──────────────▼──────────────────────────────┐
│ AgentLoop                                    │
│  提示 → LLM → 工具调用? → 执行 → 循环        │
│  每次 tool.call() 都会收到 &RequestContext    │
└──────────────┬──────────────────────────────┘
               │
┌──────────────▼──────────────────────────────┐
│ PeonTool                                     │
│  fn call(&self, args, &ctx) → Result         │
│  ctx.uid() ← LLM 无法伪造                    │
└─────────────────────────────────────────────┘
```

### 模块结构

| 模块 | 用途 |
|:---|:---|
| `context` | `RequestContext` — 不可变的请求级身份上下文 |
| `message` | 多模态消息：文字、图片、音频、视频、文件 |
| `tool` | `PeonTool` trait，具备 `call(args, &ctx)` 签名 |
| `provider` | `CompletionProvider` trait + 请求/响应类型 |
| `agent` | `AgentLoop` — 核心执行引擎 |
| `error` | `ToolError`、`CompletionError`、`AgentError` |
| `providers/` | 内建的 LLM 供应商实现 |

---

## 🔌 支持的供应商

所有供应商通过统一的 `.env` 配置文件来切换：

| `PROVIDER=` | 供应商 | API 格式 | 认证方式 |
|:---|:---|:---|:---|
| `openai` | OpenAI | Chat Completions | `Authorization: Bearer` |
| `anthropic` | Anthropic (Claude) | Messages API | `x-api-key` + 版本头 |
| `gemini` | Google Gemini | generateContent | API Key 作为查询参数 |
| `openrouter` | OpenRouter | OpenAI 兼容格式 | `Authorization: Bearer` |

> [!TIP]
> OpenRouter 让您通过一个 API Key 就能访问所有主流模型（Claude、Gemini、Llama 等）——非常适合开发与测试。

---

## 🔐 PeonTool Trait 核心设计

与 `rig` 的关键架构差异：

```rust
pub trait PeonTool: Send + Sync {
    fn name(&self) -> &str;
    fn definition(&self, ctx: &RequestContext) -> BoxFuture<'_, ToolDefinition>;
    fn call(&self, args: &str, ctx: &RequestContext) -> BoxFuture<'_, Result<String, ToolError>>;
}
```

- LLM 控制 `args`（JSON 格式的参数字符串）。
- LLM **无法**控制 `ctx`——它由宿主应用注入。
- `definition()` 也接收 `ctx`，支持按用户定制工具的 JSON Schema。

---

## ⚙ Feature Flags（功能开关）

| 标志 | 默认 | 说明 |
|:---|:---|:---|
| `native` | ✅ | 启用基于 `reqwest` 的 HTTP 供应商 + `dotenvy` 读取 `.env` |
| `wasm` | — | 预留给未来的 WebAssembly 支持（无 `reqwest`，无 `tokio`） |

---

## 📄 许可协议

本项目采用双重许可，您可以自行选择 [MIT 许可协议](../LICENSE-MIT) 或 [Apache 许可协议 2.0 版](../LICENSE-APACHE)。
