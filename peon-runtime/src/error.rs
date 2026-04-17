//! Error types for the Peon runtime.

/// Errors that can occur during tool execution.
#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    /// The tool encountered an operational error.
    #[error("{0}")]
    CallError(String),

    /// The tool's input arguments could not be parsed.
    #[error("Invalid arguments: {0}")]
    InvalidArgs(String),

    /// A permission check failed.
    #[error("{0}")]
    PermissionDenied(String),
}

impl ToolError {
    /// Create a generic call error.
    pub fn call(msg: impl Into<String>) -> Self {
        Self::CallError(msg.into())
    }

    /// Create an invalid arguments error.
    pub fn invalid_args(msg: impl Into<String>) -> Self {
        Self::InvalidArgs(msg.into())
    }

    /// Create a permission denied error.
    pub fn permission_denied(msg: impl Into<String>) -> Self {
        Self::PermissionDenied(msg.into())
    }
}

/// Errors that can occur during LLM completion.
#[derive(Debug, thiserror::Error)]
pub enum CompletionError {
    /// The HTTP request to the LLM provider failed.
    #[error("Provider request failed: {0}")]
    RequestError(String),

    /// The LLM response could not be parsed.
    #[error("Failed to parse provider response: {0}")]
    ParseError(String),

    /// The provider returned an error status.
    #[error("Provider error: {0}")]
    ProviderError(String),

    /// An unexpected error from the underlying provider implementation.
    #[error(transparent)]
    Other(Box<dyn std::error::Error + Send + Sync>),
}

/// Errors from the agent execution loop.
#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    /// The agent exceeded the maximum number of turns without producing a final answer.
    #[error("Agent exceeded maximum turns ({0}) without producing a final response")]
    MaxTurnsExceeded(usize),

    /// A tool referenced by the LLM doesn't exist in the registered toolset.
    #[error("Tool not found: '{0}'")]
    ToolNotFound(String),

    /// A tool call failed during execution.
    #[error("Tool '{name}' failed: {error}")]
    ToolCallFailed {
        name: String,
        error: ToolError,
    },

    /// The LLM completion request failed.
    #[error(transparent)]
    CompletionFailed(#[from] CompletionError),

    /// The agent has no tools registered.
    #[error("No tools registered")]
    NoTools,
}
