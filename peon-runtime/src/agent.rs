//! The core agent execution loop.
//!
//! This module replaces `rig`'s `Agent` + `ToolServer` + `PromptRequest`
//! with a simple, flat loop that gives us full control over context
//! injection and tool dispatch.

use crate::context::RequestContext;
use crate::error::AgentError;
use crate::message::{AssistantContent, Message};
use crate::provider::{CompletionProvider, CompletionRequest, CompletionResponse, Usage};
use crate::tool::PeonTool;
use log::{debug, info, warn};

// ==========================================
// Agent Response
// ==========================================

/// The result of a successful agent execution.
#[derive(Debug, Clone)]
pub struct AgentResponse {
    /// The final text output from the agent.
    pub output: String,
    /// The complete conversation history including all tool calls.
    pub messages: Vec<Message>,
    /// Accumulated token usage across all turns.
    pub usage: Usage,
}

// ==========================================
// Agent Builder
// ==========================================

/// Builder for constructing an `AgentLoop`.
///
/// # Example
///
/// ```rust,ignore
/// let agent = AgentLoop::builder(my_provider)
///     .system_prompt("You are a helpful assistant.")
///     .tool(read_skill_tool)
///     .tool(execute_script_tool)
///     .max_turns(10)
///     .temperature(0.7)
///     .build();
///
/// let response = agent.run("Hello!", &[], &ctx).await?;
/// ```
pub struct AgentLoopBuilder<P: CompletionProvider> {
    provider: P,
    system_prompt: Option<String>,
    tools: Vec<Box<dyn PeonTool>>,
    max_turns: usize,
    temperature: Option<f64>,
    max_tokens: Option<u64>,
}

impl<P: CompletionProvider> AgentLoopBuilder<P> {
    /// Create a new builder with the given completion provider.
    pub fn new(provider: P) -> Self {
        Self {
            provider,
            system_prompt: None,
            tools: Vec::new(),
            max_turns: 10,
            temperature: None,
            max_tokens: None,
        }
    }

    /// Set the system prompt (preamble).
    pub fn system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    /// Register a tool.
    pub fn tool(mut self, tool: impl PeonTool + 'static) -> Self {
        self.tools.push(Box::new(tool));
        self
    }

    /// Set the maximum number of LLM round-trips (default: 10).
    ///
    /// Each round-trip consists of: send prompt → get response → (optionally execute tools).
    /// Set higher for complex multi-step tasks.
    pub fn max_turns(mut self, turns: usize) -> Self {
        self.max_turns = turns;
        self
    }

    /// Set the sampling temperature.
    pub fn temperature(mut self, temp: f64) -> Self {
        self.temperature = Some(temp);
        self
    }

    /// Set the maximum tokens to generate per turn.
    pub fn max_tokens(mut self, tokens: u64) -> Self {
        self.max_tokens = Some(tokens);
        self
    }

    /// Build the `AgentLoop`.
    pub fn build(self) -> AgentLoop<P> {
        AgentLoop {
            provider: self.provider,
            system_prompt: self.system_prompt,
            tools: self.tools,
            max_turns: self.max_turns,
            temperature: self.temperature,
            max_tokens: self.max_tokens,
        }
    }
}

// ==========================================
// Agent Loop
// ==========================================

/// The core agent execution engine.
///
/// Orchestrates the LLM conversation loop: prompt → response → tool calls → repeat.
///
/// Unlike `rig`'s `Agent`, this struct:
/// - Passes `RequestContext` directly to every tool call (no task-local needed)
/// - Executes tools inline (no background ToolServer / actor)
/// - Accepts external chat history for session continuity
/// - Supports multimodal messages natively
pub struct AgentLoop<P: CompletionProvider> {
    provider: P,
    system_prompt: Option<String>,
    tools: Vec<Box<dyn PeonTool>>,
    max_turns: usize,
    temperature: Option<f64>,
    max_tokens: Option<u64>,
}

impl<P: CompletionProvider> AgentLoop<P> {
    /// Create a new builder.
    pub fn builder(provider: P) -> AgentLoopBuilder<P> {
        AgentLoopBuilder::new(provider)
    }

