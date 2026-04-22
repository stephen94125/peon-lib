//! OpenAI-compatible provider implementation.
//!
//! Supports OpenAI, OpenRouter, and any OpenAI-compatible API (Azure, Groq, etc.)
//! via a configurable base URL.
//!
//! # Provider Quirks Handled
//!
//! - **Tool call arguments**: OpenAI sometimes returns `arguments` as a JSON string,
//!   sometimes as a JSON object. We handle both via `deserialize_maybe_stringified`.
//! - **System prompt**: OpenAI uses `role: "system"` (or `"developer"` for newer models).
//!   We always send `"system"`.
//! - **Tool results**: OpenAI expects `role: "tool"` with `tool_call_id` referencing the
//!   original call's `id`. Some models break if tool results use the array content format
//!   vs string — we default to string.
//! - **Single text content optimization**: When a user message has only one text part,
//!   we send `content: "string"` instead of `content: [{"type":"text","text":"..."}]`
//!   to save tokens and avoid issues with older models.
//! - **Empty tool_calls array**: Some models return `tool_calls: null` or `tool_calls: []`.
//!   We normalize both to "no tool calls".
//! - **OpenRouter extras**: The `X-Title` header and `provider` field in `additional_params`
//!   are passed through when present.

use crate::error::CompletionError;
use crate::message::{AssistantContent, ContentPart, Message};
use crate::provider::{
    BoxFuture, CompletionProvider, CompletionRequest, CompletionResponse, Usage,
};
use log::debug;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use serde::Deserialize;

/// An OpenAI-compatible completion provider.
///
/// Works out of the box with:
/// - OpenAI (`https://api.openai.com/v1`)
/// - OpenRouter (`https://openrouter.ai/api/v1`)
/// - Azure OpenAI (custom base URL)
/// - Any OpenAI-compatible endpoint (Groq, Together, local vLLM, etc.)
///
/// # Example
///
/// ```rust,ignore
/// use peon_runtime::providers::openai::OpenAiProvider;
///
/// // Standard OpenAI
/// let provider = OpenAiProvider::new("gpt-4o", &api_key);
///
/// // OpenRouter
/// let provider = OpenAiProvider::openrouter("anthropic/claude-3.7-sonnet", &api_key);
///
/// // Custom endpoint
/// let provider = OpenAiProvider::custom("http://localhost:8080/v1", "my-model", &api_key);
/// ```
#[derive(Clone)]
pub struct OpenAiProvider {
    client: reqwest::Client,
    base_url: String,
    model: String,
    api_key: String,
    /// Extra headers (e.g., OpenRouter's X-Title, Helicone logging, etc.)
    extra_headers: HeaderMap,
}

impl OpenAiProvider {
    /// Create a provider for OpenAI's API.
    pub fn new(model: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: "https://api.openai.com/v1".into(),
            model: model.into(),
            api_key: api_key.into(),
            extra_headers: HeaderMap::new(),
        }
    }

    /// Create a provider for OpenRouter.
    pub fn openrouter(model: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: "https://openrouter.ai/api/v1".into(),
            model: model.into(),
            api_key: api_key.into(),
            extra_headers: HeaderMap::new(),
        }
    }

    /// Create a provider with a custom base URL (Azure, vLLM, etc.)
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

    /// Add an extra header (e.g., `X-Title` for OpenRouter).
    pub fn with_header(mut self, key: &str, value: &str) -> Self {
        if let (Ok(name), Ok(val)) = (
            key.parse::<reqwest::header::HeaderName>(),
            HeaderValue::from_str(value),
        ) {
            self.extra_headers.insert(name, val);
        }
        self
    }

    /// Build the wire-format request body.
    fn build_request_body(&self, request: &CompletionRequest) -> serde_json::Value {
        let mut messages = Vec::new();

        // System prompt
        if let Some(ref system) = request.system_prompt {
            messages.push(serde_json::json!({
                "role": "system",
                "content": system,
            }));
        }

        // Chat history + prompt
        for msg in &request.messages {
            messages.push(convert_message_to_openai(msg));
        }

        let mut body = serde_json::json!({
            "model": self.model,
            "messages": messages,
        });

        // Tools
        if !request.tools.is_empty() {
            let tools: Vec<serde_json::Value> = request
                .tools
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "type": "function",
                        "function": {
                            "name": t.name,
                            "description": t.description,
                            "parameters": t.parameters,
                        }
                    })
                })
                .collect();
            body["tools"] = serde_json::Value::Array(tools);
        }

        // Optional params
        if let Some(temp) = request.temperature {
            body["temperature"] = serde_json::json!(temp);
        }
        if let Some(max) = request.max_tokens {
            body["max_tokens"] = serde_json::json!(max);
        }

        // Merge additional_params at the top level
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

