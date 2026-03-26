//! Example: A fully wired Peon agent with skill scanning and tool registration.
//!
//! Run with: `RUST_LOG=info cargo run --example agent`
//! For maximum detail: `RUST_LOG=debug cargo run --example agent`

use peon_lib::any_model::AnyModel;
use peon_lib::enforcer::FileEnforcer;
use peon_lib::scanner::{PeonEngine, generate_skills_xml, scan_skills};
use peon_lib::tools::{ExecuteScriptTool, ListAllSkillsTool, ReadFileTool, ReadSkillTool};

use rig::completion::Prompt;
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logger — reads RUST_LOG env var.
    // Consumers can swap env_logger for android_logger, console_log (WASM), etc.
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    log::info!("🚀 Peon agent starting up...");

    // ================================================================
    // Phase 1: Boot & Discovery — scan .skills/ for available skills
    // ================================================================
    let skills_dir = ".skills";
    let skills = scan_skills(skills_dir, None).await?;
    let skills = Arc::new(skills);

    log::info!(
        "Discovered {} skill(s): {:?}",
        skills.len(),
        skills.iter().map(|s| &s.name).collect::<Vec<_>>()
    );

    // Generate the <available_skills> XML for the system prompt
    let skills_xml = generate_skills_xml(&skills);
    log::debug!("Skills XML catalog:\n{}", skills_xml);

    // ================================================================
    // Phase 2: Create PeonEngine — source of truth for path whitelists
    // ================================================================
    let enforcer = FileEnforcer::new();
    let engine = Arc::new(PeonEngine::new(Arc::clone(&enforcer)));

    // Share the live whitelists with the tools.
    // PeonEngine populates them lazily as read_skill() is called per-session.
    let read_paths = Arc::clone(&engine.read_paths);
    let execute_paths = Arc::clone(&engine.execute_paths);

    // ================================================================
    // Create model from environment
    // ================================================================
    let model = AnyModel::from_env();

    // ================================================================
    // Build tools — ReadFileTool and ExecuteScriptTool hold shared refs
    // to the whitelists so their definition() always reflects live state.
    // ================================================================
    let read_skill_tool = ReadSkillTool::new(Arc::clone(&skills), Arc::clone(&engine));
    let read_file_tool = ReadFileTool::new(Arc::clone(&enforcer), Arc::clone(&read_paths));
    let execute_script_tool =
        ExecuteScriptTool::new(Arc::clone(&enforcer), Arc::clone(&execute_paths));
    let list_all_skills_tool = ListAllSkillsTool::new(Arc::clone(&skills));

    // ================================================================
    // Build the system prompt with injected skill catalog
    // ================================================================
    let system_prompt = format!(
        r#"You are Peon, a powerful and versatile local agent executor.

**Your Execution Process:**
1. **Skill Exploration**: Check the `<available_skills>` tags below to find a matching skill.
2. **Skill Loading**: Call `read_skill` (passing the skill name) to get instructions and unlock allowed file/script paths.
3. **Task Execution**: Use `read_file` or `execute_script` with paths exactly as specified in the skill instructions.

**Security Rules — read carefully:**
- `read_file` and `execute_script` only accept paths that were explicitly listed in a skill's SKILL.md.
- If a tool returns a **Permission Denied** error, it means the path is not whitelisted. Do NOT attempt alternative paths or workarounds.
- When a permission error occurs, tell the user clearly: what was attempted, that it was blocked by the security policy, and suggest they contact an administrator if they believe this is a mistake.
- Never fabricate results. If you cannot execute a required step, say so honestly.

{}
"#,
        skills_xml
    );

    // ================================================================
    // Wire up the agent with all four tools
    // ================================================================
    let agent = model
        .agent()
        .preamble(&system_prompt)
        .default_max_turns(10) // Allow up to 10 rounds of tool calls (discovery -> execution)
        .tool(read_skill_tool)
        .tool(read_file_tool)
        .tool(execute_script_tool)
        .tool(list_all_skills_tool)
        .build();

    log::info!("Agent ready. Sending prompt...");

    // ================================================================
    // Send a test prompt
    // ================================================================
    let user_input = "Roll a 20-sided die";
    log::info!("User input: {}", user_input);

    let response = agent.prompt(user_input).await?;
    log::info!("Agent response: {}", response);

    // Example: reset session between conversations
    engine.reset_session().await;

    Ok(())
}
