//! The primary entry point for the peon-lib agent framework.
//!
//! [`PeonAgent`] encapsulates skill scanning, engine wiring, tool registration,
//! and system prompt generation behind a single constructor.

use crate::enforcer::{FileEnforcer, UserEnforcer};
use crate::scanner::{PeonEngine, generate_skills_xml, scan_skills};
use crate::tools::{ExecuteScriptTool, ListAllSkillsTool, ReadFileTool, ReadSkillTool};

use log::{debug, info};
use peon_runtime::providers::anthropic::AnthropicProvider;
use peon_runtime::providers::gemini::GeminiProvider;
use peon_runtime::providers::openai::OpenAiProvider;
use peon_runtime::{AgentLoop, CompletionProvider, RequestContext};
use std::sync::Arc;

/// Create a `CompletionProvider` from environment variables.
///
/// Reads `PROVIDER`, `MODEL`, `API_KEY` from the process environment
/// (typically loaded from `.env` via `dotenvy`).
///
/// # Supported providers
///
/// - `openai` (default)
/// - `anthropic`
/// - `gemini`
/// - `openrouter`
fn create_provider() -> Box<dyn CompletionProvider> {
    let provider = std::env::var("PROVIDER").unwrap_or_else(|_| "openai".into());
    let api_key = std::env::var("API_KEY").expect(
        "API_KEY environment variable not set. \
         Please set it in your .env file or shell environment.",
    );

    match provider.to_lowercase().as_str() {
        "openai" => {
            let model = std::env::var("MODEL").unwrap_or_else(|_| "gpt-4o-mini".into());
            info!("Provider: OpenAI ({})", model);
            Box::new(OpenAiProvider::new(model, api_key))
        }
        "anthropic" => {
            let model =
                std::env::var("MODEL").unwrap_or_else(|_| "claude-sonnet-4-20250514".into());
            info!("Provider: Anthropic ({})", model);
            Box::new(AnthropicProvider::new(model, api_key))
        }
        "gemini" => {
            let model = std::env::var("MODEL").unwrap_or_else(|_| "gemini-2.5-flash".into());
            info!("Provider: Gemini ({})", model);
            Box::new(GeminiProvider::new(model, api_key))
        }
        "openrouter" => {
            let model = std::env::var("MODEL")
                .unwrap_or_else(|_| "anthropic/claude-sonnet-4-20250514".into());
            info!("Provider: OpenRouter ({})", model);
            Box::new(OpenAiProvider::openrouter(model, api_key))
        }
        other => panic!(
            "Unsupported PROVIDER: '{}'. Supported: openai, anthropic, gemini, openrouter",
            other
        ),
    }
}

/// A builder to construct `PeonAgent` with custom tools and settings.
pub struct PeonAgentBuilder {
    provider: Box<dyn CompletionProvider>,
    engine: Arc<PeonEngine>,
    skills_xml: String,
    system_prompt: Option<String>,
    max_turns: usize,
    tools: Vec<Box<dyn peon_runtime::PeonTool>>,
}

impl PeonAgentBuilder {
    /// Initialize the builder with zero-trust foundations from the environment.
    ///
    /// Reads `PROVIDER`, `MODEL`, `API_KEY` from env to create the provider.
    /// Scans the skills directory and boots the Casbin enforcers.
    pub async fn new() -> anyhow::Result<Self> {
        Self::with_provider(create_provider()).await
    }

    /// Initialize with a custom provider (for testing or advanced use).
    pub async fn with_provider(provider: Box<dyn CompletionProvider>) -> anyhow::Result<Self> {
        info!("🚀 Peon agent initializing foundations...");

        // Phase 1: Boot & Discovery
        let skills_dir = std::env::var("PEON_SKILLS_DIR").unwrap_or_else(|_| "skills".to_string());
        let skills = scan_skills(&skills_dir, None).await?;
        let skills = Arc::new(skills);

        info!(
            "Discovered {} skill(s): {:?}",
            skills.len(),
            skills.iter().map(|s| &s.name).collect::<Vec<_>>()
        );

        let skills_xml = generate_skills_xml(&skills);
        debug!("Skills XML catalog:\n{}", skills_xml);

        // Phase 2: Engine + whitelists
        let file_enforcer = FileEnforcer::new().await;
        let user_enforcer = UserEnforcer::new().await;
        let engine = Arc::new(PeonEngine::new(
            Arc::clone(&file_enforcer),
            Arc::clone(&user_enforcer),
        ));
        let read_paths = Arc::clone(&engine.read_paths);
        let execute_paths = Arc::clone(&engine.execute_paths);

        // Phase 3: Base Tools
        let read_skill_tool = ReadSkillTool::new(Arc::clone(&skills), Arc::clone(&engine));
        let read_file_tool = ReadFileTool::new(
            Arc::clone(&file_enforcer),
            Arc::clone(&user_enforcer),
            Arc::clone(&read_paths),
        );
        let execute_script_tool = ExecuteScriptTool::new(
            Arc::clone(&file_enforcer),
            Arc::clone(&user_enforcer),
            Arc::clone(&execute_paths),
        );
        let list_all_skills_tool = ListAllSkillsTool::new(Arc::clone(&skills));

        let tools: Vec<Box<dyn peon_runtime::PeonTool>> = vec![
            Box::new(read_skill_tool),
            Box::new(read_file_tool),
            Box::new(execute_script_tool),
            Box::new(list_all_skills_tool),
        ];

        Ok(PeonAgentBuilder {
            provider,
            engine,
            skills_xml,
            system_prompt: None,
            max_turns: 10,
            tools,
        })
    }

