//! Anthropic (Claude) provider implementation.
//!
//! # API Differences from OpenAI
//!
//! Anthropic's Messages API is fundamentally different from OpenAI's:
//!
//! - **System prompt**: Top-level `system` field, NOT a message with `role: "system"`.
//! - **Tool calls**: Called `tool_use` (not `tool_calls`). Part of the `content` array,
//!   not a separate field.
//! - **Tool results**: `role: "user"` with a `tool_result` content block referencing
//!   `tool_use_id` (not `tool_call_id`).
//! - **Images**: Uses `source: { type: "base64", data, media_type }` (not `image_url`).
//!   URL images use `source: { type: "url", url }`.
//! - **No audio/video support**: Anthropic does not support audio or video content.
//! - **max_tokens is REQUIRED**: Unlike OpenAI where it's optional, Anthropic will
//!   reject requests without `max_tokens`. We default to 8192.
//! - **Content blocks**: Even text responses come as `[{"type":"text","text":"..."}]`,
//!   never as a bare string.
//! - **Versioning header**: Requires `anthropic-version: 2023-06-01` header.

use crate::error::CompletionError;
use crate::message::{AssistantContent, ContentPart, Message};
use crate::provider::{BoxFuture, CompletionProvider, CompletionRequest, CompletionResponse, Usage};
use log::debug;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use serde::Deserialize;

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1";
const ANTHROPIC_VERSION: &str = "2023-06-01";
const DEFAULT_MAX_TOKENS: u64 = 8192;

/// Anthropic (Claude) completion provider.
///
/// # Example
///
/// ```rust,ignore
/// use peon_runtime::providers::anthropic::AnthropicProvider;
///
/// let provider = AnthropicProvider::new("claude-sonnet-4-20250514", &api_key);
/// ```
#[derive(Clone)]
pub struct AnthropicProvider {
    client: reqwest::Client,
    base_url: String,
    model: String,
    api_key: String,
    extra_headers: HeaderMap,
}

impl AnthropicProvider {
    /// Create a provider for Anthropic's API.
    pub fn new(model: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: ANTHROPIC_API_URL.into(),
            model: model.into(),
            api_key: api_key.into(),
            extra_headers: HeaderMap::new(),
        }
    }

    /// Create a provider with a custom base URL (e.g., proxy).
    pub fn custom(
        base_url: impl Into<String>,
        model: impl Into<String>,
        api_key: impl Into<String>,
    ) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: base_url.into(),
            model: model.into(),
            api_key: api_key.into(),
            extra_headers: HeaderMap::new(),
        }
    }

    /// Add an extra header.
    pub fn with_header(mut self, key: &str, value: &str) -> Self {
        if let (Ok(name), Ok(val)) = (
            key.parse::<reqwest::header::HeaderName>(),
            HeaderValue::from_str(value),
        ) {
            self.extra_headers.insert(name, val);
        }
        self
    }

    /// Build the Anthropic wire-format request body.
    fn build_request_body(&self, request: &CompletionRequest) -> serde_json::Value {
        let mut messages = Vec::new();

        for msg in &request.messages {
            match msg {
                // System messages are handled as top-level `system` field, skip here
                Message::System { .. } => {}
                other => messages.push(convert_message_to_anthropic(other)),
            }
        }

        // Anthropic requires max_tokens
        let max_tokens = request.max_tokens.unwrap_or(DEFAULT_MAX_TOKENS);

        let mut body = serde_json::json!({
            "model": self.model,
            "messages": messages,
            "max_tokens": max_tokens,
        });

        // System prompt: top-level field, NOT a message
        // Combine explicit system_prompt with any System messages in the history
        let mut system_parts: Vec<String> = Vec::new();
        if let Some(ref sys) = request.system_prompt {
            system_parts.push(sys.clone());
        }
        for msg in &request.messages {
            if let Message::System { content } = msg {
                system_parts.push(content.clone());
            }
        }
        if !system_parts.is_empty() {
            body["system"] = serde_json::json!(system_parts.join("\n\n"));
        }

        // Tools — Anthropic format: { name, description, input_schema }
        if !request.tools.is_empty() {
            let tools: Vec<serde_json::Value> = request
                .tools
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "name": t.name,
                        "description": t.description,
                        "input_schema": t.parameters,
                    })
                })
                .collect();
            body["tools"] = serde_json::Value::Array(tools);
        }

        if let Some(temp) = request.temperature {
            body["temperature"] = serde_json::json!(temp);
        }

        // Merge additional_params
        if let Some(ref extra) = request.additional_params {
            if let serde_json::Value::Object(map) = extra {
                for (k, v) in map {
                    body[k] = v.clone();
                }
            }
        }

        body
    }
}

impl CompletionProvider for AnthropicProvider {
    fn complete<'a>(
        &'a self,
        request: CompletionRequest,
    ) -> BoxFuture<'a, Result<CompletionResponse, CompletionError>> {
        Box::pin(async move {
            let body = self.build_request_body(&request);
            let url = format!("{}/messages", self.base_url);

            debug!("Anthropic request to {}: {}", url, serde_json::to_string_pretty(&body).unwrap_or_default());

            let mut req = self
                .client
                .post(&url)
                .header(CONTENT_TYPE, "application/json")
                .header("x-api-key", &self.api_key)
                .header("anthropic-version", ANTHROPIC_VERSION);

            for (key, value) in &self.extra_headers {
                req = req.header(key, value);
            }

            let resp = req
                .json(&body)
                .send()
                .await
                .map_err(|e| CompletionError::RequestError(e.to_string()))?;

            let status = resp.status();
            let resp_text = resp
                .text()
                .await
                .map_err(|e| CompletionError::RequestError(e.to_string()))?;

            if !status.is_success() {
                return Err(CompletionError::ProviderError(format!(
                    "HTTP {}: {}",
                    status, resp_text
                )));
            }

            debug!("Anthropic response: {}", resp_text);

            let raw: AnthropicResponse = serde_json::from_str(&resp_text)
                .map_err(|e| CompletionError::ParseError(format!("{}: {}", e, resp_text)))?;

            parse_anthropic_response(raw)
        })
    }
}

