use crate::enforcer::FileEnforcer;
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
// 1. Read File Tool (The 'R' in RWX)
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
            description: "Read the contents of a file at the given path.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Absolute or relative path to the file to read"
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
// 2. Write File Tool (The 'W' in RWX)
// ==========================================
#[derive(Deserialize)]
pub struct WriteFileArgs {
    pub path: String,
    pub content: String,
}

pub struct WriteFileTool {
    enforcer: Arc<FileEnforcer>,
}

impl WriteFileTool {
    pub fn new(enforcer: Arc<FileEnforcer>) -> Self {
        Self { enforcer }
    }
}

impl Tool for WriteFileTool {
    const NAME: &'static str = "write_file";
    type Error = ToolCallError;
    type Args = WriteFileArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Write content to a file at the given path.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Absolute or relative path to the file to write"
                    },
                    "content": {
                        "type": "string",
                        "description": "The content to write into the file"
                    }
                },
                "required": ["path", "content"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        // Enforce 'write' action
        if !self.enforcer.enforce("agent", "write", &args.path).await {
            return Err(ToolCallError::new(format!(
                "Security Violation: Write access to '{}' denied.",
                args.path
            )));
        }

        tokio::fs::write(&args.path, args.content)
            .await
            .map_err(|e| ToolCallError::new(format!("Failed to write file: {}", e)))?;

        Ok(format!("Successfully wrote to {}", args.path))
    }
}

// ==========================================
// 3. Bash Tool (The 'X' in RWX)
// ==========================================
#[derive(Deserialize)]
pub struct BashArgs {
    pub command: String,
}

pub struct BashTool {
    enforcer: Arc<FileEnforcer>,
}

impl BashTool {
    pub fn new(enforcer: Arc<FileEnforcer>) -> Self {
        Self { enforcer }
    }
}

impl Tool for BashTool {
    const NAME: &'static str = "bash";
    type Error = ToolCallError;
    type Args = BashArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Execute a bash command and return its stdout, stderr, and exit code."
                .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The bash command string to execute"
                    }
                },
                "required": ["command"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        // Enforce 'execute' action
        if !self
            .enforcer
            .enforce("agent", "execute", &args.command)
            .await
        {
            return Err(ToolCallError::new(format!(
                "Security Violation: Execution of '{}' denied.",
                args.command
            )));
        }

        println!("⚡ [Bash] Executing: {}", args.command);

        // Spawn child process capturing stdout and stderr
        let output = Command::new("bash")
            .arg("-c")
            .arg(&args.command)
            .output()
            .await
            .map_err(|e| ToolCallError::new(format!("Failed to spawn bash process: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let exit_code = output.status.code().unwrap_or(-1);

        // Combine into a structured response for the LLM
        let mut result = format!("Exit Code: {}\n", exit_code);

        if !stdout.is_empty() {
            result.push_str(&format!("--- STDOUT ---\n{}\n", stdout));
        }
        if !stderr.is_empty() {
            result.push_str(&format!("--- STDERR ---\n{}\n", stderr));
        }

        // If both are empty, tell the LLM it succeeded silently
        if stdout.is_empty() && stderr.is_empty() {
            result.push_str("Command executed silently with no output.");
        }

        Ok(result)
    }
}
