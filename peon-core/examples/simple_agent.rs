//! Minimal example: wire up a Peon agent with one call.
//!
//! Run with: `RUST_LOG=info cargo run --example simple_agent`
//! For debug:  `RUST_LOG=debug cargo run --example simple_agent`

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    let agent = peon_core::agent::PeonAgentBuilder::new()
        .await?
        .default_prompt()
        .build();

    let user_input = "Roll a 20-sided die for me";
    log::info!("User input: {}", user_input);

    // The UID is now explicitly passed — no more CURRENT_UID.scope() magic!
    let response = agent.prompt(user_input, "agent").await?;
    log::info!("Agent response: {}", response);

    Ok(())
}
