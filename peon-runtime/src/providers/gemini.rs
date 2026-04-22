//! Google Gemini provider implementation.
//!
//! # API Differences from OpenAI
//!
//! Gemini's API is the most divergent of all major providers:
//!
//! - **Endpoint**: `POST /v1beta/models/{model}:generateContent` (not `/chat/completions`)
//! - **Auth**: API key as query param `?key=...` (not Authorization header)
//! - **System prompt**: Top-level `systemInstruction` field containing `parts`
//! - **Messages**: Called `contents`, with `role: "user"` or `role: "model"` (not `assistant`)
//! - **Content structure**: Each message has `parts` (not `content`). Each part is
//!   `{"text": "..."}` or `{"inlineData": {"mimeType": "...", "data": "..."}}` or
//!   `{"functionCall": {...}}` / `{"functionResponse": {...}}`
//! - **Tool definitions**: Wrapped in `tools: [{ functionDeclarations: [...] }]`,
//!   with `description` and `parameters` (a `Schema` object, not raw JSON Schema).
//! - **Tool responses**: Use `functionResponse` parts (not a separate `tool` role)
//! - **Max tokens**: Field is `maxOutputTokens` inside `generationConfig`
//! - **Temperature**: Inside `generationConfig`
//! - **Empty parameters**: If a tool has no parameters, omit the field entirely
//!   (sending `{"type":"object","properties":{}}` causes errors on some models)

use crate::error::CompletionError;
use crate::message::{AssistantContent, ContentPart, Message};
use crate::provider::{
    BoxFuture, CompletionProvider, CompletionRequest, CompletionResponse, Usage,
};
use log::debug;
use serde::Deserialize;

const GEMINI_API_URL: &str = "https://generativelanguage.googleapis.com";

/// Google Gemini completion provider.
///
/// # Example
///
/// ```rust,ignore
/// use peon_runtime::providers::gemini::GeminiProvider;
///
/// let provider = GeminiProvider::new("gemini-2.5-flash", &api_key);
/// ```
#[derive(Clone)]
pub struct GeminiProvider {
    client: reqwest::Client,
    base_url: String,
    model: String,
    api_key: String,
}

impl GeminiProvider {
    /// Create a provider for Google's Gemini API.
    pub fn new(model: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: GEMINI_API_URL.into(),
            model: model.into(),
            api_key: api_key.into(),
        }
    }

    /// Create a provider with a custom base URL.
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
        }
    }

    /// Build the Gemini wire-format request body.
    fn build_request_body(&self, request: &CompletionRequest) -> serde_json::Value {
        let mut contents = Vec::new();

        for msg in &request.messages {
            match msg {
                Message::System { .. } => {} // handled as systemInstruction
                other => {
                    if let Some(c) = convert_message_to_gemini(other) {
                        contents.push(c);
                    }
                }
            }
        }

        let mut body = serde_json::json!({
            "contents": contents,
        });

        // System prompt → systemInstruction
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
            body["systemInstruction"] = serde_json::json!({
                "parts": [{ "text": system_parts.join("\n\n") }],
            });
        }

        // Tools → functionDeclarations
        if !request.tools.is_empty() {
            let declarations: Vec<serde_json::Value> = request
                .tools
                .iter()
                .map(|t| {
                    let mut decl = serde_json::json!({
                        "name": t.name,
                        "description": t.description,
                    });
                    // Gemini quirk: omit empty parameters entirely
                    let is_empty = t.parameters
                        == serde_json::json!({"type": "object", "properties": {}})
                        || t.parameters == serde_json::json!({});
                    if !is_empty {
                        decl["parameters"] = t.parameters.clone();
                    }
                    decl
                })
                .collect();

            body["tools"] = serde_json::json!([{
                "functionDeclarations": declarations,
            }]);
        }

        // generationConfig
        let mut gen_config = serde_json::Map::new();
        if let Some(temp) = request.temperature {
            gen_config.insert("temperature".into(), serde_json::json!(temp));
        }
        if let Some(max) = request.max_tokens {
            gen_config.insert("maxOutputTokens".into(), serde_json::json!(max));
        }
        if !gen_config.is_empty() {
            body["generationConfig"] = serde_json::Value::Object(gen_config);
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

impl CompletionProvider for GeminiProvider {
    fn complete<'a>(
        &'a self,
        request: CompletionRequest,
    ) -> BoxFuture<'a, Result<CompletionResponse, CompletionError>> {
        Box::pin(async move {
            let body = self.build_request_body(&request);
            let url = format!(
                "{}/v1beta/models/{}:generateContent?key={}",
                self.base_url, self.model, self.api_key
            );

            debug!(
                "Gemini request: {}",
                serde_json::to_string_pretty(&body).unwrap_or_default()
            );

            let resp = self
                .client
                .post(&url)
                .header("Content-Type", "application/json")
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

            debug!("Gemini response: {}", resp_text);

            let raw: GeminiResponse = serde_json::from_str(&resp_text)
                .map_err(|e| CompletionError::ParseError(format!("{}: {}", e, resp_text)))?;

            parse_gemini_response(raw)
        })
    }
}

