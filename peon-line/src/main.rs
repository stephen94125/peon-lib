mod handler;
mod tools;

use axum::{routing::post, Router};
use dotenvy::dotenv;
use line_bot_sdk_rust::{
    client::LINE, line_webhook::models::CallbackRequest, parser::signature::validate_signature,
    support::axum::Signature,
};
use std::env;

/// Webhook callback endpoint.
///
/// Receives LINE webhook events, validates the signature, parses the request,
/// and dispatches each event to the agent handler.
async fn callback(
    signature: Signature,
    body: String,
) -> Result<&'static str, (axum::http::StatusCode, String)> {
    let channel_secret =
        env::var("LINE_CHANNEL_SECRET").expect("Failed to get LINE_CHANNEL_SECRET");
    let access_token =
        env::var("LINE_CHANNEL_ACCESS_TOKEN").expect("Failed to get LINE_CHANNEL_ACCESS_TOKEN");

    let line = LINE::new(access_token);

    // Verify the request signature using the channel secret
    if !validate_signature(&channel_secret, &signature.key, &body) {
        log::error!("Invalid LINE X-Line-Signature");
        return Err((
            axum::http::StatusCode::BAD_REQUEST,
            "x-line-signature is invalid.".to_string(),
        ));
    }

    let request: CallbackRequest = serde_json::from_str(&body).map_err(|e| {
        log::error!("Failed to parse LINE CallbackRequest JSON: {}", e);
        (
            axum::http::StatusCode::BAD_REQUEST,
            format!("JSON parse error: {e}"),
        )
    })?;

    // Dispatch each event to the handler
    for event in request.events {
        handler::handle_event(&line, event).await;
    }

    Ok("ok")
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    // 2. Load environment variables
    dotenv().ok();

    log::info!("🚀 Starting Peon LINE Bot Webhook Server...");

    // Validate that the keys exist at startup so it fails fast
    let _ = env::var("LINE_CHANNEL_SECRET").expect("LINE_CHANNEL_SECRET is required in .env");
    let _ = env::var("LINE_CHANNEL_ACCESS_TOKEN").expect("LINE_CHANNEL_ACCESS_TOKEN is required in .env");

    let app = Router::new().route("/callback", post(callback));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    log::info!("Webhook Server listening on http://{}", listener.local_addr()?);
    axum::serve(listener, app).await?;

    Ok(())
}
