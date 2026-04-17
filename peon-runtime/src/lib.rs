//! # Peon Runtime
//!
//! A runtime-agnostic agent execution engine with zero-trust context injection,
//! multimodal messaging, and pluggable LLM providers.
//!
//! ## Why This Exists
//!
//! The `rig-core` library's `Tool` trait has no mechanism for passing
//! request-scoped context (like a user ID) into tool execution. Its
//! `ToolServer` dispatches tools via `Arc<RwLock>`, which severs
//! `tokio::task_local!` propagation. This crate replaces `rig`'s
//! orchestration layer with a simple, flat loop that passes
//! `RequestContext` directly to every tool call.
//!
//! ## Architecture
//!
//! ```text
//! ┌────────────────────────────────────────────┐
//! │ CompletionProvider (you implement this)     │
//! │ e.g., wraps OpenAI / Anthropic / Gemini    │
//! └──────────────┬─────────────────────────────┘
//!                │
//! ┌──────────────▼─────────────────────────────┐
//! │ AgentLoop                                   │
//! │  prompt → LLM → tool_call? → execute → loop │
//! │  Every tool.call() receives &RequestContext  │
//! └──────────────┬─────────────────────────────┘
//!                │
//! ┌──────────────▼─────────────────────────────┐
//! │ PeonTool (you implement this)               │
//! │  fn call(&self, args, &ctx) → Result        │
//! │  ctx.uid() ← unforgeable by LLM            │
//! └────────────────────────────────────────────┘
//! ```
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use peon_runtime::*;
//! use peon_runtime::providers::openai::OpenAiProvider;
//!
//! let provider = OpenAiProvider::new("gpt-4o", "sk-...");
//!
//! let agent = AgentLoop::builder(provider)
//!     .system_prompt("You are a helpful assistant.")
//!     .tool(my_tool)
//!     .max_turns(10)
//!     .build();
//!
//! let ctx = RequestContext::new("user_12345");
//! let response = agent.run("Hello!", &[], &ctx).await?;
//! println!("{}", response.output);
//! ```
//!
//! ## Feature Flags
//!
//! - `native` (default): Enables HTTP-based providers (OpenAI, Anthropic, Gemini) via `reqwest`.
//! - `wasm`: Reserved for future WebAssembly support.

pub mod agent;
pub mod context;
pub mod error;
pub mod message;
pub mod provider;
pub mod tool;

#[cfg(feature = "native")]
pub mod providers;

// Re-export agent-facing types at the crate root.
// CompletionProvider is re-exported because AgentLoop::builder() needs it.
// CompletionRequest/Response are NOT re-exported — use providers::* directly.
pub use agent::{AgentLoop, AgentLoopBuilder, AgentResponse};
pub use context::RequestContext;
pub use error::{AgentError, CompletionError, ToolError};
pub use message::{AssistantContent, ContentPart, Message};
pub use provider::CompletionProvider;
pub use tool::{BoxFuture, PeonTool, ToolDefinition};