// ==========================================
// Wire Format Types
// ==========================================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiResponse {
    candidates: Vec<GeminiCandidate>,
    usage_metadata: Option<GeminiUsage>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiCandidate {
    content: Option<GeminiContent>,
    finish_reason: Option<String>,
    finish_message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GeminiContent {
    #[serde(default)]
    parts: Vec<GeminiPart>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct GeminiPart {
    // Text part
    #[serde(default)]
    text: Option<String>,
    // Function call
    #[serde(default)]
    function_call: Option<serde_json::Value>,
    // Function response (for history)
    #[serde(default)]
    function_response: Option<GeminiFunctionResponse>,
    // Inline data (images etc.)
    #[serde(default)]
    inline_data: Option<GeminiInlineData>,
    // Whether this is a thought/thinking part
    #[serde(default)]
    thought: Option<bool>,
    // Opaque signature for the thought, required by Gemini 2.0+
    #[serde(default, rename = "thoughtSignature")]
    thought_signature: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct GeminiFunctionResponse {
    name: String,
    response: serde_json::Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct GeminiInlineData {
    mime_type: String,
    data: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct GeminiUsage {
    prompt_token_count: Option<u64>,
    candidates_token_count: Option<u64>,
    total_token_count: Option<u64>,
}

// ==========================================
// Message Conversion
// ==========================================

fn convert_message_to_gemini(msg: &Message) -> Option<serde_json::Value> {
    match msg {
        Message::System { .. } => None, // handled at top level

        Message::User { content } => {
            let parts: Vec<serde_json::Value> = content
                .iter()
                .map(|part| match part {
                    ContentPart::Text { text } => serde_json::json!({ "text": text }),
                    ContentPart::ImageUrl { url, .. } => {
                        // Gemini supports fileData for URLs
                        serde_json::json!({
                            "fileData": {
                                "fileUri": url,
                            }
                        })
                    }
                    ContentPart::ImageBase64 { data, media_type } => serde_json::json!({
                        "inlineData": {
                            "mimeType": media_type,
                            "data": data,
                        }
                    }),
                    ContentPart::Audio { data, format } => {
                        let mime = match format.as_str() {
                            "wav" => "audio/wav",
                            "mp3" => "audio/mp3",
                            "ogg" => "audio/ogg",
                            "flac" => "audio/flac",
                            other => other,
                        };
                        serde_json::json!({
                            "inlineData": {
                                "mimeType": mime,
                                "data": data,
                            }
                        })
                    }
                    ContentPart::VideoUrl { url } => serde_json::json!({
                        "fileData": {
                            "fileUri": url,
                        }
                    }),
                    ContentPart::VideoBase64 { data, media_type } => serde_json::json!({
                        "inlineData": {
                            "mimeType": media_type,
                            "data": data,
                        }
                    }),
                    ContentPart::File {
                        data, media_type, ..
                    } => serde_json::json!({
                        "inlineData": {
                            "mimeType": media_type,
                            "data": data,
                        }
                    }),
                })
                .collect();

            Some(serde_json::json!({
                "role": "user",
                "parts": parts,
            }))
        }

        Message::Assistant { content } => {
            let parts: Vec<serde_json::Value> = content
                .iter()
                .map(|part| match part {
                    AssistantContent::Text { text } => serde_json::json!({ "text": text }),
                    AssistantContent::ToolCall {
                        id: packed_id,
                        name,
                        arguments,
                        ..
                    } => {
                        let mut part_json = serde_json::json!({});
                        if let Ok(meta) = serde_json::from_str::<serde_json::Value>(packed_id) {
                            if let Some(fc) = meta.get("functionCall") {
                                part_json["functionCall"] = fc.clone();
                            } else {
                                let args: serde_json::Value = serde_json::from_str(arguments)
                                    .unwrap_or_else(|_| serde_json::json!({}));
                                part_json["functionCall"] = serde_json::json!({
                                    "name": name,
                                    "args": args,
                                });
                            }
                            if let Some(ts) = meta.get("thoughtSignature") {
                                part_json["thoughtSignature"] = ts.clone();
                            }
                        } else {
                            let args: serde_json::Value = serde_json::from_str(arguments)
                                .unwrap_or_else(|_| serde_json::json!({}));
                            part_json["functionCall"] = serde_json::json!({
                                "name": name,
                                "args": args,
                            });
                        }
                        part_json
                    }
                })
                .collect();

            Some(serde_json::json!({
                "role": "model",
                "parts": parts,
            }))
        }

        // Gemini: tool results are sent as user messages with functionResponse parts
        Message::ToolResult {
            tool_call_id,
            content,
        } => {
            let response_value: serde_json::Value = serde_json::from_str(content)
                .unwrap_or_else(|_| serde_json::json!({ "result": content }));

            let mut function_response = serde_json::json!({
                "response": response_value,
            });

            if let Ok(meta) = serde_json::from_str::<serde_json::Value>(tool_call_id) {
                if let Some(fc) = meta.get("functionCall") {
                    if let Some(name) = fc.get("name").and_then(|v| v.as_str()) {
                        function_response["name"] = serde_json::json!(name);
                    } else {
                        function_response["name"] = serde_json::json!(tool_call_id);
                    }
                    if let Some(fid) = fc.get("id").filter(|v| !v.is_null()) {
                        function_response["id"] = fid.clone();
                    }
                } else {
                    function_response["name"] = serde_json::json!(tool_call_id);
                }
            } else {
                function_response["name"] = serde_json::json!(tool_call_id);
            }

            Some(serde_json::json!({
                "role": "user",
                "parts": [{
                    "functionResponse": function_response
                }],
            }))
        }
    }
}

// ==========================================
// Response Parsing
// ==========================================

fn parse_gemini_response(raw: GeminiResponse) -> Result<CompletionResponse, CompletionError> {
    let candidate = raw
        .candidates
        .into_iter()
        .next()
        .ok_or_else(|| CompletionError::ParseError("No response candidates".into()))?;

    let gemini_content = candidate.content.ok_or_else(|| {
        let reason = candidate.finish_reason.as_deref().unwrap_or("unknown");
        let message = candidate.finish_message.as_deref().unwrap_or("no message");
        CompletionError::ParseError(format!(
            "Gemini candidate missing content (finish_reason={}, message={})",
            reason, message
        ))
    })?;

    let mut content = Vec::new();

    for part in gemini_content.parts {
        // Skip thinking/reasoning parts
        if part.thought.unwrap_or(false) {
            if let Some(text) = part.text {
                content.push(AssistantContent::Text {
                    text: format!("<thinking>{}</thinking>", text),
                });
            }
            continue;
        }

        if let Some(text) = part.text {
            if !text.is_empty() {
                content.push(AssistantContent::Text { text });
            }
        } else if let Some(fc) = part.function_call {
            let name = fc
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let empty_args = serde_json::json!({});
            let args = fc.get("args").unwrap_or(&empty_args);
            let arguments = serde_json::to_string(args).unwrap_or_default();

            // Encode the entire functionCall json object into the id field so we can perfectly reconstruct it
            let mut meta = serde_json::json!({
                "functionCall": fc,
            });
            if let Some(ts) = part.thought_signature {
                meta["thoughtSignature"] = serde_json::json!(ts);
            }
            let packed_id = serde_json::to_string(&meta).unwrap_or_default();

            content.push(AssistantContent::ToolCall {
                id: packed_id,
                name,
                arguments,
            });
        }
        // inline_data and function_response are for input, not output
    }

    if content.is_empty() {
        return Err(CompletionError::ParseError(
            "Gemini response contained no text or function calls".into(),
        ));
    }

    let usage = raw.usage_metadata.map(|u| Usage {
        input_tokens: u.prompt_token_count,
        output_tokens: u.candidates_token_count,
        cached_input_tokens: None,
    });

    Ok(CompletionResponse { content, usage })
}
