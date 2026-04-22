mod tools;

use anyhow::Result;
use base64::Engine as _;
use peon_core::{ChatSession, PeonSharedCore};
use peon_runtime::message::{ContentPart, Message as PeonMessage};
use std::collections::HashMap;
use std::sync::Arc;
use teloxide::{
    net::Download,
    prelude::*,
    types::{ChatAction, MediaKind, Message as TgMessage, MessageKind},
};
use tokio::sync::Mutex;

// ──────────────────────────────────────────────
// Session store: per-chat persistent state
// ──────────────────────────────────────────────

/// Keyed by Telegram ChatId, holds the per-chat conversation history and
/// PeonEngine path whitelist. Last-write-wins for concurrent messages.
type SessionStore = Arc<Mutex<HashMap<ChatId, ChatSession>>>;

// ──────────────────────────────────────────────
// Entry point
// ──────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    if std::env::args().any(|arg| arg == "--init" || arg == "-i") {
        peon_core::setup::init_workspace().await?;
        return Ok(());
    }

    log::info!("🚀 Starting Peon Telegram Bot...");

    // Build the shared core ONCE — expensive (skill scan + Casbin load)
    let core = Arc::new(PeonSharedCore::new().await?.default_prompt());

    let session_store: SessionStore = Arc::new(Mutex::new(HashMap::new()));
    let bot = Bot::from_env();

    log::info!("✅ Core initialized. Bot ready.");

    teloxide::repl(bot, move |bot: Bot, msg: TgMessage| {
        let core = Arc::clone(&core);
        let store = Arc::clone(&session_store);
        async move {
            if let Err(e) = handle_message(bot, msg, core, store).await {
                log::error!("Unhandled handler error: {}", e);
            }
            respond(())
        }
    })
    .await;

    Ok(())
}

// ──────────────────────────────────────────────
// Message handler
// ──────────────────────────────────────────────

async fn handle_message(
    bot: Bot,
    msg: TgMessage,
    core: Arc<PeonSharedCore>,
    store: SessionStore,
) -> Result<()> {
    let chat_id = msg.chat.id;
    let uid = msg
        .from
        .as_ref()
        .map(|u| u.id.to_string())
        .unwrap_or_else(|| chat_id.to_string());

    let _ = bot.send_chat_action(chat_id, ChatAction::Typing).await;

    // Extract multimodal content
    let content_parts = match extract_content(&bot, &msg).await {
        Ok(parts) => parts,
        Err(e) => {
            log::warn!("Content extraction failed for {}: {}", uid, e);
            bot.send_message(chat_id, format!("❌ Could not process your message: {}", e))
                .await?;
            return Ok(());
        }
    };

    if content_parts.is_empty() {
        log::debug!("Ignored unhandled message type from {}", uid);
        return Ok(());
    }

    log::info!(
        "Message from uid={} chat={}: {} part(s)",
        uid,
        chat_id,
        content_parts.len()
    );

    // ── Get or create session for this chat, then snapshot ──────────────────
    // Snapshotting (deep-clone) is done under the lock but the heavy work
    // (agent run) happens outside, so the lock is held briefly.
    let session_snapshot = {
        let mut store_guard = store.lock().await;
        let session = store_guard
            .entry(chat_id)
            .or_insert_with(|| core.new_session());

        if session.history.is_empty() {
            log::info!("New chat session created for chat_id={}", chat_id);
        }

        session.snapshot().await
    };

    // ── Build the agent (cheap) with per-request Telegram output tools ───────
    let agent = core.build_agent(
        session_snapshot,
        vec![
            Box::new(tools::SendVoiceTool::new(bot.clone(), chat_id)),
            Box::new(tools::SendCsvTool::new(bot.clone(), chat_id)),
            Box::new(tools::SendInlineKeyboardTool::new(bot.clone(), chat_id)),
            Box::new(tools::SendChatActionTool::new(bot.clone(), chat_id)),
        ],
    );

    // ── Run the agent ────────────────────────────────────────────────────────
    // Build input message
    let input: PeonMessage = if content_parts.len() == 1 {
        match content_parts.into_iter().next().unwrap() {
            ContentPart::Text { text } => text.into(),
            other => vec![other].into(),
        }
    } else {
        content_parts.into()
    };

    let (response, updated_session) = match agent.prompt(input, &uid).await {
        Ok(pair) => pair,
        Err(e) => {
            let err = format!("❌ Agent error:\n{}", e);
            log::error!("{}", err);
            bot.send_message(chat_id, err).await?;
            return Ok(());
        }
    };

    // ── Write session back (last-write-wins) ─────────────────────────────────
    {
        let mut store_guard = store.lock().await;
        store_guard.insert(chat_id, updated_session);
    }

    // ── Send the final text response ─────────────────────────────────────────
    if !response.output.trim().is_empty() {
        bot.send_message(chat_id, response.output).await?;
    }
    Ok(())
}

