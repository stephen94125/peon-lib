use crate::enforcer::{FileEnforcer, UserEnforcer};
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
use tokio::task_local;

task_local! {
    /// The User ID used for security enforcement by Peon tools in the current tokio task.
    /// If not explicitly set via `CURRENT_UID.scope(...)`, tools will default to `"agent"`.
    pub static CURRENT_UID: String;
}

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
    file_enforcer: Arc<FileEnforcer>,
    user_enforcer: Arc<UserEnforcer>,
    /// Live whitelist — shared with `PeonEngine`. Definition reads it on every call.
    allowed_paths: SharedReadPaths,
}

impl ReadFileTool {
    pub fn new(
        file_enforcer: Arc<FileEnforcer>,
        user_enforcer: Arc<UserEnforcer>,
        allowed_paths: SharedReadPaths,
    ) -> Self {
        Self {
            file_enforcer,
            user_enforcer,
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
        // Using double-check approach: Ask UserEnforcer, then FileEnforcer.
        // Dynamically grab the user ID bound to the current tokio task, or default to "agent"
        let current_uid = CURRENT_UID
            .try_with(|uid| uid.clone())
            .unwrap_or_else(|_| "agent".to_string());

        let user_ok = self
            .user_enforcer
            .enforce(&current_uid, "read", &args.path)
            .await;
        if !user_ok {
            warn!("Read access DENIED by USER enforcer for: {}", args.path);
            return Err(ToolCallError::new(format!(
                "USER_PERMISSION_DENIED: The user enforcer rejected read access to '{}'. \
                 Please inform the user that their current role lacks permission for this action.",
                args.path
            )));
        }

        let file_ok = self
            .file_enforcer
            .enforce("agent", "read", &args.path)
            .await;
        if !file_ok {
            warn!("Read access DENIED by FILE enforcer for: {}", args.path);
            return Err(ToolCallError::new(format!(
                "FILE_PERMISSION_DENIED: The file enforcer rejected read access to '{}'. \
                 Please inform the user that this file cannot be accessed due to system permission policies.",
                args.path
            )));
        }

        info!("Read access granted for: {}", args.path);

        let content = tokio::fs::read_to_string(&args.path)
            .await
            .map_err(|e| ToolCallError::new(format!("Failed to read file: {}", e)))?;

        debug!(
            "read_file('{}') returned {} bytes",
            args.path,
            content.len()
        );
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
    file_enforcer: Arc<FileEnforcer>,
    user_enforcer: Arc<UserEnforcer>,
    /// Live whitelist — shared with `PeonEngine`. Definition reads it on every call.
    allowed_paths: SharedExecutePaths,
}

impl ExecuteScriptTool {
    pub fn new(
        file_enforcer: Arc<FileEnforcer>,
        user_enforcer: Arc<UserEnforcer>,
        allowed_paths: SharedExecutePaths,
    ) -> Self {
        Self {
            file_enforcer,
            user_enforcer,
            allowed_paths,
        }
    }

    /// Determines the interpreter to use based on the file extension.
    ///
    /// Supported extensions: `.sh` (bash), `.py` (python3), `.js` (node),
    /// `.rb` (ruby), `.ts` (npx tsx).
    ///
    /// Unknown extensions → executed directly as a native binary or shebang script.
    /// The OS will use the shebang line (`#!/usr/bin/env ...`) if present,
    /// or treat the file as a compiled ELF/binary. This is intentional and
    /// still fully gated by the whitelist.
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
            // No known extension: run as native binary or shebang script.
            // Equivalent to `./path` in a shell — the OS decides how to exec it.
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
        // Using double-check approach: Ask UserEnforcer, then FileEnforcer.
        // Dynamically grab the user ID bound to the current tokio task, or default to "agent"
        let current_uid = CURRENT_UID
            .try_with(|uid| uid.clone())
            .unwrap_or_else(|_| "agent".to_string());

        let user_ok = self
            .user_enforcer
            .enforce(&current_uid, "execute", &args.path)
            .await;
        if !user_ok {
            warn!("Execute access DENIED by USER enforcer for: {}", args.path);
            return Err(ToolCallError::new(format!(
                "USER_PERMISSION_DENIED: The user enforcer rejected execute access to '{}'. \
                 Please inform the user that their current role lacks permission for this action.",
                args.path
            )));
        }

        let file_ok = self
            .file_enforcer
            .enforce("agent", "execute", &args.path)
            .await;
        if !file_ok {
            warn!("Execute access DENIED by FILE enforcer for: {}", args.path);
            return Err(ToolCallError::new(format!(
                "FILE_PERMISSION_DENIED: The file enforcer rejected execute access to '{}'. \
                 Please inform the user that this script cannot be run due to system permission policies.",
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::{PeonEngine, SkillMeta};
    use rig::tool::Tool;
    use std::sync::Arc;
    use tokio::fs as tfs;

    // ========================================
    // resolve_interpreter unit tests
    // ========================================

    #[test]
    fn test_resolve_interpreter_sh() {
        let (cmd, args) = ExecuteScriptTool::resolve_interpreter("scripts/run.sh");
        assert_eq!(cmd, "bash");
        assert_eq!(args, vec!["scripts/run.sh"]);
    }

    #[test]
    fn test_resolve_interpreter_py() {
        let (cmd, args) = ExecuteScriptTool::resolve_interpreter("./tools/analyze.py");
        assert_eq!(cmd, "python3");
        assert_eq!(args, vec!["./tools/analyze.py"]);
    }

    #[test]
    fn test_resolve_interpreter_js() {
        let (cmd, args) = ExecuteScriptTool::resolve_interpreter("index.js");
        assert_eq!(cmd, "node");
        assert_eq!(args, vec!["index.js"]);
    }

    #[test]
    fn test_resolve_interpreter_ts() {
        let (cmd, args) = ExecuteScriptTool::resolve_interpreter("src/main.ts");
        assert_eq!(cmd, "npx");
        assert_eq!(args, vec!["tsx", "src/main.ts"]);
    }

    #[test]
    fn test_resolve_interpreter_rb() {
        let (cmd, args) = ExecuteScriptTool::resolve_interpreter("scripts/deploy.rb");
        assert_eq!(cmd, "ruby");
        assert_eq!(args, vec!["scripts/deploy.rb"]);
    }

    #[test]
    fn test_resolve_interpreter_unknown_falls_through_to_direct_exec() {
        let (cmd, args) = ExecuteScriptTool::resolve_interpreter("./mybinary");
        assert_eq!(cmd, "./mybinary", "unknown ext should use path as command");
        assert!(args.is_empty(), "no interpreter args for direct exec");
    }

    // ========================================
    // Whitelist security tests (CRITICAL)
    // ========================================

    #[tokio::test]
    async fn test_read_file_rejects_path_not_in_whitelist() {
        let file_enforcer = FileEnforcer::new().await;
        let user_enforcer = UserEnforcer::new().await;
        let read_paths: SharedReadPaths =
            Arc::new(tokio::sync::RwLock::new(std::collections::HashSet::new()));

        let tool = ReadFileTool::new(
            Arc::clone(&file_enforcer),
            Arc::clone(&user_enforcer),
            Arc::clone(&read_paths),
        );

        let result = tool
            .call(ReadFileArgs {
                path: "/etc/passwd".to_string(),
            })
            .await;

        assert!(result.is_err(), "unwhitelisted path must be rejected");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("Permission Denied"),
            "error must say Permission Denied, got: {}",
            err_msg
        );
    }

    #[tokio::test]
    async fn test_execute_script_rejects_path_not_in_whitelist() {
        let file_enforcer = FileEnforcer::new().await;
        let user_enforcer = UserEnforcer::new().await;
        let execute_paths: SharedExecutePaths =
            Arc::new(tokio::sync::RwLock::new(std::collections::HashSet::new()));

        let tool = ExecuteScriptTool::new(
            Arc::clone(&file_enforcer),
            Arc::clone(&user_enforcer),
            Arc::clone(&execute_paths),
        );

        let result = tool
            .call(ExecuteScriptArgs {
                path: "/bin/sh".to_string(),
                arguments: None,
            })
            .await;

        assert!(result.is_err(), "unwhitelisted script must be rejected");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("Permission Denied"),
            "error must say Permission Denied, got: {}",
            err_msg
        );
    }

    #[tokio::test]
    async fn test_read_file_accepts_whitelisted_path() {
        let tmp = tempfile::tempdir().unwrap();
        let file_path = tmp.path().join("allowed.txt");
        tfs::write(&file_path, "hello from allowed file")
            .await
            .unwrap();

        let resolved = file_path
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .to_string();

        let file_enforcer = FileEnforcer::new().await;
        let user_enforcer = UserEnforcer::new().await;
        let read_paths: SharedReadPaths =
            Arc::new(tokio::sync::RwLock::new(std::collections::HashSet::new()));

        // Insert into whitelist
        read_paths.write().await.insert(resolved.clone());

        let tool = ReadFileTool::new(
            Arc::clone(&file_enforcer),
            Arc::clone(&user_enforcer),
            Arc::clone(&read_paths),
        );

        let result = tool.call(ReadFileArgs { path: resolved }).await;

        assert!(result.is_ok(), "whitelisted path must succeed");
        assert_eq!(result.unwrap(), "hello from allowed file");
    }

    #[tokio::test]
    async fn test_execute_script_accepts_whitelisted_path() {
        let tmp = tempfile::tempdir().unwrap();
        let script_path = tmp.path().join("test.sh");
        tfs::write(&script_path, "#!/bin/bash\necho OK")
            .await
            .unwrap();

        // Make executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }

        let resolved = script_path
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .to_string();

        let file_enforcer = FileEnforcer::new().await;
        let user_enforcer = UserEnforcer::new().await;
        let execute_paths: SharedExecutePaths =
            Arc::new(tokio::sync::RwLock::new(std::collections::HashSet::new()));

        execute_paths.write().await.insert(resolved.clone());

        let tool = ExecuteScriptTool::new(
            Arc::clone(&file_enforcer),
            Arc::clone(&user_enforcer),
            Arc::clone(&execute_paths),
        );

        let result = tool
            .call(ExecuteScriptArgs {
                path: resolved,
                arguments: None,
            })
            .await;

        assert!(result.is_ok(), "whitelisted script must execute");
        let output = result.unwrap();
        assert!(output.contains("OK"), "output must contain script stdout");
    }

    // ========================================
    // read_skill tool tests
    // ========================================

    #[tokio::test]
    async fn test_read_skill_unknown_name_returns_error() {
        let skills = Arc::new(vec![SkillMeta {
            name: "roll-dice".to_string(),
            description: "Roll dice.".to_string(),
            location: "/tmp/roll-dice/SKILL.md".to_string(),
        }]);
        let file_enforcer = FileEnforcer::new().await;
        let user_enforcer = UserEnforcer::new().await;
        let engine = Arc::new(PeonEngine::new(
            Arc::clone(&file_enforcer),
            Arc::clone(&user_enforcer),
        ));

        let tool = ReadSkillTool::new(skills, engine);

        let result = tool
            .call(ReadSkillArgs {
                skill_name: "nonexistent-skill".to_string(),
            })
            .await;

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("not found"),
            "error must mention 'not found', got: {}",
            err_msg
        );
    }

    // ========================================
    // list_all_skills tool tests
    // ========================================

    #[tokio::test]
    async fn test_list_all_skills_returns_xml() {
        let skills = Arc::new(vec![
            SkillMeta {
                name: "deploy".to_string(),
                description: "Deploy app.".to_string(),
                location: "/tmp/deploy/SKILL.md".to_string(),
            },
            SkillMeta {
                name: "rollback".to_string(),
                description: "Rollback app.".to_string(),
                location: "/tmp/rollback/SKILL.md".to_string(),
            },
        ]);

        let tool = ListAllSkillsTool::new(skills);

        let result = tool.call(ListAllSkillsArgs {}).await;
        assert!(result.is_ok());
        let xml = result.unwrap();
        assert!(xml.contains("<available_skills>"));
        assert!(xml.contains("deploy"));
        assert!(xml.contains("rollback"));
        assert!(xml.contains("</available_skills>"));
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;
    use std::sync::Arc;

    proptest! {
        /// Any random path must be rejected by `read_file` when whitelist is empty.
        #[test]
        fn read_file_rejects_any_random_path(path in "\\PC+") {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let file_enforcer = FileEnforcer::new().await;
                let user_enforcer = UserEnforcer::new().await;
                let read_paths: SharedReadPaths = Arc::new(
                    tokio::sync::RwLock::new(std::collections::HashSet::new()),
                );
                let tool = ReadFileTool::new(file_enforcer, user_enforcer, read_paths);
                let result = tool.call(ReadFileArgs { path: path.clone() }).await;
                prop_assert!(
                    result.is_err(),
                    "random path '{}' must be rejected by empty whitelist", path
                );
                Ok(())
            })?;
        }

        /// Any random path must be rejected by `execute_script` when whitelist is empty.
        #[test]
        fn execute_script_rejects_any_random_path(path in "\\PC+") {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let file_enforcer = FileEnforcer::new().await;
                let user_enforcer = UserEnforcer::new().await;
                let execute_paths: SharedExecutePaths = Arc::new(
                    tokio::sync::RwLock::new(std::collections::HashSet::new()),
                );
                let tool = ExecuteScriptTool::new(file_enforcer, user_enforcer, execute_paths);
                let result = tool.call(ExecuteScriptArgs {
                    path: path.clone(),
                    arguments: None,
                }).await;
                prop_assert!(
                    result.is_err(),
                    "random path '{}' must be rejected by empty whitelist", path
                );
                Ok(())
            })?;
        }
    }
}
