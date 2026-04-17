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
//!
//! // 1. Implement CompletionProvider for your LLM backend
//! // 2. Implement PeonTool for your tools
//! // 3. Build and run the agent
//!
//! let agent = AgentLoop::builder(my_provider)
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
//! - `native` (default): Enables native platform support.
//! - `wasm`: Reserved for future WebAssembly support.

pub mod agent;
pub mod context;
pub mod error;
pub mod message;
pub mod provider;
pub mod tool;

// Re-export primary types at the crate root for ergonomic imports.
pub use agent::{AgentLoop, AgentLoopBuilder, AgentResponse};
pub use context::RequestContext;
pub use error::{AgentError, CompletionError, ToolError};
pub use message::{AssistantContent, ContentPart, Message};
pub use provider::{CompletionProvider, CompletionRequest, CompletionResponse, Usage};
pub use tool::{BoxFuture, PeonTool, ToolDefinition};
