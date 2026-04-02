use anyhow::Result;
use peon_core::agent::PeonAgent;
use teloxide::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    log::info!("🚀 Starting Peon Telegram Bot...");

    // 2. Initialize Telegram Bot from TELOXIDE_TOKEN environment variable
    let bot = Bot::from_env();

    // 3. Start single-threaded REPL processing incoming messages
    teloxide::repl(bot, |bot: Bot, msg: Message| async move {
        // We only process pure text messages for this MVP phase
        if let Some(text) = msg.text() {
            log::info!("Received message from chat ID {}: {}", msg.chat.id, text);
            
            // Provide visual feedback that the bot is thinking
            let _ = bot.send_chat_action(msg.chat.id, teloxide::types::ChatAction::Typing).await;

            // Instantiate a fresh PeonAgent
            // We instantiate per-message to guarantee strict isolation and empty tool whitelists
            // between requests until long-term memory management is implemented.
            match PeonAgent::new().await {
                Ok(agent) => {
                    match agent.prompt(text).await {
                        Ok(response) => {
                            bot.send_message(msg.chat.id, response).await?;
                        }
                        Err(e) => {
                            let error_msg = format!("❌ Agent encountered an error:\n{}", e);
                            log::error!("{}", error_msg);
                            bot.send_message(msg.chat.id, error_msg).await?;
                        }
                    }
                }
                Err(e) => {
                    let env_err = format!("❌ Failed to initialize Agent. Please check the host server logs and environment variables:\n{}", e);
                    log::error!("{}", env_err);
                    bot.send_message(msg.chat.id, env_err).await?;
                }
            }
        } else {
            log::debug!("Ignored a non-text message type.");
        }
        
        Ok(())
    })
    .await;

    Ok(())
}
