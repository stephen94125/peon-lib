mod any_model;
mod enforcer;
mod tools;
mod scanner;

use any_model::AnyModel;
use rig::completion::Prompt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // ================================================================
    // Usage 1: Create model and agent directly via AnyModel
    // ================================================================
    let model = AnyModel::from_env(); // reads DEFAULT_PROVIDER + DEFAULT_MODEL

    let system_prompt = String::from("
        You are OpenFang, a powerful and versatile local agent executor.

        **Your Core Responsibilities:**
        1. Select and execute the appropriate skills to complete tasks based on the user's intent.
        2. Ensure a safe execution process; do not randomly guess unknown file paths.
        3. If a task can be answered directly with text and does not require operating the system, return the text directly without calling any tools.

        **Your Execution Process:**
        1. **Skill Exploration**: Check the `<available_skills>` tags below to find if there is a skill that matches the task requirements.
        2. **Skill Loading**: If a suitable skill is found, you 【MUST】 prioritize calling the `Skill` tool (passing the skill name) to obtain the detailed execution steps and script locations for that skill.
        3. **Task Execution**: After carefully reading the skill instructions, use tools like `bash` and `read_file` to strictly follow the instructions. If the instructions include requirements for recursive execution, follow them.
    ");

    let agent = model
        .agent()
        .preamble(&system_prompt)
        .build();

    let response = agent.prompt("你是什麼模型？").await?;
    println!("Agent 回覆: {response}");

    // ================================================================
    // Usage 2: Use multiple models from different providers simultaneously
    // ================================================================
    // let openai_model = AnyModel::new("openai", "gpt-4o");
    // let gemini_model = AnyModel::new("gemini", "gemini-2.5-flash");
    // let anthropic_model = AnyModel::new("anthropic", "claude-sonnet-4-20250514");
    //
    // // All three models exist simultaneously; their types are all AnyModel
    // let models: Vec<AnyModel> = vec![openai_model, gemini_model, anthropic_model];
    // for m in &models {
    //     let resp = m.prompt("Say hello in one word.").await?;
    //     println!("{resp}");
    // }

    Ok(())
}
