use line_bot_sdk_rust::line_messaging_api::models::{
    AudioMessage, ImageMessage, LocationMessage, Message, StickerMessage, VideoMessage,
};
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

/// A thread-safe queue holding LINE messages to be batched into a single reply_message API call.
/// LINE allows up to 5 messages per reply token.
pub type SharedMessageQueue = Arc<Mutex<Vec<Message>>>;

// ==========================================
// Tool Error
// ==========================================
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct ToolCallError(String);

impl ToolCallError {
    pub fn new(msg: impl Into<String>) -> Self {
        Self(msg.into())
    }
}

// ==========================================
// 1. SEND IMAGE TOOL
// ==========================================

#[derive(Deserialize)]
pub struct SendImageArgs {
    original_content_url: String,
    preview_image_url: String,
}

#[derive(Serialize)]
pub struct ToolStatusResult {
    status: String,
}

pub struct SendLineImageTool {
    pub queue: SharedMessageQueue,
}

impl Tool for SendLineImageTool {
    const NAME: &'static str = "send_line_image";

    type Error = ToolCallError;
    type Args = SendImageArgs;
    type Output = ToolStatusResult;

    async fn definition(&self, _prompt: String) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: Self::NAME.to_string(),
            description: "CRITICAL:如果你覺得使用者有看圖片的需求，或是某個技能明確生成了圖片網址（Image URL）並且需要展示給使用者，請『絕對要』選擇這個傳圖片的功能，將圖片發送給客戶。絕對不要只用純文字回覆冗長的 URL，這會嚴重破壞使用體驗！".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "original_content_url": {
                        "type": "string",
                        "description": "Original image URL. HTTPS only, max 10MB."
                    },
                    "preview_image_url": {
                        "type": "string",
                        "description": "Preview image URL. HTTPS only, max 1MB."
                    }
                },
                "required": ["original_content_url", "preview_image_url"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let msg = Message::ImageMessage(ImageMessage::new(
            args.original_content_url,
            args.preview_image_url,
        ));

        self.queue.lock().unwrap().push(msg);

        Ok(ToolStatusResult {
            status: "Image successfully prepared for sending.".to_string(),
        })
    }
}

// ==========================================
// 2. SEND LOCATION TOOL
// ==========================================

#[derive(Deserialize)]
pub struct SendLocationArgs {
    title: String,
    address: String,
    latitude: f64,
    longitude: f64,
}

pub struct SendLineLocationTool {
    pub queue: SharedMessageQueue,
}

impl Tool for SendLineLocationTool {
    const NAME: &'static str = "send_line_location";

    type Error = ToolCallError;
    type Args = SendLocationArgs;
    type Output = ToolStatusResult;

    async fn definition(&self, _prompt: String) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: Self::NAME.to_string(),
            description: "MANDATORY:當你需要向使用者分享某個真實地址、經緯度座標或是店家與公司位置時，請『務必』選擇這個原生地位分享功能！這可以讓使用者在手機上直接點擊並開啟地圖導航。千萬不要只把地址當成純文字送出。".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "title": { "type": "string", "description": "Location title (max 100 characters)" },
                    "address": { "type": "string", "description": "Location address (max 100 characters)" },
                    "latitude": { "type": "number", "description": "Latitude" },
                    "longitude": { "type": "number", "description": "Longitude" }
                },
                "required": ["title", "address", "latitude", "longitude"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let msg = Message::LocationMessage(LocationMessage::new(
            args.title,
            args.address,
            args.latitude,
            args.longitude,
        ));

        self.queue.lock().unwrap().push(msg);

        Ok(ToolStatusResult {
            status: "Location map card successfully prepared for sending.".to_string(),
        })
    }
}

// ==========================================
// 3. SEND VIDEO TOOL
// ==========================================

#[derive(Deserialize)]
pub struct SendVideoArgs {
    original_content_url: String,
    preview_image_url: String,
}

pub struct SendLineVideoTool {
    pub queue: SharedMessageQueue,
}

impl Tool for SendLineVideoTool {
    const NAME: &'static str = "send_line_video";

    type Error = ToolCallError;
    type Args = SendVideoArgs;
    type Output = ToolStatusResult;

    async fn definition(&self, _prompt: String) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: Self::NAME.to_string(),
            description: "CRITICAL:當你的對話情境或是某個技能分析出了一段影片並且需要播放給使用者看時，『絕對要』使用此功能發送影片檔案！不要只回傳影片連結。".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "original_content_url": { "type": "string", "description": "URL of the video file (mp4, HTTPS). Max 200MB." },
                    "preview_image_url": { "type": "string", "description": "URL of the thumbnail image (jpeg, HTTPS). Max 1MB." }
                },
                "required": ["original_content_url", "preview_image_url"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let msg = Message::VideoMessage(VideoMessage::new(
            args.original_content_url,
            args.preview_image_url,
        ));
        self.queue.lock().unwrap().push(msg);
        Ok(ToolStatusResult {
            status: "Video prepared.".to_string(),
        })
    }
}

// ==========================================
// 4. SEND AUDIO TOOL
// ==========================================

#[derive(Deserialize)]
pub struct SendAudioArgs {
    original_content_url: String,
    duration: i64,
}

pub struct SendLineAudioTool {
    pub queue: SharedMessageQueue,
}

impl Tool for SendLineAudioTool {
    const NAME: &'static str = "send_line_audio";

    type Error = ToolCallError;
    type Args = SendAudioArgs;
    type Output = ToolStatusResult;

    async fn definition(&self, _prompt: String) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: Self::NAME.to_string(),
            description: "CRITICAL:如果使用者要求聽取聲音、或是你生成了一段音檔 (Audio)，『必須』使用此功能發送語音訊息給使用者收聽！".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "original_content_url": { "type": "string", "description": "URL of the audio file (m4a, HTTPS). Max 200MB." },
                    "duration": { "type": "integer", "description": "Length of audio file in milliseconds." }
                },
                "required": ["original_content_url", "duration"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let msg =
            Message::AudioMessage(AudioMessage::new(args.original_content_url, args.duration));
        self.queue.lock().unwrap().push(msg);
        Ok(ToolStatusResult {
            status: "Audio prepared.".to_string(),
        })
    }
}

// ==========================================
// 5. SEND STICKER TOOL
// ==========================================

#[derive(Deserialize)]
pub struct SendStickerArgs {
    package_id: String,
    sticker_id: String,
}

pub struct SendLineStickerTool {
    pub queue: SharedMessageQueue,
}

impl Tool for SendLineStickerTool {
    const NAME: &'static str = "send_line_sticker";

    type Error = ToolCallError;
    type Args = SendStickerArgs;
    type Output = ToolStatusResult;

    async fn definition(&self, _prompt: String) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: Self::NAME.to_string(),
            description: "OPTIONAL_BUT_RECOMMENDED:當你想要表達強烈的情感、打招呼、慶祝成功、或是想展現活潑的人格魅力時，請『踴躍』使用這個發送貼圖的功能！這能大幅增加使用者的好感度喔！".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "package_id": { "type": "string", "description": "LINE sticker package ID (e.g., '446')" },
                    "sticker_id": { "type": "string", "description": "LINE sticker ID (e.g., '1988')" }
                },
                "required": ["package_id", "sticker_id"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let msg = Message::StickerMessage(StickerMessage::new(args.package_id, args.sticker_id));
        self.queue.lock().unwrap().push(msg);
        Ok(ToolStatusResult {
            status: "Sticker prepared.".to_string(),
        })
    }
}