// ──────────────────────────────────────────────
// Content extraction helpers
// ──────────────────────────────────────────────

async fn download_file(bot: &Bot, file_id: &str) -> Result<Vec<u8>> {
    let file = bot.get_file(file_id).await?;
    let mut buf = Vec::new();
    bot.download_file(&file.path, &mut buf).await?;
    Ok(buf)
}

fn to_base64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// Extract `ContentPart`s from an incoming Telegram message.
///
/// Returns empty vec for ignored types (stickers, polls, system events).
/// Returns `Err` for attempted-but-unsupported types (binary files, etc.).
async fn extract_content(bot: &Bot, msg: &TgMessage) -> Result<Vec<ContentPart>> {
    let MessageKind::Common(common) = &msg.kind else {
        return Ok(vec![]);
    };

    let caption_text: Option<String> = match &common.media_kind {
        MediaKind::Photo(m) => m.caption.clone(),
        MediaKind::Video(m) => m.caption.clone(),
        MediaKind::Audio(m) => m.caption.clone(),
        MediaKind::Voice(m) => m.caption.clone(),
        MediaKind::Document(m) => m.caption.clone(),
        _ => None,
    };

    match &common.media_kind {
        // ── Text ───────────────────────────────────────────────────────────────
        MediaKind::Text(m) => Ok(vec![ContentPart::Text {
            text: m.text.clone(),
        }]),

        // ── Photo ──────────────────────────────────────────────────────────────
        MediaKind::Photo(m) => {
            let photo = m
                .photo
                .last()
                .ok_or_else(|| anyhow::anyhow!("Photo had no sizes"))?;
            log::info!(
                "Downloading photo ({}×{}, file_id={})",
                photo.width,
                photo.height,
                photo.file.id
            );
            let bytes = download_file(bot, &photo.file.id).await?;
            let mut parts = Vec::new();
            if let Some(cap) = caption_text {
                parts.push(ContentPart::Text { text: cap });
            }
            parts.push(ContentPart::ImageBase64 {
                data: to_base64(&bytes),
                media_type: "image/jpeg".into(),
            });
            Ok(parts)
        }

        // ── Voice note (OGG/Opus) ──────────────────────────────────────────────
        MediaKind::Voice(m) => {
            log::info!(
                "Downloading voice note ({}s, file_id={})",
                m.voice.duration,
                m.voice.file.id
            );
            let bytes = download_file(bot, &m.voice.file.id).await?;
            let mut parts = Vec::new();
            if let Some(cap) = caption_text {
                parts.push(ContentPart::Text { text: cap });
            }
            parts.push(ContentPart::Audio {
                data: to_base64(&bytes),
                format: "wav".into(),
            });
            Ok(parts)
        }

        // ── Audio file ────────────────────────────────────────────────────────
        MediaKind::Audio(m) => {
            let mime = m
                .audio
                .mime_type
                .as_ref()
                .map(|mt| mt.to_string())
                .unwrap_or_else(|| "audio/mpeg".into());
            let format = if mime.contains("ogg") {
                "wav"
            } else if mime.contains("wav") {
                "wav"
            } else {
                "mp3"
            }
            .to_string();
            log::info!(
                "Downloading audio (mime={}, file_id={})",
                mime,
                m.audio.file.id
            );
            let bytes = download_file(bot, &m.audio.file.id).await?;
            let mut parts = Vec::new();
            if let Some(cap) = caption_text {
                parts.push(ContentPart::Text { text: cap });
            }
            parts.push(ContentPart::Audio {
                data: to_base64(&bytes),
                format,
            });
            Ok(parts)
        }

        // ── Video ──────────────────────────────────────────────────────────────
        MediaKind::Video(m) => {
            let mime = m
                .video
                .mime_type
                .as_ref()
                .map(|mt| mt.to_string())
                .unwrap_or_else(|| "video/mp4".into());
            log::info!(
                "Downloading video ({}s, mime={}, file_id={})",
                m.video.duration,
                mime,
                m.video.file.id
            );
            let bytes = download_file(bot, &m.video.file.id).await?;
            let mut parts = Vec::new();
            if let Some(cap) = caption_text {
                parts.push(ContentPart::Text { text: cap });
            }
            parts.push(ContentPart::VideoBase64 {
                data: to_base64(&bytes),
                media_type: mime,
            });
            Ok(parts)
        }

        // ── Document ──────────────────────────────────────────────────────────
        MediaKind::Document(m) => {
            let mime = m
                .document
                .mime_type
                .as_ref()
                .map(|mt| mt.to_string())
                .unwrap_or_else(|| "application/octet-stream".into());
            let filename = m
                .document
                .file_name
                .clone()
                .unwrap_or_else(|| "file".into());

            let is_text = mime.starts_with("text/")
                || mime == "application/json"
                || mime == "application/xml";
            let is_csv =
                mime == "text/csv" || filename.ends_with(".csv") || filename.ends_with(".tsv");
            let is_pdf = mime == "application/pdf";

            if !is_text && !is_csv && !is_pdf {
                return Err(anyhow::anyhow!(
                    "Unsupported file type '{}'. Accepted: plain text, CSV, JSON, PDF.",
                    mime
                ));
            }

            log::info!(
                "Downloading document '{}' (mime={}, file_id={})",
                filename,
                mime,
                m.document.file.id
            );
            let bytes = download_file(bot, &m.document.file.id).await?;
            let mut parts = Vec::new();
            if let Some(cap) = caption_text {
                parts.push(ContentPart::Text { text: cap });
            }

            if is_pdf {
                parts.push(ContentPart::File {
                    data: to_base64(&bytes),
                    media_type: "application/pdf".into(),
                    filename: Some(filename.clone()),
                });
                parts.push(ContentPart::Text {
                    text: format!("[Attached PDF: {}]", filename),
                });
            } else {
                let text = String::from_utf8(bytes)
                    .map_err(|_| anyhow::anyhow!("'{}' has non-UTF-8 data.", filename))?;
                let content = if is_csv {
                    format!("[File: {} (CSV)]\n\n```csv\n{}\n```", filename, text)
                } else {
                    format!("[File: {}]\n\n{}", filename, text)
                };
                parts.push(ContentPart::Text { text: content });
            }
            Ok(parts)
        }

        // ── Location ───────────────────────────────────────────────────────────
        MediaKind::Location(m) => {
            let (lat, lon) = (m.location.latitude, m.location.longitude);
            log::info!("Received location ({}, {})", lat, lon);
            Ok(vec![ContentPart::Text {
                text: format!(
                    "The user shared their location:\nLatitude: {}\nLongitude: {}\nGoogle Maps: https://maps.google.com/?q={},{}",
                    lat, lon, lat, lon
                ),
            }])
        }

        // ── Ignored ────────────────────────────────────────────────────────────
        _ => Ok(vec![]),
    }
}
