//! Simple chat example: send a prompt to the LLM and print the response.
//!
//! # Setup
//!
//! Create a `.env` file in the project root with one of:
//! ```text
//! OPENAI_API_KEY=sk-...
//! ANTHROPIC_API_KEY=sk-ant-...
//! GEMINI_API_KEY=AIza...
//! OPENROUTER_API_KEY=sk-or-...
//! ```
//!
//! # Run
//!
//! ```bash
//! # OpenAI (default)
//! cargo run --example simple_chat
//!
//! # Anthropic
//! PROVIDER=anthropic cargo run --example simple_chat
//!
//! # Gemini
//! PROVIDER=gemini cargo run --example simple_chat
//!
//! # OpenRouter
//! PROVIDER=openrouter cargo run --example simple_chat
//! ```

use peon_runtime::providers::anthropic::AnthropicProvider;
use peon_runtime::providers::gemini::GeminiProvider;
use peon_runtime::providers::openai::OpenAiProvider;
use peon_runtime::{AgentLoop, CompletionProvider, RequestContext};

fn create_provider() -> Box<dyn CompletionProvider> {
    dotenvy::dotenv().ok();

    let provider = std::env::var("PROVIDER").unwrap_or_else(|_| "openai".into());

    match provider.as_str() {
        "openai" => {
            let key = std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not set");
            let model = std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-4o-mini".into());
            println!("Using OpenAI: {}", model);
            Box::new(OpenAiProvider::new(model, key))
        }
        "anthropic" => {
            let key = std::env::var("ANTHROPIC_API_KEY").expect("ANTHROPIC_API_KEY not set");
            let model = std::env::var("ANTHROPIC_MODEL")
                .unwrap_or_else(|_| "claude-sonnet-4-20250514".into());
            println!("Using Anthropic: {}", model);
            Box::new(AnthropicProvider::new(model, key))
        }
        "gemini" => {
            let key = std::env::var("GEMINI_API_KEY").expect("GEMINI_API_KEY not set");
            let model = std::env::var("GEMINI_MODEL").unwrap_or_else(|_| "gemini-2.5-flash".into());
            println!("Using Gemini: {}", model);
            Box::new(GeminiProvider::new(model, key))
        }
        "openrouter" => {
            let key = std::env::var("OPENROUTER_API_KEY").expect("OPENROUTER_API_KEY not set");
            let model = std::env::var("OPENROUTER_MODEL")
                .unwrap_or_else(|_| "anthropic/claude-sonnet-4-20250514".into());
            println!("Using OpenRouter: {}", model);
            Box::new(OpenAiProvider::openrouter(model, key))
        }
        other => panic!(
            "Unknown provider: {}. Use: openai, anthropic, gemini, openrouter",
            other
        ),
    }
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let provider = create_provider();

    let agent = AgentLoop::builder(provider)
        .system_prompt("You are a helpful assistant. Be concise.")
        .max_turns(1) // No tools, so 1 turn is enough
        .build();

    let ctx = RequestContext::new("example_user");

    let prompt = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "What is the capital of France? Answer in one sentence.".into());

    println!("\n> {}\n", prompt);

    match agent.run(prompt.as_str(), &[], &ctx).await {
        Ok(response) => {
            println!("{}\n", response.output);
            if let Some(usage) = &response.usage.input_tokens {
                println!(
                    "Tokens: {} in / {} out",
                    usage,
                    response.usage.output_tokens.unwrap_or(0)
                );
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
        }
    }
}
