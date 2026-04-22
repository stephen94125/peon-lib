//! Telegram-specific output tools for the Peon agent.
//!
//! Each tool clones the `Bot` handle and `ChatId` into the async future,
//! which avoids lifetime conflicts with the `PeonTool` trait's `'_` signature.

use peon_runtime::{BoxFuture, PeonTool, RequestContext, ToolDefinition, ToolError};
use serde_json::json;
use teloxide::{
    prelude::*,
    types::{ChatAction, InlineKeyboardButton, InlineKeyboardMarkup, InputFile, ParseMode},
};

// ==========================================
// SendVoiceTool
// ==========================================

/// Sends an OGG/Opus voice message to the user.
///
/// # Tool Arguments (JSON)
/// ```json
/// { "audio_base64": "<base64 OGG or MP3>", "caption": "optional" }
/// ```
pub struct SendVoiceTool {
    bot: Bot,
    chat_id: ChatId,
}

impl SendVoiceTool {
    pub fn new(bot: Bot, chat_id: ChatId) -> Self {
        Self { bot, chat_id }
    }
}

impl PeonTool for SendVoiceTool {
    fn name(&self) -> &str {
        "send_voice"
    }

    fn definition(&self, _ctx: &RequestContext) -> BoxFuture<'_, ToolDefinition> {
        Box::pin(async {
            ToolDefinition {
                name: "send_voice".into(),
                description: "Send a voice audio message to the user via Telegram. \
                    Pass a base64-encoded OGG or MP3 audio file. \
                    Use when you have synthesized speech or generated audio content."
                    .into(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "audio_base64": {
                            "type": "string",
                            "description": "Base64-encoded audio data (OGG Opus preferred, MP3 also accepted)"
                        },
                        "caption": {
                            "type": "string",
                            "description": "Optional caption below the voice message"
                        }
                    },
                    "required": ["audio_base64"]
                }),
            }
        })
    }

    fn call(&self, args: &str, _ctx: &RequestContext) -> BoxFuture<'_, Result<String, ToolError>> {
        let args = args.to_string();
        let bot = self.bot.clone();
        let chat_id = self.chat_id;
        Box::pin(async move {
            let parsed: serde_json::Value =
                serde_json::from_str(&args).map_err(|e| ToolError::InvalidArgs(e.to_string()))?;

            let b64 = parsed["audio_base64"]
                .as_str()
                .ok_or_else(|| ToolError::InvalidArgs("missing audio_base64".into()))?;

            let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64)
                .map_err(|e| ToolError::CallError(format!("base64 decode: {}", e)))?;

            let caption = parsed["caption"].as_str().map(|s| s.to_string());
            let file = InputFile::memory(bytes);
            let mut req = bot.send_voice(chat_id, file);
            if let Some(cap) = caption {
                req = req.caption(cap);
            }
            req.await
                .map_err(|e| ToolError::CallError(format!("Telegram error: {}", e)))?;

            Ok("Voice message sent.".into())
        })
    }
}

// ==========================================
// SendCsvTool
// ==========================================

/// Sends structured data as a downloadable CSV file.
///
/// # Tool Arguments (JSON)
/// ```json
/// {
///   "filename": "report.csv",
///   "rows": [{"col1": "val1", "col2": "val2"}],
///   "caption": "optional"
/// }
/// ```
/// Column order follows the keys of the **first row object**.
pub struct SendCsvTool {
    bot: Bot,
    chat_id: ChatId,
}

impl SendCsvTool {
    pub fn new(bot: Bot, chat_id: ChatId) -> Self {
        Self { bot, chat_id }
    }
}

impl PeonTool for SendCsvTool {
    fn name(&self) -> &str {
        "send_csv"
    }

