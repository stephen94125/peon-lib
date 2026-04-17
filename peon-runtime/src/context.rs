//! Request-scoped context passed to every tool invocation.
//!
//! This is the **core architectural solution** to `rig`'s inability to propagate
//! user identity through its `ToolServer`. By passing `RequestContext` as a
//! function argument to `PeonTool::call()`, the UID is physically bound to each
//! invocation at the Rust type level — unforgeable by the LLM.

use std::collections::HashMap;

/// Immutable, request-scoped context carried through the entire agent execution.
///
/// Created once per incoming request (e.g., a Telegram webhook) and passed by
/// reference to every tool call. The LLM never sees or controls this struct —
/// it only controls the `args: &str` parameter.
///
/// # Example
/// ```
/// use peon_runtime::RequestContext;
///
/// let ctx = RequestContext::new("7444174610")
///     .with_metadata("chat_type", "group")
///     .with_metadata("platform", "telegram");
///
/// assert_eq!(ctx.uid(), "7444174610");
/// assert_eq!(ctx.get_metadata("chat_type"), Some("group"));
/// ```
#[derive(Debug, Clone)]
pub struct RequestContext {
    /// The authenticated user identity.
    /// Set by the host application (e.g., Telegram bot) at request creation time.
    /// Tools use this for Casbin permission enforcement.
    uid: String,

    /// Optional key-value metadata for extensibility.
    /// Useful for passing platform-specific info (chat_type, locale, etc.)
    /// without breaking the core API.
    metadata: HashMap<String, String>,
}

impl RequestContext {
    /// Create a new context with the given user identity.
    pub fn new(uid: impl Into<String>) -> Self {
        Self {
            uid: uid.into(),
            metadata: HashMap::new(),
        }
    }

    /// The authenticated user ID for this request.
    pub fn uid(&self) -> &str {
        &self.uid
    }

    /// Add a metadata key-value pair (builder-style).
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Look up a metadata value by key.
    pub fn get_metadata(&self, key: &str) -> Option<&str> {
        self.metadata.get(key).map(|v| v.as_str())
    }

    /// Iterate over all metadata entries.
    pub fn metadata(&self) -> &HashMap<String, String> {
        &self.metadata
    }
}