    /// Execute the agent loop.
    ///
    /// # Arguments
    ///
    /// - `prompt`: The user's input message (can be multimodal via `Message`).
    /// - `chat_history`: Previous conversation turns for context continuity.
    ///   Pass `&[]` for a fresh conversation.
    /// - `ctx`: The request-scoped context (contains UID, metadata).
    ///   This is passed to every tool invocation, guaranteeing identity isolation.
    ///
    /// # Returns
    ///
    /// An `AgentResponse` containing the final text output, the complete
    /// message history (useful for persisting to a session store), and
    /// accumulated token usage.
    pub async fn run(
        &self,
        prompt: impl Into<Message>,
        chat_history: &[Message],
        ctx: &RequestContext,
    ) -> Result<AgentResponse, AgentError> {
        let prompt_msg = prompt.into();
        info!("Agent run: uid='{}', prompt={:?}", ctx.uid(), prompt_msg.text().unwrap_or_default());

        // Build the working message list: history + new prompt
        let mut messages: Vec<Message> = chat_history.to_vec();
        messages.push(prompt_msg);

        let mut total_usage = Usage::default();

        for turn in 0..self.max_turns {
            debug!("Turn {}/{}", turn + 1, self.max_turns);

            // 1. Collect tool definitions
            let mut tool_defs = Vec::new();
            for tool in &self.tools {
                tool_defs.push(tool.definition(ctx).await);
            }
            debug!("Registered {} tool definition(s)", tool_defs.len());

            // 2. Build completion request
            let request = CompletionRequest {
                system_prompt: self.system_prompt.clone(),
                messages: messages.clone(),
                tools: tool_defs,
                temperature: self.temperature,
                max_tokens: self.max_tokens,
                additional_params: None,
            };

            // 3. Send to LLM
            let response: CompletionResponse = self.provider.complete(request).await?;

            if let Some(usage) = response.usage.clone() {
                total_usage += usage;
            }

            // 4. Partition response into tool calls and text parts
            let (tool_calls, texts): (Vec<_>, Vec<_>) =
                response.content.iter().partition(|c| matches!(c, AssistantContent::ToolCall { .. }));

            // 5. Append assistant message to history
            messages.push(Message::assistant(response.content.clone()));

            // 6. If no tool calls, we're done — extract text and return
            if tool_calls.is_empty() {
                let output = texts
                    .iter()
                    .filter_map(|c| {
                        if let AssistantContent::Text { text } = c {
                            Some(text.as_str())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n");

                info!("Agent response (turn {}): {}", turn + 1, output);

                return Ok(AgentResponse {
                    output,
                    messages,
                    usage: total_usage,
                });
            }

            // 7. Execute tool calls sequentially
            //    (concurrent execution can be added later without breaking changes)
            for tool_call in &tool_calls {
                if let AssistantContent::ToolCall {
                    id,
                    name,
                    arguments,
                } = tool_call
                {
                    info!("Tool call: {}({})", name, arguments);

                    // Find the tool by name
                    let tool = self
                        .tools
                        .iter()
                        .find(|t| t.name() == name.as_str())
                        .ok_or_else(|| AgentError::ToolNotFound(name.clone()))?;

                    // Execute with context injection — the whole point of this runtime!
                    let result = match tool.call(arguments, ctx).await {
                        Ok(output) => {
                            info!("Tool '{}' succeeded ({} bytes)", name, output.len());
                            output
                        }
                        Err(e) => {
                            warn!("Tool '{}' failed: {}", name, e);
                            // Return the error as a string so the LLM can reason about it.
                            // This matches rig's behavior — tool errors are not fatal to the loop.
                            e.to_string()
                        }
                    };

                    // Append the tool result to history
                    messages.push(Message::tool_result(id.clone(), result));
                }
            }

            // Loop continues — the LLM will see the tool results on the next turn
        }

        // Exhausted all turns without a final text response
        Err(AgentError::MaxTurnsExceeded(self.max_turns))
    }
}

// ==========================================
// Allow `&str` and `String` to be used as prompts
// ==========================================

impl From<&str> for Message {
    fn from(s: &str) -> Self {
        Message::user_text(s)
    }
}

impl From<String> for Message {
    fn from(s: String) -> Self {
        Message::user_text(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::AssistantContent;
    use crate::provider::CompletionResponse;
    use crate::tool::{BoxFuture, ToolDefinition};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    // === Mock Provider ===

    struct MockProvider {
        responses: Vec<CompletionResponse>,
        call_count: AtomicUsize,
    }

    impl MockProvider {
        fn new(responses: Vec<CompletionResponse>) -> Self {
            Self {
                responses,
                call_count: AtomicUsize::new(0),
            }
        }
    }

    impl CompletionProvider for MockProvider {
        fn complete<'a>(
            &'a self,
            _request: CompletionRequest,
        ) -> crate::provider::BoxFuture<'a, Result<CompletionResponse, crate::error::CompletionError>>
        {
            let idx = self.call_count.fetch_add(1, Ordering::SeqCst);
            let response = self.responses[idx].clone();
            Box::pin(async move { Ok(response) })
        }
    }

    // === Mock Tool ===

    struct EchoTool {
        call_log: Arc<std::sync::Mutex<Vec<(String, String)>>>,
    }

    impl PeonTool for EchoTool {
        fn name(&self) -> &str {
            "echo"
        }

        fn definition(&self, _ctx: &RequestContext) -> BoxFuture<'_, ToolDefinition> {
            Box::pin(async {
                ToolDefinition {
                    name: "echo".into(),
                    description: "Echoes back the input".into(),
                    parameters: serde_json::json!({"type": "object", "properties": {}}),
                }
            })
        }

        fn call(
            &self,
            args: &str,
            ctx: &RequestContext,
        ) -> BoxFuture<'_, Result<String, crate::error::ToolError>> {
            let args = args.to_string();
            let uid = ctx.uid().to_string();
            let log = self.call_log.clone();
            Box::pin(async move {
                log.lock().unwrap().push((uid.clone(), args.clone()));
                Ok(format!("echo from {}: {}", uid, args))
            })
        }
    }

    #[tokio::test]
    async fn test_simple_text_response() {
        let provider = MockProvider::new(vec![CompletionResponse {
            content: vec![AssistantContent::Text {
                text: "Hello!".into(),
            }],
            usage: None,
        }]);

        let agent = AgentLoop::builder(provider)
            .system_prompt("You are helpful.")
            .max_turns(5)
            .build();

        let ctx = RequestContext::new("user123");
        let result = agent.run("Hi", &[], &ctx).await.unwrap();

        assert_eq!(result.output, "Hello!");
        assert_eq!(result.messages.len(), 2); // user + assistant
    }

    #[tokio::test]
    async fn test_tool_call_then_text() {
        let call_log = Arc::new(std::sync::Mutex::new(Vec::new()));

        let provider = MockProvider::new(vec![
            // Turn 1: LLM requests a tool call
            CompletionResponse {
                content: vec![AssistantContent::ToolCall {
                    id: "call_1".into(),
                    name: "echo".into(),
                    arguments: r#"{"input": "test"}"#.into(),
                }],
                usage: None,
            },
            // Turn 2: LLM returns text after seeing tool result
            CompletionResponse {
                content: vec![AssistantContent::Text {
                    text: "Done!".into(),
                }],
                usage: None,
            },
        ]);

        let agent = AgentLoop::builder(provider)
            .tool(EchoTool {
                call_log: call_log.clone(),
            })
            .max_turns(5)
            .build();

        let ctx = RequestContext::new("user456");
        let result = agent.run("Do the thing", &[], &ctx).await.unwrap();

        assert_eq!(result.output, "Done!");

        // Verify the tool was called with the correct UID
        let log = call_log.lock().unwrap();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].0, "user456"); // UID was correctly passed!
        assert_eq!(log[0].1, r#"{"input": "test"}"#);
    }

    #[tokio::test]
    async fn test_max_turns_exceeded() {
        // Provider always returns tool calls, never text
        let responses: Vec<CompletionResponse> = (0..5)
            .map(|i| CompletionResponse {
                content: vec![AssistantContent::ToolCall {
                    id: format!("call_{}", i),
                    name: "echo".into(),
                    arguments: "{}".into(),
                }],
                usage: None,
            })
            .collect();

        let provider = MockProvider::new(responses);
        let agent = AgentLoop::builder(provider)
            .tool(EchoTool {
                call_log: Arc::new(std::sync::Mutex::new(Vec::new())),
            })
            .max_turns(3)
            .build();

        let ctx = RequestContext::new("user789");
        let result = agent.run("Loop forever", &[], &ctx).await;

        assert!(matches!(result, Err(AgentError::MaxTurnsExceeded(3))));
    }

    #[tokio::test]
    async fn test_uid_isolation_between_requests() {
        let call_log = Arc::new(std::sync::Mutex::new(Vec::new()));

        // Two separate runs with different UIDs
        for uid in &["alice", "bob"] {
            let provider = MockProvider::new(vec![
                CompletionResponse {
                    content: vec![AssistantContent::ToolCall {
                        id: "call_1".into(),
                        name: "echo".into(),
                        arguments: "{}".into(),
                    }],
                    usage: None,
                },
                CompletionResponse {
                    content: vec![AssistantContent::Text {
                        text: "ok".into(),
                    }],
                    usage: None,
                },
            ]);

            let agent = AgentLoop::builder(provider)
                .tool(EchoTool {
                    call_log: call_log.clone(),
                })
                .max_turns(5)
                .build();

            let ctx = RequestContext::new(*uid);
            agent.run("test", &[], &ctx).await.unwrap();
        }

        let log = call_log.lock().unwrap();
        assert_eq!(log.len(), 2);
        assert_eq!(log[0].0, "alice");
        assert_eq!(log[1].0, "bob");
    }

    #[tokio::test]
    async fn test_chat_history_preserved() {
        let provider = MockProvider::new(vec![CompletionResponse {
            content: vec![AssistantContent::Text {
                text: "Continued!".into(),
            }],
            usage: None,
        }]);

        let history = vec![
            Message::user_text("First message"),
            Message::assistant_text("First response"),
        ];

        let agent = AgentLoop::builder(provider).max_turns(5).build();
        let ctx = RequestContext::new("user");
        let result = agent.run("Second message", &history, &ctx).await.unwrap();

        assert_eq!(result.output, "Continued!");
        // history(2) + new prompt(1) + assistant response(1)
        assert_eq!(result.messages.len(), 4);
    }
}