    /// Apply the standard Peon system instructions, injecting the discovered skills XML catalog.
    pub fn default_prompt(mut self) -> Self {
        let system_prompt = SYSTEM_PROMPT_TEMPLATE
            .replace("{skills}", &self.skills_xml)
            .replace("{custom_prompt}", "");
        self.system_prompt = Some(system_prompt);
        self
    }

    /// Apply the standard Peon system instructions, but append your own custom instructions 
    /// to the very end of the prompt.
    pub fn append_system_prompt(mut self, custom_prompt: &str) -> Self {
        let system_prompt = SYSTEM_PROMPT_TEMPLATE
            .replace("{skills}", &self.skills_xml)
            .replace("{custom_prompt}", custom_prompt);
        self.system_prompt = Some(system_prompt);
        self
    }

    /// Completely replace the base system prompt template with your own string.
    /// Your template string **MUST** contain the `{skills}` placeholder to allow Peon 
    /// to inject the dynamically discovered capabilities XML. 
    /// You should also include `{custom_prompt}` if you intend to append custom data later.
    pub fn custom_system_prompt(mut self, template: &str, custom_prompt: Option<&str>) -> Self {
        let system_prompt = template
            .replace("{skills}", &self.skills_xml)
            .replace("{custom_prompt}", custom_prompt.unwrap_or(""));
        self.system_prompt = Some(system_prompt);
        self
    }

    /// Provide a custom system prompt string or preamble.
    pub fn preamble(mut self, preamble: &str) -> Self {
        self.system_prompt = Some(preamble.to_string());
        self
    }

    /// Set the default maximum number of chain-of-thought turns.
    pub fn default_max_turns(mut self, turns: usize) -> Self {
        self.max_turns = turns;
        self
    }

    /// Add a custom tool to the agent.
    pub fn tool(mut self, tool: impl peon_runtime::PeonTool + 'static) -> Self {
        self.tools.push(Box::new(tool));
        self
    }

    /// Finalize and build the PeonAgent.
    pub fn build(self) -> PeonAgent {
        info!("Agent ready.");

        let mut builder = AgentLoop::builder(self.provider);

        if let Some(prompt) = self.system_prompt {
            builder = builder.system_prompt(&prompt);
        }

        builder = builder.max_turns(self.max_turns);

        for tool in self.tools {
            builder = builder.tool_boxed(tool);
        }

        PeonAgent {
            agent: builder.build(),
            engine: self.engine,
        }
    }
}

/// A fully-wired Peon agent with zero-trust tool security.
///
/// Encapsulates skill discovery, whitelist management, tool registration,
/// and system prompt generation. The caller only needs to provide
/// provider credentials and a prompt.
///
/// # Example
/// ```rust,ignore
/// let agent = PeonAgentBuilder::new().await?.default_prompt().build();
/// let response = agent.prompt("Roll a 20-sided die", "user_123").await?;
/// println!("{}", response);
/// ```
pub struct PeonAgent {
    agent: AgentLoop<Box<dyn CompletionProvider>>,
    engine: Arc<PeonEngine>,
}

impl PeonAgent {
    /// Send a prompt and get a text response.
    ///
    /// The agent will autonomously discover skills, read instructions,
    /// and execute scripts as needed, all governed by the whitelist security model.
    ///
    /// # Arguments
    /// - `input`: The user's message.
    /// - `uid`: The user ID for zero-trust identity enforcement. This is passed
    ///   directly to every tool call via `RequestContext` — it cannot be forged by the LLM.
    pub async fn prompt(&self, input: &str, uid: &str) -> anyhow::Result<String> {
        info!("User input (uid={}): {}", uid, input);
        let ctx = RequestContext::new(uid);
        let response = self.agent.run(input, &[], &ctx).await?;
        info!("Agent response: {}", response.output);
        Ok(response.output)
    }

    /// Reset the session, clearing all whitelists.
    ///
    /// Call between conversations to ensure a clean security state.
    pub async fn reset_session(&self) {
        self.engine.reset_session().await;
    }
}

/// The default system prompt template.
/// `{skills}` is replaced with the `<available_skills>` XML catalog.
/// `{custom_prompt}` is replaced by any appended prompt string.
const SYSTEM_PROMPT_TEMPLATE: &str = r#"You are Peon, a powerful and versatile local agent executor.

**Your Execution Process:**
1. **Skill Exploration**: Check the `<available_skills>` tags below to find a matching skill.
2. **Skill Loading**: Call `read_skill` (passing the skill name) to get instructions and unlock allowed file/script paths.
3. **Task Execution**: Use `read_file` or `execute_script` with paths exactly as specified in the skill instructions.

**Security Rules — read carefully:**
- `read_file` and `execute_script` only accept paths that were explicitly listed in a skill's SKILL.md.
- If a tool returns **USER_PERMISSION_DENIED**, it means the user's role lacks personnel permissions to perform this action. Inform the user clearly about this lack of personnel access.
- If a tool returns **FILE_PERMISSION_DENIED**, it means the action violates the system's file-level policy. Inform the user clearly that this specific path/script is locked down.
- Do NOT attempt alternative paths or workarounds if permission is denied.
- Never fabricate results. If you cannot execute a required step, say so honestly.

{skills}

{custom_prompt}"#;
