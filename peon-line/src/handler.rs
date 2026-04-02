use crate::tools::{
    SendLineAudioTool, SendLineImageTool, SendLineLocationTool, SendLineStickerTool,
    SendLineVideoTool,
};
use line_bot_sdk_rust::{
    client::LINE,
    line_messaging_api::{
        apis::MessagingApiApi,
        models::{Message, ReplyMessageRequest, TextMessage},
    },
    line_webhook::models::{Event, MessageContent, MessageEvent, Source},
};
use peon_core::agent::PeonAgentBuilder;

pub async fn handle_event(line: &LINE, event: Event) {
    if let Event::MessageEvent(msg_evt) = event {
        handle_message(line, msg_evt).await;
    } else {
        log::debug!("Ignored non-message event type.");
    }
}

/// Extracts the target ID (user, group, or room) from the event source
fn extract_target_id(source: &Option<Box<Source>>) -> Option<String> {
    match source {
        Some(s) => match s.as_ref() {
            Source::UserSource(u) => u.user_id.clone(),
            Source::GroupSource(g) => Some(g.group_id.clone()),
            Source::RoomSource(r) => Some(r.room_id.clone()),
            _ => None,
        },
        None => None,
    }
}

async fn handle_message(line: &LINE, event: MessageEvent) {
    let reply_token = match event.reply_token {
        Some(token) => token,
        None => return,
    };

    let target_id = match extract_target_id(&event.source) {
        Some(id) => id,
        None => {
            log::warn!("Could not extract target ID from event source. Cannot reply.");
            return;
        }
    };

    let text_input = match *event.message {
        MessageContent::TextMessageContent(txt) => txt.text,
        _ => {
            log::debug!("Ignored non-text message payload.");
            return;
        }
    };

    log::info!("Received text from target {}: {}", target_id, text_input);

    let builder = match PeonAgentBuilder::new().await {
        Ok(b) => b,
        Err(e) => {
            log::error!("Failed to initialize Agent Foundations: {}", e);
            let req = ReplyMessageRequest {
                reply_token,
                messages: vec![Message::TextMessage(TextMessage::new(
                    "❌ 系统初始化失败，请检查环境变数与主机权限設定。".to_string(),
                ))],
                notification_disabled: Some(false),
            };
            let _ = line.messaging_api_client.reply_message(req).await;
            return;
        }
    };

    // Inject LINE Tools (they will use push_message to send asynchronously during reasoning!)
    let agent = builder
        .default_prompt()
        .tool(SendLineImageTool {
            line: line.clone(),
            target_id: target_id.clone(),
        })
        .tool(SendLineLocationTool {
            line: line.clone(),
            target_id: target_id.clone(),
        })
        .tool(SendLineVideoTool {
            line: line.clone(),
            target_id: target_id.clone(),
        })
        .tool(SendLineAudioTool {
            line: line.clone(),
            target_id: target_id.clone(),
        })
        .tool(SendLineStickerTool {
            line: line.clone(),
            target_id: target_id.clone(),
        })
        .build();

    // Call LLM
    let text_response_result = agent.prompt(&text_input).await;

    let text_response = match text_response_result {
        Ok(txt) => txt,
        Err(e) => format!("❌ Agent 执行期发生错误:\n{}", e),
    };

    // Push the final text response via reply_message.
    // This resolves the reply_token cleanly while other tools used push_message.
    if !text_response.trim().is_empty() {
        let req = ReplyMessageRequest {
            reply_token,
            messages: vec![Message::TextMessage(TextMessage::new(text_response))],
            notification_disabled: Some(false),
        };

        if let Err(e) = line.messaging_api_client.reply_message(req).await {
            log::error!("Failed to send LINE reply: {:?}", e);
        }
    }
}
