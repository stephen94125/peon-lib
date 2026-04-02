use crate::tools::{
    SendLineAudioTool, SendLineImageTool, SendLineLocationTool, SendLineStickerTool,
    SendLineVideoTool, SharedMessageQueue,
};
use line_bot_sdk_rust::{
    client::LINE,
    line_messaging_api::{
        apis::MessagingApiApi,
        models::{Message, ReplyMessageRequest, TextMessage},
    },
    line_webhook::models::{Event, MessageContent, MessageEvent},
};
use peon_core::agent::PeonAgentBuilder;
use std::sync::{Arc, Mutex};

pub async fn handle_event(line: &LINE, event: Event) {
    if let Event::MessageEvent(msg_evt) = event {
        handle_message(line, msg_evt).await;
    } else {
        log::debug!("Ignored non-message event type.");
    }
}

async fn handle_message(line: &LINE, event: MessageEvent) {
    let reply_token = match event.reply_token {
        Some(token) => token,
        None => return,
    };

    let text_input = match *event.message {
        MessageContent::TextMessageContent(txt) => txt.text,
        _ => {
            log::debug!("Ignored non-text message payload.");
            return;
        }
    };

    log::info!("Received text from user: {}", text_input);

    let queue: SharedMessageQueue = Arc::new(Mutex::new(Vec::new()));

    let builder = match PeonAgentBuilder::new().await {
        Ok(b) => b,
        Err(e) => {
            log::error!("Failed to initialize Agent Foundations: {}", e);
            let req = ReplyMessageRequest {
                reply_token: reply_token.clone(),
                messages: vec![Message::TextMessage(TextMessage::new(
                    "❌ 系统初始化失败，请检查环境变数与主机权限設定。".to_string(),
                ))],
                notification_disabled: Some(false),
            };
            let _ = line.messaging_api_client.reply_message(req).await;
            return;
        }
    };

    // Inject LINE Tools
    let agent = builder
        .default_prompt()
        .tool(SendLineImageTool {
            queue: Arc::clone(&queue),
        })
        .tool(SendLineLocationTool {
            queue: Arc::clone(&queue),
        })
        .tool(SendLineVideoTool {
            queue: Arc::clone(&queue),
        })
        .tool(SendLineAudioTool {
            queue: Arc::clone(&queue),
        })
        .tool(SendLineStickerTool {
            queue: Arc::clone(&queue),
        })
        .build();

    // Call LLM
    let text_response_result = agent.prompt(&text_input).await;

    let text_response = match text_response_result {
        Ok(txt) => txt,
        Err(e) => format!("❌ Agent 执行期发生错误:\n{}", e),
    };

    // Construct final payload
    let mut messages_to_send: Vec<Message> = Vec::new();

    // Extract all generated rich-media messages from the tool queue
    if let Ok(mut lock) = queue.lock() {
        for msg in lock.drain(..) {
            messages_to_send.push(msg);
        }
    }

    // Always push the text response as the final answering bubble if it conveys info
    if !text_response.trim().is_empty() {
        // Only append if we aren't exceeding the LINE API limit of 5 bubbles per reply
        if messages_to_send.len() < 5 {
            messages_to_send.push(Message::TextMessage(TextMessage::new(text_response)));
        } else {
            messages_to_send[4] = Message::TextMessage(TextMessage::new(text_response));
        }
    }

    if messages_to_send.is_empty() {
        return;
    }

    let req = ReplyMessageRequest {
        reply_token,
        messages: messages_to_send,
        notification_disabled: Some(false),
    };

    if let Err(e) = line.messaging_api_client.reply_message(req).await {
        log::error!("Failed to send LINE reply: {:?}", e);
    }
}
