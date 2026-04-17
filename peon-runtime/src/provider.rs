//! The `CompletionProvider` trait — abstracts over any LLM backend.
//!
//! `peon-runtime` defines this trait; concrete implementations live in
//! downstream crates (e.g., `peon-core` implements it via `PeonModel`
//! which wraps `rig`'s provider ecosystem).

use crate::error::CompletionError;
use crate::message::{AssistantContent, Message};
use crate::tool::ToolDefinition;
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;

/// A boxed, `Send`-compatible future for the provider trait.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

// ==========================================
// Request / Response types
// ==========================================

/// A request to send to the LLM.
///
/// Built by the `AgentLoop` on each turn from the accumulated
/// conversation history and available tool definitions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionRequest {
    /// System-level instructions (preamble).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,

    /// The conversation history (user messages, assistant responses, tool results).
    pub messages: Vec<Message>,

    /// Tool definitions available for this turn.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<ToolDefinition>,

    /// Sampling temperature (0.0 = deterministic, 2.0 = creative).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,

    /// Maximum tokens to generate.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u64>,

    /// Provider-specific additional parameters (e.g., top_p, stop sequences).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional_params: Option<serde_json::Value>,
}

/// The LLM's response to a completion request.
#[derive(Debug, Clone)]
pub struct CompletionResponse {
    /// The assistant's output — may contain text, tool calls, or both.
    pub content: Vec<AssistantContent>,

    /// Token usage statistics (if the provider reports them).
    pub usage: Option<Usage>,
}

/// Token usage statistics from an LLM response.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Usage {
    /// Number of tokens in the input prompt.
    pub input_tokens: Option<u64>,
    /// Number of tokens generated in the output.
    pub output_tokens: Option<u64>,
    /// Number of cached input tokens (if supported by provider).
    pub cached_input_tokens: Option<u64>,
}

impl std::ops::AddAssign for Usage {
    fn add_assign(&mut self, rhs: Self) {
        self.input_tokens = match (self.input_tokens, rhs.input_tokens) {
            (Some(a), Some(b)) => Some(a + b),
            (Some(a), None) | (None, Some(a)) => Some(a),
            (None, None) => None,
        };
        self.output_tokens = match (self.output_tokens, rhs.output_tokens) {
            (Some(a), Some(b)) => Some(a + b),
            (Some(a), None) | (None, Some(a)) => Some(a),
            (None, None) => None,
        };
        self.cached_input_tokens = match (self.cached_input_tokens, rhs.cached_input_tokens) {
            (Some(a), Some(b)) => Some(a + b),
            (Some(a), None) | (None, Some(a)) => Some(a),
            (None, None) => None,
        };
    }
}

// ==========================================
// CompletionProvider trait
// ==========================================

/// Trait for LLM completion providers.
///
/// Implementations bridge the gap between `peon-runtime`'s message types
/// and a specific LLM API (OpenAI, Anthropic, Gemini, etc.).
///
/// This trait is object-safe — you can use `Box<dyn CompletionProvider>`
/// or `Arc<dyn CompletionProvider>` for dynamic dispatch.
///
/// # Implementor Responsibilities
///
/// 1. Convert `CompletionRequest` into the provider's wire format.
/// 2. Send the HTTP request and parse the response.
/// 3. Convert tool calls and text responses back into `CompletionResponse`.
///
/// # Example
///
/// ```rust,ignore
/// struct MyProvider { api_key: String }
///
/// impl CompletionProvider for MyProvider {
///     fn complete<'a>(
///         &'a self,
///         request: CompletionRequest,
///     ) -> BoxFuture<'a, Result<CompletionResponse, CompletionError>> {
///         Box::pin(async move {
///             // ... HTTP request to LLM API ...
///             Ok(CompletionResponse {
///                 content: vec![AssistantContent::Text { text: "Hello!".into() }],
///                 usage: None,
///             })
///         })
///     }
/// }
/// ```
pub trait CompletionProvider: Send + Sync {
    /// Send a completion request and return the LLM's response.
    fn complete<'a>(
        &'a self,
        request: CompletionRequest,
    ) -> BoxFuture<'a, Result<CompletionResponse, CompletionError>>;
}
