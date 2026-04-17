//! Tool-calling example: demonstrate the agent loop with context injection.
//!
//! # What This Shows
//!
//! 1. How to implement `PeonTool`
//! 2. How `RequestContext` (uid) is passed to every tool call
//! 3. Multi-turn agent loop (LLM → tool call → result → LLM → final answer)
//!
//! # Setup
//!
//! Create a `.env` file in `peon-runtime/` (see `.env.example`):
//! ```text
//! PROVIDER=openai
//! MODEL=gpt-4o-mini
//! API_KEY=sk-...
//! ```
//!
//! # Run
//!
//! ```bash
//! cargo run -p peon-runtime --example tool_call
//! ```

use peon_runtime::providers::anthropic::AnthropicProvider;
use peon_runtime::providers::gemini::GeminiProvider;
use peon_runtime::providers::openai::OpenAiProvider;
use peon_runtime::{
    AgentLoop, BoxFuture, CompletionProvider, PeonTool, RequestContext, ToolDefinition, ToolError,
};

// ==========================================
// Example Tool: Weather Lookup
// ==========================================

struct WeatherTool;

impl PeonTool for WeatherTool {
    fn name(&self) -> &str {
        "get_weather"
    }

    fn definition(&self, _ctx: &RequestContext) -> BoxFuture<'_, ToolDefinition> {
        Box::pin(async {
            ToolDefinition {
                name: "get_weather".into(),
                description: "Get the current weather for a city.".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "city": {
                            "type": "string",
                            "description": "The city name, e.g., 'Tokyo'"
                        }
                    },
                    "required": ["city"]
                }),
            }
        })
    }

    fn call(&self, args: &str, ctx: &RequestContext) -> BoxFuture<'_, Result<String, ToolError>> {
        let args = args.to_string();
        let uid = ctx.uid().to_string();
        Box::pin(async move {
            let parsed: serde_json::Value =
                serde_json::from_str(&args).map_err(|e| ToolError::invalid_args(e.to_string()))?;

            let city = parsed["city"]
                .as_str()
                .ok_or_else(|| ToolError::invalid_args("Missing 'city' field"))?;

            // In real code, you'd check permissions here:
            // enforcer.check_permission(ctx.uid(), "weather", "read")?;

            println!("  [Tool] get_weather called by uid={}, city={}", uid, city);

            // Fake weather data
            Ok(format!(
                "Weather in {}: 22°C, partly cloudy, 65% humidity",
                city
            ))
        })
    }
}

// ==========================================
// Example Tool: User Info (demonstrates ctx usage)
// ==========================================

struct WhoAmITool;

impl PeonTool for WhoAmITool {
    fn name(&self) -> &str {
        "who_am_i"
    }

    fn definition(&self, _ctx: &RequestContext) -> BoxFuture<'_, ToolDefinition> {
        Box::pin(async {
            ToolDefinition {
                name: "who_am_i".into(),
                description: "Returns information about the current user.".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {}
                }),
            }
        })
    }

    fn call(&self, _args: &str, ctx: &RequestContext) -> BoxFuture<'_, Result<String, ToolError>> {
        let uid = ctx.uid().to_string();
        let platform = ctx
            .get_metadata("platform")
            .unwrap_or("unknown")
            .to_string();
        Box::pin(async move {
            println!("  [Tool] who_am_i called by uid={}", uid);
            Ok(format!("User ID: {}, Platform: {}", uid, platform))
        })
    }
}

// ==========================================
// Provider Factory
// ==========================================

fn create_provider() -> Box<dyn CompletionProvider> {
    dotenvy::dotenv().ok();

    let provider = std::env::var("PROVIDER").unwrap_or_else(|_| "openai".into());
    let api_key = std::env::var("API_KEY").expect("API_KEY not set in .env");

    match provider.as_str() {
        "openai" => {
            let model = std::env::var("MODEL").unwrap_or_else(|_| "gpt-4o-mini".into());
            println!("Using OpenAI: {}", model);
            Box::new(OpenAiProvider::new(model, api_key))
        }
        "anthropic" => {
            let model =
                std::env::var("MODEL").unwrap_or_else(|_| "claude-sonnet-4-20250514".into());
            println!("Using Anthropic: {}", model);
            Box::new(AnthropicProvider::new(model, api_key))
        }
        "gemini" => {
            let model = std::env::var("MODEL").unwrap_or_else(|_| "gemini-2.5-flash".into());
            println!("Using Gemini: {}", model);
            Box::new(GeminiProvider::new(model, api_key))
        }
        "openrouter" => {
            let model = std::env::var("MODEL")
                .unwrap_or_else(|_| "anthropic/claude-sonnet-4-20250514".into());
            println!("Using OpenRouter: {}", model);
            Box::new(OpenAiProvider::openrouter(model, api_key))
        }
        other => panic!(
            "Unknown PROVIDER: '{}'. Supported: openai, anthropic, gemini, openrouter",
            other
        ),
    }
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let provider = create_provider();

    let agent = AgentLoop::builder(provider)
        .system_prompt(
            "You are a helpful assistant with access to tools. \
             Use the tools when needed to answer questions. \
             Always be concise.",
        )
        .tool(WeatherTool)
        .tool(WhoAmITool)
        .max_turns(5)
        .build();

    // Simulate a request from user "3856588331" on Telegram
    let ctx = RequestContext::new("3856588331")
        .with_metadata("platform", "telegram")
        .with_metadata("chat_type", "private");

    let prompt = "What's the weather in Tokyo? Also, tell me who I am.";
    println!("\n> {}\n", prompt);

    match agent.run(prompt, &[], &ctx).await {
        Ok(response) => {
            println!("\n{}\n", response.output);
            println!("Messages in history: {}", response.messages.len());
            if let Some(input) = response.usage.input_tokens {
                println!(
                    "Tokens: {} in / {} out",
                    input,
                    response.usage.output_tokens.unwrap_or(0)
                );
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
        }
    }
}