impl CompletionProvider for OpenAiProvider {
    fn complete<'a>(
        &'a self,
        request: CompletionRequest,
    ) -> BoxFuture<'a, Result<CompletionResponse, CompletionError>> {
        Box::pin(async move {
            let body = self.build_request_body(&request);
            let url = format!("{}/chat/completions", self.base_url);

            debug!(
                "OpenAI request to {}: {}",
                url,
                serde_json::to_string_pretty(&body).unwrap_or_default()
            );

            let mut req = self
                .client
                .post(&url)
                .header(CONTENT_TYPE, "application/json")
                .header(AUTHORIZATION, format!("Bearer {}", self.api_key));

            // Attach extra headers
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

            debug!("OpenAI response: {}", resp_text);

            let raw: OpenAiResponse = serde_json::from_str(&resp_text)
                .map_err(|e| CompletionError::ParseError(format!("{}: {}", e, resp_text)))?;

            parse_openai_response(raw)
        })
    }
}

// ==========================================
// Wire Format Types
// ==========================================

#[derive(Debug, Deserialize)]
struct OpenAiResponse {
    choices: Vec<OpenAiChoice>,
    usage: Option<OpenAiUsage>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChoice {
    message: OpenAiMessage,
}

#[derive(Debug, Deserialize)]
struct OpenAiMessage {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<OpenAiToolCall>>,
    #[serde(default)]
    phase: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct OpenAiToolCall {
    id: String,
    function: OpenAiFunction,
}

#[derive(Debug, Deserialize)]
struct OpenAiFunction {
    name: String,
    /// Arguments can be a JSON string or a JSON object depending on the provider.
    /// We always receive it as a raw Value and normalize later.
    arguments: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct OpenAiUsage {
    prompt_tokens: Option<u64>,
    completion_tokens: Option<u64>,
    #[serde(default)]
    prompt_tokens_details: Option<PromptTokensDetails>,
}

#[derive(Debug, Deserialize)]
struct PromptTokensDetails {
    #[serde(default)]
    cached_tokens: Option<u64>,
}

// ==========================================
// Message Conversion (Our types → OpenAI wire format)
// ==========================================

fn convert_message_to_openai(msg: &Message) -> serde_json::Value {
    match msg {
        Message::System { content } => serde_json::json!({
            "role": "system",
            "content": content,
        }),

        Message::User { content } => {
            // Optimization: single text part → send as string (saves tokens)
            if content.len() == 1 {
                if let ContentPart::Text { text } = &content[0] {
                    return serde_json::json!({
                        "role": "user",
                        "content": text,
                    });
                }
            }

            let parts: Vec<serde_json::Value> = content
                .iter()
                .map(|part| match part {
                    ContentPart::Text { text } => serde_json::json!({
                        "type": "text",
                        "text": text,
                    }),
                    ContentPart::ImageUrl { url, detail } => {
                        let mut img = serde_json::json!({ "url": url });
                        if let Some(d) = detail {
                            img["detail"] = serde_json::json!(d);
                        }
                        serde_json::json!({
                            "type": "image_url",
                            "image_url": img,
                        })
                    }
                    ContentPart::ImageBase64 { data, media_type } => {
                        let url = format!("data:{};base64,{}", media_type, data);
                        serde_json::json!({
                            "type": "image_url",
                            "image_url": { "url": url },
                        })
                    }
                    ContentPart::Audio { data, format } => serde_json::json!({
                        "type": "input_audio",
                        "input_audio": { "data": data, "format": format },
                    }),
                    ContentPart::VideoUrl { url } => serde_json::json!({
                        "type": "video_url",
                        "video_url": { "url": url },
                    }),
                    ContentPart::VideoBase64 { data, media_type } => {
                        let url = format!("data:{};base64,{}", media_type, data);
                        serde_json::json!({
                            "type": "video_url",
                            "video_url": { "url": url },
                        })
                    }
                    ContentPart::File {
                        data,
                        media_type,
                        filename,
                    } => {
                        let url = format!("data:{};base64,{}", media_type, data);
                        let mut file = serde_json::json!({ "url": url });
                        if let Some(f) = filename {
                            file["filename"] = serde_json::json!(f);
                        }
                        serde_json::json!({
                            "type": "file",
                            "file": file,
                        })
                    }
                })
                .collect();

            serde_json::json!({
                "role": "user",
                "content": parts,
            })
        }

        Message::Assistant { content } => {
            let mut text_parts = Vec::new();
            let mut tool_calls = Vec::new();
            let mut phase: Option<serde_json::Value> = None;

            for part in content {
                match part {
                    AssistantContent::Text { text } => text_parts.push(text.clone()),
                    AssistantContent::ToolCall {
                        id,
                        name,
                        arguments,
                    } => {
                        let actual_id = if let Ok(meta) = serde_json::from_str::<serde_json::Value>(id) {
                            if let Some(p) = meta.get("phase") {
                                phase = Some(p.clone());
                            }
                            meta.get("id").and_then(|v| v.as_str()).unwrap_or(id).to_string()
                        } else {
                            id.clone()
                        };

                        tool_calls.push(serde_json::json!({
                            "id": actual_id,
                            "type": "function",
                            "function": {
                                "name": name,
                                "arguments": arguments,
                            }
                        }));
                    }
                }
            }

            let mut msg = serde_json::json!({ "role": "assistant" });
            if !text_parts.is_empty() {
                msg["content"] = serde_json::json!(text_parts.join("\n"));
            }
            if !tool_calls.is_empty() {
                msg["tool_calls"] = serde_json::Value::Array(tool_calls);
            }
            if let Some(p) = phase {
                msg["phase"] = p;
            }
            msg
        }

        Message::ToolResult {
            tool_call_id,
            content,
        } => {
            let actual_id = if let Ok(meta) = serde_json::from_str::<serde_json::Value>(tool_call_id) {
                meta.get("id").and_then(|v| v.as_str()).unwrap_or(tool_call_id).to_string()
            } else {
                tool_call_id.clone()
            };

            serde_json::json!({
                "role": "tool",
                "tool_call_id": actual_id,
                "content": content,
            })
        }
    }
}

// ==========================================
// Response Parsing (OpenAI wire format → Our types)
// ==========================================

fn parse_openai_response(raw: OpenAiResponse) -> Result<CompletionResponse, CompletionError> {
    let choice = raw
        .choices
        .into_iter()
        .next()
        .ok_or_else(|| CompletionError::ParseError("Response contained no choices".into()))?;

    let mut content = Vec::new();

    // Text content
    if let Some(text) = choice.message.content {
        if !text.is_empty() {
            content.push(AssistantContent::Text { text });
        }
    }

    // Tool calls
    if let Some(tool_calls) = choice.message.tool_calls {
        let phase = choice.message.phase;
        let mut first = true;

        for tc in tool_calls {
            let arguments = match &tc.function.arguments {
                serde_json::Value::String(s) => s.clone(),
                other => serde_json::to_string(other).unwrap_or_default(),
            };

            let mut meta = serde_json::json!({
                "id": tc.id,
            });

            if first {
                if let Some(p) = &phase {
                    meta["phase"] = p.clone();
                }
                first = false;
            }

            let packed_id = serde_json::to_string(&meta).unwrap_or_default();

            content.push(AssistantContent::ToolCall {
                id: packed_id,
                name: tc.function.name,
                arguments,
            });
        }
    }

    if content.is_empty() {
        return Err(CompletionError::ParseError(
            "Response contained neither text nor tool calls".into(),
        ));
    }

    let usage = raw.usage.map(|u| Usage {
        input_tokens: u.prompt_tokens,
        output_tokens: u.completion_tokens,
        cached_input_tokens: u.prompt_tokens_details.and_then(|d| d.cached_tokens),
    });

    Ok(CompletionResponse { content, usage })
}
