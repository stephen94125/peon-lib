//! Minimal example: wire up a Peon agent with one call.
//!
//! Run with: `RUST_LOG=info cargo run --example simple_agent`
//! For debug:  `RUST_LOG=debug cargo run --example simple_agent`

use peon_lib::agent::PeonAgent;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    let agent = PeonAgent::new().await?;

    let user_input = "Roll a 20-sided die for me";
    log::info!("User input: {}", user_input);

    let response = agent.prompt(user_input).await?;
    log::info!("Agent response: {}", response);

    Ok(())
}