// ==========================================
// Wire Format Types
// ==========================================

#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContent>,
    usage: AnthropicUsage,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum AnthropicContent {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    // Thinking/reasoning blocks — we extract text and skip signatures
    #[serde(rename = "thinking")]
    Thinking { thinking: String },
    #[serde(rename = "redacted_thinking")]
    #[allow(dead_code)]
    RedactedThinking { data: String },
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct AnthropicUsage {
    input_tokens: u64,
    output_tokens: u64,
    #[serde(default)]
    cache_read_input_tokens: Option<u64>,
    #[serde(default)]
    cache_creation_input_tokens: Option<u64>,
}

// ==========================================
// Message Conversion
// ==========================================

fn convert_message_to_anthropic(msg: &Message) -> serde_json::Value {
    match msg {
        Message::System { content } => {
            // This shouldn't be called — system messages are handled at the top level.
            // But if it is, convert to a user message to avoid API errors.
            serde_json::json!({
                "role": "user",
                "content": [{ "type": "text", "text": content }],
            })
        }

        Message::User { content } => {
            let parts: Vec<serde_json::Value> = content
                .iter()
                .map(|part| match part {
                    ContentPart::Text { text } => serde_json::json!({
                        "type": "text",
                        "text": text,
                    }),
                    ContentPart::ImageUrl { url, .. } => serde_json::json!({
                        "type": "image",
                        "source": { "type": "url", "url": url },
                    }),
                    ContentPart::ImageBase64 { data, media_type } => serde_json::json!({
                        "type": "image",
                        "source": {
                            "type": "base64",
                            "media_type": media_type,
                            "data": data,
                        },
                    }),
                    // Anthropic doesn't support audio/video — send as text fallback
                    ContentPart::Audio { .. } => serde_json::json!({
                        "type": "text",
                        "text": "[Audio content not supported by this provider]",
                    }),
                    ContentPart::VideoUrl { url } => serde_json::json!({
                        "type": "text",
                        "text": format!("[Video: {}]", url),
                    }),
                    ContentPart::VideoBase64 { .. } => serde_json::json!({
                        "type": "text",
                        "text": "[Video content not supported by this provider]",
                    }),
                    ContentPart::File { data, media_type, .. } => {
                        // Anthropic supports PDF via document blocks
                        if media_type == "application/pdf" {
                            serde_json::json!({
                                "type": "document",
                                "source": {
                                    "type": "base64",
                                    "media_type": media_type,
                                    "data": data,
                                },
                            })
                        } else {
                            serde_json::json!({
                                "type": "text",
                                "text": format!("[File: {}]", media_type),
                            })
                        }
                    }
                })
                .collect();

            serde_json::json!({
                "role": "user",
                "content": parts,
            })
        }

        Message::Assistant { content } => {
            let parts: Vec<serde_json::Value> = content
                .iter()
                .map(|part| match part {
                    AssistantContent::Text { text } => serde_json::json!({
                        "type": "text",
                        "text": text,
                    }),
                    AssistantContent::ToolCall { id, name, arguments } => {
                        // Anthropic expects `input` as a JSON object, not a string
                        let input: serde_json::Value = serde_json::from_str(arguments)
                            .unwrap_or_else(|_| serde_json::json!({ "raw": arguments }));
                        serde_json::json!({
                            "type": "tool_use",
                            "id": id,
                            "name": name,
                            "input": input,
                        })
                    }
                })
                .collect();

            serde_json::json!({
                "role": "assistant",
                "content": parts,
            })
        }

        // Anthropic: tool results are sent as a user message with tool_result content block
        Message::ToolResult { tool_call_id, content } => serde_json::json!({
            "role": "user",
            "content": [{
                "type": "tool_result",
                "tool_use_id": tool_call_id,
                "content": content,
            }],
        }),
    }
}

// ==========================================
// Response Parsing
// ==========================================

fn parse_anthropic_response(raw: AnthropicResponse) -> Result<CompletionResponse, CompletionError> {
    let mut content = Vec::new();

    for block in raw.content {
        match block {
            AnthropicContent::Text { text } => {
                content.push(AssistantContent::Text { text });
            }
            AnthropicContent::ToolUse { id, name, input } => {
                // Anthropic sends input as a JSON object; we serialize to string
                // to match our unified AssistantContent format
                let arguments = serde_json::to_string(&input).unwrap_or_default();
                content.push(AssistantContent::ToolCall { id, name, arguments });
            }
            // Thinking blocks — prepend to text output for transparency
            AnthropicContent::Thinking { thinking } => {
                content.push(AssistantContent::Text {
                    text: format!("<thinking>{}</thinking>", thinking),
                });
            }
            AnthropicContent::RedactedThinking { .. } => {
                // Skip redacted thinking blocks
            }
        }
    }

    let usage = Some(Usage {
        input_tokens: Some(raw.usage.input_tokens),
        output_tokens: Some(raw.usage.output_tokens),
        cached_input_tokens: raw.usage.cache_read_input_tokens,
    });

    Ok(CompletionResponse { content, usage })
}