    fn definition(&self, _ctx: &RequestContext) -> BoxFuture<'_, ToolDefinition> {
        Box::pin(async {
            ToolDefinition {
                name: "send_csv".into(),
                description: "Send structured tabular data as a downloadable CSV file. \
                    Provide rows as a JSON array of objects. \
                    Column headers come from the keys of the first object. \
                    Use for reports, rankings, query results, or any tabular output."
                    .into(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "filename": {
                            "type": "string",
                            "description": "Attachment filename (e.g. 'report.csv'). Must end with .csv"
                        },
                        "rows": {
                            "type": "array",
                            "items": { "type": "object" },
                            "description": "Array of JSON objects. Keys of the first object become column headers."
                        },
                        "caption": {
                            "type": "string",
                            "description": "Optional caption below the file"
                        }
                    },
                    "required": ["filename", "rows"]
                }),
            }
        })
    }

    fn call(&self, args: &str, _ctx: &RequestContext) -> BoxFuture<'_, Result<String, ToolError>> {
        let args = args.to_string();
        let bot = self.bot.clone();
        let chat_id = self.chat_id;
        Box::pin(async move {
            let parsed: serde_json::Value =
                serde_json::from_str(&args).map_err(|e| ToolError::InvalidArgs(e.to_string()))?;

            let filename = parsed["filename"]
                .as_str()
                .unwrap_or("output.csv")
                .to_string();

            let rows = parsed["rows"]
                .as_array()
                .ok_or_else(|| ToolError::InvalidArgs("'rows' must be a JSON array".into()))?;

            if rows.is_empty() {
                return Err(ToolError::InvalidArgs("'rows' array is empty".into()));
            }

            let headers: Vec<String> = rows[0]
                .as_object()
                .ok_or_else(|| ToolError::InvalidArgs("each row must be a JSON object".into()))?
                .keys()
                .cloned()
                .collect();

            let mut csv_bytes = Vec::new();
            {
                let mut writer = csv::Writer::from_writer(&mut csv_bytes);
                writer
                    .write_record(&headers)
                    .map_err(|e| ToolError::CallError(format!("CSV header: {}", e)))?;
                for row in rows {
                    let obj = row
                        .as_object()
                        .ok_or_else(|| ToolError::InvalidArgs("row must be object".into()))?;
                    let record: Vec<String> = headers
                        .iter()
                        .map(|h| {
                            obj.get(h)
                                .map(|v| match v {
                                    serde_json::Value::String(s) => s.clone(),
                                    other => other.to_string(),
                                })
                                .unwrap_or_default()
                        })
                        .collect();
                    writer
                        .write_record(&record)
                        .map_err(|e| ToolError::CallError(format!("CSV row: {}", e)))?;
                }
                writer
                    .flush()
                    .map_err(|e| ToolError::CallError(format!("CSV flush: {}", e)))?;
            }

            let caption = parsed["caption"].as_str().map(|s| s.to_string());
            let row_count = rows.len();
            let col_count = headers.len();

            let file = InputFile::memory(csv_bytes).file_name(filename.clone());
            let mut req = bot.send_document(chat_id, file);
            if let Some(cap) = caption {
                req = req.caption(cap);
            }
            req.await
                .map_err(|e| ToolError::CallError(format!("Telegram error: {}", e)))?;

            Ok(format!(
                "CSV '{}' sent ({} rows × {} columns).",
                filename, row_count, col_count
            ))
        })
    }
}

// ==========================================
// SendInlineKeyboardTool
// ==========================================

/// Sends a message with interactive inline keyboard buttons.
///
/// # Tool Arguments (JSON)
/// ```json
/// {
///   "message": "Choose an option:",
///   "buttons": [
///     [{"text": "✅ Yes", "callback_data": "yes"},
///      {"text": "❌ No",  "callback_data": "no"}]
///   ]
/// }
/// ```
///
/// - `buttons` is a **2D array**: outer = rows, inner = buttons per row.
/// - `callback_data` ≤ 64 bytes (Telegram hard limit).
/// - 2–3 buttons per row recommended for mobile.
pub struct SendInlineKeyboardTool {
    bot: Bot,
    chat_id: ChatId,
}

impl SendInlineKeyboardTool {
    pub fn new(bot: Bot, chat_id: ChatId) -> Self {
        Self { bot, chat_id }
    }
}

impl PeonTool for SendInlineKeyboardTool {
    fn name(&self) -> &str {
        "send_inline_keyboard"
    }

