use crate::enforcer::FileEnforcer;
use crate::scanner::{SkillMeta, generate_skills_xml};
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::Deserialize;
use std::sync::Arc;
use tokio::process::Command;

// ==========================================
// Shared error type for all tools
// ==========================================
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct ToolCallError(String);

impl ToolCallError {
    fn new(msg: impl Into<String>) -> Self {
        Self(msg.into())
    }
}

// ==========================================
// 1. Read Skill Tool (The Discovery Layer)
// ==========================================
#[derive(Deserialize)]
pub struct ReadSkillArgs {
    pub skill_name: String,
}

pub struct ReadSkillTool {
    skills: Arc<Vec<SkillMeta>>,
}

impl ReadSkillTool {
    pub fn new(skills: Arc<Vec<SkillMeta>>) -> Self {
        Self { skills }
    }
}

impl Tool for ReadSkillTool {
    const NAME: &'static str = "read_skill";
    type Error = ToolCallError;
    type Args = ReadSkillArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        let skill_names: Vec<String> = self.skills.iter().map(|s| s.name.clone()).collect();
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Read a skill's SKILL.md to get its instructions and available resources."
                .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "skill_name": {
                        "type": "string",
                        "enum": skill_names,
                        "description": "Name of the skill to read"
                    }
                },
                "required": ["skill_name"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let skill = self
            .skills
            .iter()
            .find(|s| s.name == args.skill_name)
            .ok_or_else(|| {
                ToolCallError::new(format!(
                    "Skill '{}' not found. Use list_all_skills to see available skills.",
                    args.skill_name
                ))
            })?;

        println!("📖 [ReadSkill] Reading skill: {}", skill.name);

        tokio::fs::read_to_string(&skill.location)
            .await
            .map_err(|e| {
                ToolCallError::new(format!(
                    "Failed to read SKILL.md at '{}': {}",
                    skill.location, e
                ))
            })
    }
}

// ==========================================
// 2. Read File Tool (The Information Layer)
// ==========================================
#[derive(Deserialize)]
pub struct ReadFileArgs {
    pub path: String,
}

pub struct ReadFileTool {
    enforcer: Arc<FileEnforcer>,
}

impl ReadFileTool {
    pub fn new(enforcer: Arc<FileEnforcer>) -> Self {
        Self { enforcer }
    }
}

impl Tool for ReadFileTool {
    const NAME: &'static str = "read_file";
    type Error = ToolCallError;
    type Args = ReadFileArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Read the contents of a pre-validated file.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file to read"
                    }
                },
                "required": ["path"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        // Enforce 'read' action
        if !self.enforcer.enforce("agent", "read", &args.path).await {
            return Err(ToolCallError::new(format!(
                "Security Violation: Read access to '{}' denied.",
                args.path
            )));
        }

        tokio::fs::read_to_string(&args.path)
            .await
            .map_err(|e| ToolCallError::new(format!("Failed to read file: {}", e)))
    }
}

// ==========================================
// 3. Execute Script Tool (The Action Layer)
// ==========================================
#[derive(Deserialize)]
pub struct ExecuteScriptArgs {
    pub path: String,
    pub arguments: Option<Vec<String>>,
}

pub struct ExecuteScriptTool {
    enforcer: Arc<FileEnforcer>,
}

impl ExecuteScriptTool {
    pub fn new(enforcer: Arc<FileEnforcer>) -> Self {
        Self { enforcer }
    }

    /// Determines the interpreter to use based on the file extension.
    /// Falls back to running the script directly (relies on shebang).
    fn resolve_interpreter(path: &str) -> (String, Vec<String>) {
        if path.ends_with(".py") {
            ("python3".to_string(), vec![path.to_string()])
        } else if path.ends_with(".js") {
            ("node".to_string(), vec![path.to_string()])
        } else if path.ends_with(".sh") {
            ("bash".to_string(), vec![path.to_string()])
        } else if path.ends_with(".rb") {
            ("ruby".to_string(), vec![path.to_string()])
        } else if path.ends_with(".ts") {
            ("npx".to_string(), vec!["tsx".to_string(), path.to_string()])
        } else {
            // Fallback: execute directly, relying on shebang
            (path.to_string(), vec![])
        }
    }
}

impl Tool for ExecuteScriptTool {
    const NAME: &'static str = "execute_script";
    type Error = ToolCallError;
    type Args = ExecuteScriptArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Execute a pre-validated script with optional CLI arguments.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the script to execute"
                    },
                    "arguments": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Optional CLI arguments to pass to the script"
                    }
                },
                "required": ["path"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        // Enforce 'execute' action
        if !self.enforcer.enforce("agent", "execute", &args.path).await {
            return Err(ToolCallError::new(format!(
                "Security Violation: Execution of '{}' denied.",
                args.path
            )));
        }

        let (interpreter, mut interpreter_args) = Self::resolve_interpreter(&args.path);
        // Append user-provided arguments
        if let Some(user_args) = args.arguments {
            interpreter_args.extend(user_args);
        }

        println!(
            "⚡ [ExecuteScript] Running: {} {}",
            interpreter,
            interpreter_args.join(" ")
        );

        let output = Command::new(interpreter)
            .args(&interpreter_args)
            .output()
            .await
            .map_err(|e| {
                ToolCallError::new(format!("Failed to execute script '{}': {}", args.path, e))
            })?;

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let exit_code = output.status.code().unwrap_or(-1);

        let mut result = format!("Exit Code: {}\n", exit_code);
        if !stdout.is_empty() {
            result.push_str(&format!("--- STDOUT ---\n{}\n", stdout));
        }
        if !stderr.is_empty() {
            result.push_str(&format!("--- STDERR ---\n{}\n", stderr));
        }
        if stdout.is_empty() && stderr.is_empty() {
            result.push_str("Script executed silently with no output.");
        }

        Ok(result)
    }
}

// ==========================================
// 4. List All Skills Tool (Discovery Helper)
// ==========================================
#[derive(Deserialize)]
pub struct ListAllSkillsArgs {}

pub struct ListAllSkillsTool {
    skills: Arc<Vec<SkillMeta>>,
}

impl ListAllSkillsTool {
    pub fn new(skills: Arc<Vec<SkillMeta>>) -> Self {
        Self { skills }
    }
}

impl Tool for ListAllSkillsTool {
    const NAME: &'static str = "list_all_skills";
    type Error = ToolCallError;
    type Args = ListAllSkillsArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "List all available skills with their names, descriptions, and locations."
                .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        }
    }

    async fn call(&self, _args: Self::Args) -> Result<Self::Output, Self::Error> {
        println!(
            "📋 [ListAllSkills] Returning skill catalog ({} skills)",
            self.skills.len()
        );
        Ok(generate_skills_xml(&self.skills))
    }
}
