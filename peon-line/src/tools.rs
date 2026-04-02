use line_bot_sdk_rust::line_messaging_api::models::{
    AudioMessage, ImageMessage, LocationMessage, Message, PushMessageRequest, StickerMessage, VideoMessage
};
use line_bot_sdk_rust::line_messaging_api::apis::MessagingApiApi;
use line_bot_sdk_rust::client::LINE;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};

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

#[derive(Serialize)]
pub struct ToolStatusResult {
    status: String,
}

// ==========================================
// 1. SEND IMAGE TOOL
// ==========================================

#[derive(Deserialize)]
pub struct SendImageArgs {
    original_content_url: String,
    preview_image_url: String,
}

pub struct SendLineImageTool {
    pub line: LINE,
    pub target_id: String,
}

impl Tool for SendLineImageTool {
    const NAME: &'static str = "send_line_image";

    type Error = ToolCallError;
    type Args = SendImageArgs;
    type Output = ToolStatusResult;

    async fn definition(&self, _prompt: String) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: Self::NAME.to_string(),
            description: "CRITICAL: If the user needs to view an image, or if a skill explicitly generates an image URL intended for the user, you MUST use this tool to display the image directly. NEVER reply with just a pure text URL, as it degrades the user experience.".to_string(),
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
            args.preview_image_url
        ));

        let req = PushMessageRequest {
            to: self.target_id.clone(),
            messages: vec![msg],
            custom_aggregation_units: None,
            notification_disabled: Some(false),
        };

        if let Err(e) = self.line.messaging_api_client.push_message(req, None).await {
            return Err(ToolCallError::new(format!("Failed to push image: {e}")));
        }

        Ok(ToolStatusResult {
            status: "Image successfully pushed to the user.".to_string(),
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
    pub line: LINE,
    pub target_id: String,
}

impl Tool for SendLineLocationTool {
    const NAME: &'static str = "send_line_location";

    type Error = ToolCallError;
    type Args = SendLocationArgs;
    type Output = ToolStatusResult;

    async fn definition(&self, _prompt: String) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: Self::NAME.to_string(),
            description: "MANDATORY: When sharing a physical address, coordinates, or store location with the user, you MUST use this native location sharing tool. This allows the user to tap and interact with maps directly. NEVER send an address as just pure text.".to_string(),
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
            args.longitude
        ));

        let req = PushMessageRequest {
            to: self.target_id.clone(),
            messages: vec![msg],
            custom_aggregation_units: None,
            notification_disabled: Some(false),
        };

        if let Err(e) = self.line.messaging_api_client.push_message(req, None).await {
            return Err(ToolCallError::new(format!("Failed to push location: {e}")));
        }

        Ok(ToolStatusResult {
            status: "Location card successfully pushed to the user.".to_string(),
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
    pub line: LINE,
    pub target_id: String,
}

impl Tool for SendLineVideoTool {
    const NAME: &'static str = "send_line_video";

    type Error = ToolCallError;
    type Args = SendVideoArgs;
    type Output = ToolStatusResult;

    async fn definition(&self, _prompt: String) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: Self::NAME.to_string(),
            description: "CRITICAL: If the context or a skill generates a video that needs to be shown to the user, you MUST use this tool to send the video file directly! Never reply with just a generic video link.".to_string(),
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
            args.preview_image_url
        ));
        
        let req = PushMessageRequest {
            to: self.target_id.clone(),
            messages: vec![msg],
            custom_aggregation_units: None,
            notification_disabled: Some(false),
        };

        if let Err(e) = self.line.messaging_api_client.push_message(req, None).await {
            return Err(ToolCallError::new(format!("Failed to push video: {e}")));
        }

        Ok(ToolStatusResult { status: "Video pushed to user.".to_string() })
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
    pub line: LINE,
    pub target_id: String,
}

impl Tool for SendLineAudioTool {
    const NAME: &'static str = "send_line_audio";

    type Error = ToolCallError;
    type Args = SendAudioArgs;
    type Output = ToolStatusResult;

    async fn definition(&self, _prompt: String) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: Self::NAME.to_string(),
            description: "CRITICAL: If the user requests to listen to an audio, or if you generate an audio file, you MUST use this tool to send the voice message directly to the user for playback!".to_string(),
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
        let msg = Message::AudioMessage(AudioMessage::new(
            args.original_content_url,
            args.duration
        ));
        
        let req = PushMessageRequest {
            to: self.target_id.clone(),
            messages: vec![msg],
            custom_aggregation_units: None,
            notification_disabled: Some(false),
        };

        if let Err(e) = self.line.messaging_api_client.push_message(req, None).await {
            return Err(ToolCallError::new(format!("Failed to push audio: {e}")));
        }

        Ok(ToolStatusResult { status: "Audio pushed to user.".to_string() })
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
    pub line: LINE,
    pub target_id: String,
}

impl Tool for SendLineStickerTool {
    const NAME: &'static str = "send_line_sticker";

    type Error = ToolCallError;
    type Args = SendStickerArgs;
    type Output = ToolStatusResult;

    async fn definition(&self, _prompt: String) -> rig::completion::ToolDefinition {
        rig::completion::ToolDefinition {
            name: Self::NAME.to_string(),
            description: "OPTIONAL_BUT_RECOMMENDED: When you want to convey strong emotion, greet, celebrate a success, or show a vibrant personality, use this tool to send an engaging sticker! This significantly increases user affinity.".to_string(),
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
        let msg = Message::StickerMessage(StickerMessage::new(
            args.package_id,
            args.sticker_id
        ));

        let req = PushMessageRequest {
            to: self.target_id.clone(),
            messages: vec![msg],
            custom_aggregation_units: None,
            notification_disabled: Some(false),
        };

        if let Err(e) = self.line.messaging_api_client.push_message(req, None).await {
            return Err(ToolCallError::new(format!("Failed to push sticker: {e}")));
        }

        Ok(ToolStatusResult { status: "Sticker pushed to user.".to_string() })
    }
}

// ==========================================
// TODO / Pending Tools
// ==========================================
// 
// 1. Coupon Message: Waiting for implementation
//    - Needs Coupon API specifics.
// 2. Imagemap Message: Waiting for implementation
//    - Needs coordinate and map mapping details.
// 3. Flex Message: Waiting for implementation
//    - Needs extensive JSON dynamic construction template builder.
// 4. Template Message: DEPRECATED / LOW PRIORITY 
//    - The LINE API has largely superseded Template messages with Flex messages or plain text + Quick Reply.
//    - We will stick to Flex message later, so template message support may be skipped.