    fn definition(&self, _ctx: &RequestContext) -> BoxFuture<'_, ToolDefinition> {
        Box::pin(async {
            ToolDefinition {
                name: "send_inline_keyboard".into(),
                description:
                    "Send a message with clickable inline keyboard buttons. \
                    Use for Human-in-the-loop confirmations or multi-choice menus. \
                    \n\nLayout: `buttons` is a 2D array — outer = rows, inner = columns. \
                    Each button has `text` (label) and `callback_data` (opaque string ≤64 bytes returned on click). \
                    Keep rows to 2-3 buttons for mobile. \
                    \nExample: [[{\"text\":\"✅ Yes\",\"callback_data\":\"yes\"},{\"text\":\"❌ No\",\"callback_data\":\"no\"}]]"
                    .into(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "message": {
                            "type": "string",
                            "description": "Message shown above the keyboard. Supports MarkdownV2."
                        },
                        "buttons": {
                            "type": "array",
                            "items": {
                                "type": "array",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "text": { "type": "string" },
                                        "callback_data": { "type": "string", "description": "Max 64 bytes" }
                                    },
                                    "required": ["text", "callback_data"]
                                }
                            },
                            "description": "2D array: [[{text, callback_data}, ...], ...]"
                        }
                    },
                    "required": ["message", "buttons"]
                }),
            }
        })
    }

    fn call(&self, args: &str, _ctx: &RequestContext) -> BoxFuture<'_, Result<String, ToolError>> {
        let args = args.to_string();
        let bot = self.bot.clone();
        let chat_id = self.chat_id;
        Box::pin(async move {
            let parsed: serde_json::Value =
                serde_json::from_str(&args).map_err(|e| ToolError::InvalidArgs(e.to_string()))?;

            let message = parsed["message"]
                .as_str()
                .ok_or_else(|| ToolError::InvalidArgs("missing 'message'".into()))?
                .to_string();

            let button_rows = parsed["buttons"]
                .as_array()
                .ok_or_else(|| ToolError::InvalidArgs("'buttons' must be a 2D array".into()))?;

            let mut keyboard: Vec<Vec<InlineKeyboardButton>> = Vec::new();
            for row in button_rows {
                let cols = row
                    .as_array()
                    .ok_or_else(|| ToolError::InvalidArgs("each row must be an array".into()))?;
                let mut btn_row = Vec::new();
                for btn in cols {
                    let text = btn["text"]
                        .as_str()
                        .ok_or_else(|| ToolError::InvalidArgs("button missing 'text'".into()))?
                        .to_string();
                    let data = btn["callback_data"]
                        .as_str()
                        .ok_or_else(|| {
                            ToolError::InvalidArgs("button missing 'callback_data'".into())
                        })?
                        .to_string();
                    if data.len() > 64 {
                        return Err(ToolError::InvalidArgs(format!(
                            "callback_data '{}' exceeds 64-byte Telegram limit",
                            data
                        )));
                    }
                    btn_row.push(InlineKeyboardButton::callback(text, data));
                }
                keyboard.push(btn_row);
            }

            bot.send_message(chat_id, &message)
                .parse_mode(ParseMode::MarkdownV2)
                .reply_markup(InlineKeyboardMarkup::new(keyboard))
                .await
                .map_err(|e| ToolError::CallError(format!("Telegram error: {}", e)))?;

            Ok("Inline keyboard sent.".into())
        })
    }
}

// ==========================================
// SendChatActionTool
// ==========================================

/// Shows a temporary non-blocking status indicator.
///
/// Fires and forgets — lasts ~5 seconds. Use before long operations.
///
/// # Tool Arguments (JSON)
/// ```json
/// { "action": "typing" }
/// ```
///
/// | `action`          | User sees                |
/// |-------------------|--------------------------|
/// | `typing`          | "Typing…"                |
/// | `upload_voice`    | "Sending voice…"         |
/// | `upload_document` | "Sending document…"      |
/// | `upload_photo`    | "Sending photo…"         |
/// | `upload_video`    | "Sending video…"         |
/// | `find_location`   | "Sending location…"      |
pub struct SendChatActionTool {
    bot: Bot,
    chat_id: ChatId,
}

impl SendChatActionTool {
    pub fn new(bot: Bot, chat_id: ChatId) -> Self {
        Self { bot, chat_id }
    }
}

impl PeonTool for SendChatActionTool {
    fn name(&self) -> &str {
        "send_chat_action"
    }

    fn definition(&self, _ctx: &RequestContext) -> BoxFuture<'_, ToolDefinition> {
        Box::pin(async {
            ToolDefinition {
                name: "send_chat_action".into(),
                description:
                    "Show a temporary non-blocking status indicator (fires and forgets). \
                    Call BEFORE a long operation so the user knows the bot is working. \
                    Disappears after ~5 seconds automatically. \
                    Supported: typing, upload_voice, upload_document, upload_photo, upload_video, find_location."
                    .into(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "action": {
                            "type": "string",
                            "enum": [
                                "typing", "upload_voice", "upload_document",
                                "upload_photo", "upload_video", "find_location"
                            ]
                        }
                    },
                    "required": ["action"]
                }),
            }
        })
    }

    fn call(&self, args: &str, _ctx: &RequestContext) -> BoxFuture<'_, Result<String, ToolError>> {
        let args = args.to_string();
        let bot = self.bot.clone();
        let chat_id = self.chat_id;
        Box::pin(async move {
            let parsed: serde_json::Value =
                serde_json::from_str(&args).map_err(|e| ToolError::InvalidArgs(e.to_string()))?;

            let action_str = parsed["action"].as_str().unwrap_or("typing");
            let action = match action_str {
                "typing" => ChatAction::Typing,
                "upload_voice" => ChatAction::UploadVoice,
                "upload_document" => ChatAction::UploadDocument,
                "upload_photo" => ChatAction::UploadPhoto,
                "upload_video" => ChatAction::UploadVideo,
                "find_location" => ChatAction::FindLocation,
                other => {
                    return Err(ToolError::InvalidArgs(format!(
                        "Unknown action '{}'. Supported: typing, upload_voice, upload_document, upload_photo, upload_video, find_location",
                        other
                    )));
                }
            };

            // Fire and forget — non-blocking
            let _ = bot.send_chat_action(chat_id, action).await;
            Ok(format!("Chat action '{}' sent.", action_str))
        })
    }
}
