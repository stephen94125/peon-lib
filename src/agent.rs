//! The primary entry point for the peon-lib agent framework.
//!
//! [`PeonAgent`] encapsulates skill scanning, engine wiring, tool registration,
//! and system prompt generation behind a single constructor.

use crate::enforcer::FileEnforcer;
use crate::peon_model::PeonModel;
use crate::scanner::{PeonEngine, generate_skills_xml, scan_skills};
use crate::tools::{ExecuteScriptTool, ListAllSkillsTool, ReadFileTool, ReadSkillTool};

use log::{debug, info};
use rig::agent::Agent;
use rig::completion::Prompt;
use std::sync::Arc;

/// A fully-wired Peon agent with zero-trust tool security.
///
/// Encapsulates skill discovery, whitelist management, tool registration,
/// and system prompt generation. The caller only needs to provide
/// provider credentials and a prompt.
///
/// # Example
/// ```rust,ignore
/// let agent = PeonAgent::new().await?;
/// let response = agent.prompt("Roll a 20-sided die").await?;
/// println!("{}", response);
/// ```
pub struct PeonAgent {
    agent: Agent<PeonModel>,
    engine: Arc<PeonEngine>,
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
- If a tool returns a **Permission Denied** error, it means the path is not whitelisted. Do NOT attempt alternative paths or workarounds.
- When a permission error occurs, tell the user clearly: what was attempted, that it was blocked by the security policy, and suggest they contact an administrator if they believe this is a mistake.
- Never fabricate results. If you cannot execute a required step, say so honestly.

{}"#;

impl PeonAgent {
    /// Build a fully-wired agent.
    pub async fn new() -> anyhow::Result<Self> {
        Self::build_with_model(PeonModel::from_env()).await
    }

    /// Internal: wire up scanning, engine, tools, system prompt, and build the agent.
    async fn build_with_model(model: PeonModel) -> anyhow::Result<Self> {
        info!("🚀 Peon agent starting up...");

        // Phase 1: Boot & Discovery
        let skills_dir = ".skills";
        let skills = scan_skills(skills_dir, None).await?;
        let skills = Arc::new(skills);

        info!(
            "Discovered {} skill(s): {:?}",
            skills.len(),
            skills.iter().map(|s| &s.name).collect::<Vec<_>>()
        );

        let skills_xml = generate_skills_xml(&skills);
        debug!("Skills XML catalog:\n{}", skills_xml);

        // Phase 2: Engine + whitelists
        let enforcer = FileEnforcer::new();
        let engine = Arc::new(PeonEngine::new(Arc::clone(&enforcer)));
        let read_paths = Arc::clone(&engine.read_paths);
        let execute_paths = Arc::clone(&engine.execute_paths);

        // Phase 3: Tools
        let read_skill_tool = ReadSkillTool::new(Arc::clone(&skills), Arc::clone(&engine));
        let read_file_tool = ReadFileTool::new(Arc::clone(&enforcer), Arc::clone(&read_paths));
        let execute_script_tool =
            ExecuteScriptTool::new(Arc::clone(&enforcer), Arc::clone(&execute_paths));
        let list_all_skills_tool = ListAllSkillsTool::new(Arc::clone(&skills));

        // Phase 4: System prompt + build
        let system_prompt = SYSTEM_PROMPT_TEMPLATE.replace("{}", &skills_xml);

        let agent = model
            .agent()
            .preamble(&system_prompt)
            .default_max_turns(10)
            .tool(read_skill_tool)
            .tool(read_file_tool)
            .tool(execute_script_tool)
            .tool(list_all_skills_tool)
            .build();

        info!("Agent ready.");

        Ok(Self { agent, engine })
    }

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
