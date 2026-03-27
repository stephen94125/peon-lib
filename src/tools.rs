use crate::enforcer::FileEnforcer;
use crate::scanner::{
    PeonEngine, SharedExecutePaths, SharedReadPaths, SkillMeta, generate_skills_xml,
};
use log::{debug, error, info, warn};
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::Deserialize;
use std::path::Path;
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
    engine: Arc<PeonEngine>,
}

impl ReadSkillTool {
    pub fn new(skills: Arc<Vec<SkillMeta>>, engine: Arc<PeonEngine>) -> Self {
        Self { skills, engine }
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

        info!("Tool call: read_skill('{}')", skill.name);
        debug!("Reading SKILL.md from: {}", skill.location);

        let content = tokio::fs::read_to_string(&skill.location)
            .await
            .map_err(|e| {
                ToolCallError::new(format!(
                    "Failed to read SKILL.md at '{}': {}",
                    skill.location, e
                ))
            })?;

        debug!(
            "read_skill('{}') returned {} bytes",
            skill.name,
            content.len()
        );

        // === Populate the whitelist from this skill's content ===
        // The skill's base dir is the folder containing SKILL.md.
        let skill_base_dir = Path::new(&skill.location)
            .parent()
            .unwrap_or(Path::new("."));
        self.engine
            .process_skill_content("agent", skill_base_dir, &content)
            .await;

        Ok(content)
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
    /// Live whitelist — shared with `PeonEngine`. Definition reads it on every call.
    allowed_paths: SharedReadPaths,
}

impl ReadFileTool {
    pub fn new(enforcer: Arc<FileEnforcer>, allowed_paths: SharedReadPaths) -> Self {
        Self {
            enforcer,
            allowed_paths,
        }
    }
}

impl Tool for ReadFileTool {
    const NAME: &'static str = "read_file";
    type Error = ToolCallError;
    type Args = ReadFileArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        let guard = self.allowed_paths.read().await;
        let mut paths: Vec<String> = guard.iter().cloned().collect();
        paths.sort();
        drop(guard);

        // If whitelist is empty, send an empty enum so LLM can't invent paths.
        let enum_values: serde_json::Value = if paths.is_empty() {
            serde_json::json!([""])
        } else {
            serde_json::json!(paths)
        };

        debug!("read_file definition: {} path(s) in whitelist", paths.len());

        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Read the contents of a pre-validated file.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "enum": enum_values,
                        "description": "Exact path to read — must be one of the whitelisted paths"
                    }
                },
                "required": ["path"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        info!("Tool call: read_file('{}')", args.path);

        // === Layer 1: Hard whitelist check (cannot be bypassed by LLM) ===
        {
            let guard = self.allowed_paths.read().await;
            if !guard.contains(&args.path) {
                warn!(
                    "SECURITY VIOLATION: '{}' not in read whitelist — blocked",
                    args.path
                );
                return Err(ToolCallError::new(format!(
                    "Permission Denied: '{}' is not in the read whitelist. \
                     Only paths discovered from a skill's SKILL.md are allowed. \
                     Call read_skill first to load the relevant skill and unlock its paths.",
                    args.path
                )));
            }
        }

        // === Layer 2: Enforcer check (Casbin-ready) ===
        if !self.enforcer.enforce("agent", "read", &args.path).await {
            warn!("Read access DENIED by enforcer for: {}", args.path);
            return Err(ToolCallError::new(format!(
                "Permission Denied: The enforcer rejected read access to '{}'. \
                 Please inform the user that this file cannot be accessed due to permission policy.",
                args.path
            )));
        }

        info!("Read access granted for: {}", args.path);

        let content = tokio::fs::read_to_string(&args.path)
            .await
            .map_err(|e| ToolCallError::new(format!("Failed to read file: {}", e)))?;

        debug!("read_file('{}') returned {} bytes", args.path, content.len());
        Ok(content)
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
    /// Live whitelist — shared with `PeonEngine`. Definition reads it on every call.
    allowed_paths: SharedExecutePaths,
}

impl ExecuteScriptTool {
    pub fn new(enforcer: Arc<FileEnforcer>, allowed_paths: SharedExecutePaths) -> Self {
        Self {
            enforcer,
            allowed_paths,
        }
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
        let guard = self.allowed_paths.read().await;
        let mut paths: Vec<String> = guard.iter().cloned().collect();
        paths.sort();
        drop(guard);

        // If whitelist is empty, send an empty enum so LLM can't invent paths.
        let enum_values: serde_json::Value = if paths.is_empty() {
            serde_json::json!([""])
        } else {
            serde_json::json!(paths)
        };

        debug!(
            "execute_script definition: {} path(s) in whitelist",
            paths.len()
        );

        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Execute a pre-validated script with optional CLI arguments.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "enum": enum_values,
                        "description": "Exact script path to execute — must be one of the whitelisted paths"
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
        info!(
            "Tool call: execute_script('{}', args={:?})",
            args.path, args.arguments
        );

        // === Layer 1: Hard whitelist check (cannot be bypassed by LLM) ===
        {
            let guard = self.allowed_paths.read().await;
            if !guard.contains(&args.path) {
                warn!(
                    "SECURITY VIOLATION: '{}' not in execute whitelist — blocked",
                    args.path
                );
                return Err(ToolCallError::new(format!(
                    "Permission Denied: '{}' is not in the execute whitelist. \
                     Only script files discovered from a skill's SKILL.md are allowed. \
                     Call read_skill first to load the relevant skill and unlock its scripts.",
                    args.path
                )));
            }
        }

        // === Layer 2: Enforcer check (Casbin-ready) ===
        if !self.enforcer.enforce("agent", "execute", &args.path).await {
            warn!("Execute access DENIED by enforcer for: {}", args.path);
            return Err(ToolCallError::new(format!(
                "Permission Denied: The enforcer rejected execute access to '{}'. \
                 Please inform the user that this script cannot be run due to permission policy.",
                args.path
            )));
        }

        info!("Execute access granted for: {}", args.path);

        let (interpreter, mut interpreter_args) = Self::resolve_interpreter(&args.path);
        if let Some(user_args) = args.arguments {
            interpreter_args.extend(user_args);
        }

        debug!(
            "Resolved interpreter: '{}', full args: {:?}",
            interpreter, interpreter_args
        );

        let output = Command::new(&interpreter)
            .args(&interpreter_args)
            .output()
            .await
            .map_err(|e| {
                error!("Failed to spawn process '{}': {}", interpreter, e);
                ToolCallError::new(format!("Failed to execute script '{}': {}", args.path, e))
            })?;

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let exit_code = output.status.code().unwrap_or(-1);

        debug!(
            "Script '{}' exit_code={}, stdout_len={}, stderr_len={}",
            args.path,
            exit_code,
            stdout.len(),
            stderr.len()
        );

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
        info!(
            "Tool call: list_all_skills() — returning {} skill(s)",
            self.skills.len()
        );
        let xml = generate_skills_xml(&self.skills);
        debug!("list_all_skills output:\n{}", xml);
        Ok(xml)
    }
}
