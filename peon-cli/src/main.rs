use anyhow::{bail, Context, Result};
use clap::Parser;
use std::io::{self, IsTerminal, Read};

#[derive(Parser, Debug)]
#[command(name = "peon-cli", version, about = "CLI for the Peon Zero-Trust Agent Framework")]
struct Cli {
    /// The message to prompt the agent with
    #[arg(short, long)]
    message: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Setup logging strictly to STDERR so STDOUT remains clean for pipes.
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .target(env_logger::Target::Stderr)
        .format_timestamp_millis()
        .init();

    // 2. Parse arguments
    let cli = Cli::parse();

    // 3. Resolve Input
    let input = if let Some(msg) = cli.message {
        msg
    } else if !io::stdin().is_terminal() {
        let mut buffer = String::new();
        io::stdin()
            .read_to_string(&mut buffer)
            .context("Failed to read from stdin")?;
        
        let trimmed = buffer.trim();
        if trimmed.is_empty() {
            bail!("Input via stdin was empty.");
        }
        trimmed.to_string()
    } else {
        // According to user request: "cli 就是沒輸入就直接退出" -> "just exit if no input"
        bail!("No input provided. Please use standard input pipe (e.g. `echo '...' | peon-cli`) or provide the `-m` argument.");
    };

    // 4. Initialize Agent
    log::info!("Starting Peon Agent...");
    let agent = peon_core::agent::PeonAgentBuilder::new()
        .await
        .context("Failed to initialize PeonAgent")?
        .default_prompt()
        .build();

    // 5. Prompt Agent
    log::info!("Dispatching prompt to agent...");
    log::debug!("Prompt payload length: {} characters", input.len());
    use peon_core::tools::CURRENT_UID;
    let response = CURRENT_UID.scope("agent".to_string(), async {
        agent.prompt(&input).await
    }).await.context("Agent execution failed")?;

    // 6. Return purely to STDOUT
    // We use print! or println! here because it writes to STDOUT explicitly. 
    println!("{}", response);

    Ok(())
}
