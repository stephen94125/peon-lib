//! Example: A fully wired Peon agent with skill scanning and tool registration.
//!
//! Run with: `RUST_LOG=info cargo run --example agent`
//! For maximum detail: `RUST_LOG=debug cargo run --example agent`

use peon_lib::any_model::AnyModel;
use peon_lib::enforcer::FileEnforcer;
use peon_lib::scanner::{generate_skills_xml, scan_skills};
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
    // Create model from environment
    // ================================================================
    let model = AnyModel::from_env();

    // ================================================================
    // Build enforcer + tools
    // ================================================================
    let enforcer = FileEnforcer::new();

    let read_skill_tool = ReadSkillTool::new(Arc::clone(&skills));
    let read_file_tool = ReadFileTool::new(Arc::clone(&enforcer));
    let execute_script_tool = ExecuteScriptTool::new(Arc::clone(&enforcer));
    let list_all_skills_tool = ListAllSkillsTool::new(Arc::clone(&skills));

    // ================================================================
    // Build the system prompt with injected skill catalog
    // ================================================================
    let system_prompt = format!(
        r#"You are Peon, a powerful and versatile local agent executor.

**Your Core Responsibilities:**
1. Select and execute the appropriate skills to complete tasks based on the user's intent.
2. Ensure a safe execution process; do not randomly guess unknown file paths.
3. If a task can be answered directly with text and does not require operating the system, return the text directly without calling any tools.

**Your Execution Process:**
1. **Skill Exploration**: Check the `<available_skills>` tags below to find a skill that matches the task requirements.
2. **Skill Loading**: If a suitable skill is found, you MUST call the `read_skill` tool (passing the skill name) to obtain instructions.
3. **Task Execution**: After reading the skill instructions, use `read_file` or `execute_script` to follow them.

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
    let user_input = "幫我擲一顆 20 面骰";
    log::info!("User input: {}", user_input);

    let response = agent.prompt(user_input).await?;
    log::info!("Agent response: {}", response);

    Ok(())
}
