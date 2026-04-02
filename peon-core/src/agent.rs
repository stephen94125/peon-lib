//! The primary entry point for the peon-lib agent framework.
//!
//! [`PeonAgent`] encapsulates skill scanning, engine wiring, tool registration,
//! and system prompt generation behind a single constructor.

use crate::enforcer::{FileEnforcer, UserEnforcer};
use crate::peon_model::PeonModel;
use crate::scanner::{PeonEngine, generate_skills_xml, scan_skills};
use crate::tools::{ExecuteScriptTool, ListAllSkillsTool, ReadFileTool, ReadSkillTool};

use log::{debug, info};
use rig::agent::Agent;
use rig::completion::Prompt;
use std::sync::Arc;

/// A builder to construct `PeonAgent` with custom tools and settings.
pub struct PeonAgentBuilder<T = rig::agent::WithBuilderTools> {
    builder: rig::agent::AgentBuilder<PeonModel, (), T>,
    engine: Arc<PeonEngine>,
    skills_xml: String,
}

impl PeonAgentBuilder<rig::agent::NoToolConfig> {
    /// Initialize the builder with zero-trust foundations from the environment.
    pub async fn new() -> anyhow::Result<PeonAgentBuilder<rig::agent::WithBuilderTools>> {
        Self::with_model(PeonModel::from_env()).await
    }

    /// Internal: wire up scanning, engine, tools, but wait for prompt configuration.
    pub async fn with_model(
        model: PeonModel,
    ) -> anyhow::Result<PeonAgentBuilder<rig::agent::WithBuilderTools>> {
        info!("🚀 Peon agent initializing foundations...");

        // Phase 1: Boot & Discovery
        let skills_dir = std::env::var("PEON_SKILLS_DIR").unwrap_or_else(|_| ".skills".to_string());
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

        // The builder initializes with NoToolConfig, and after calling `.tool()`,
        // it upgrades its type state to `WithBuilderTools`.
        let builder = model
            .agent()
            .default_max_turns(10)
            .tool(read_skill_tool)
            .tool(read_file_tool)
            .tool(execute_script_tool)
            .tool(list_all_skills_tool);

        Ok(PeonAgentBuilder {
            builder,
            engine,
            skills_xml,
        })
    }
}

impl PeonAgentBuilder<rig::agent::WithBuilderTools> {
    /// Apply the standard Peon system instructions, injecting the discovered skills XML catalog.
    pub fn default_prompt(self) -> Self {
        let system_prompt = SYSTEM_PROMPT_TEMPLATE.replace("{}", &self.skills_xml);
        Self {
            builder: self.builder.preamble(&system_prompt),
            engine: self.engine,
            skills_xml: self.skills_xml,
        }
    }

    /// Provide a custom system prompt string or preamble.
    pub fn preamble(self, preamble: &str) -> Self {
        Self {
            builder: self.builder.preamble(preamble),
            engine: self.engine,
            skills_xml: self.skills_xml,
        }
    }

    /// Set the default maximum number of chain-of-thought turns.
    pub fn default_max_turns(self, turns: usize) -> Self {
        Self {
            builder: self.builder.default_max_turns(turns),
            engine: self.engine,
            skills_xml: self.skills_xml,
        }
    }

    /// Add a custom tool to the agent.
    pub fn tool<NewTool: rig::tool::Tool + 'static>(
        self,
        tool: NewTool,
    ) -> PeonAgentBuilder<rig::agent::WithBuilderTools> {
        PeonAgentBuilder {
            builder: self.builder.tool(tool),
            engine: self.engine,
            skills_xml: self.skills_xml,
        }
    }

    /// Finalize and build the PeonAgent.
    pub fn build(self) -> PeonAgent {
        info!("Agent ready.");
        PeonAgent {
            agent: self.builder.build(),
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
/// let response = agent.prompt("Roll a 20-sided die").await?;
/// println!("{}", response);
/// ```
pub struct PeonAgent {
    agent: Agent<PeonModel>,
    engine: Arc<PeonEngine>,
}

impl PeonAgent {
    /// Send a prompt and get a text response.
    ///
    /// The agent will autonomously discover skills, read instructions,
    /// and execute scripts as needed, all governed by the whitelist security model.
    pub async fn prompt(&self, input: &str) -> anyhow::Result<String> {
        info!("User input: {}", input);
        let response = self.agent.prompt(input).await?;
        info!("Agent response: {}", response);
        Ok(response)
    }

    /// Reset the session, clearing all whitelists.
    ///
    /// Call between conversations to ensure a clean security state.
    pub async fn reset_session(&self) {
        self.engine.reset_session().await;
    }
}

/// The default system prompt template.
/// `{}` is replaced with the `<available_skills>` XML catalog.
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

{}"#;
