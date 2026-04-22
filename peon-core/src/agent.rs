//! The primary entry point for the peon-lib agent framework.
//!
//! # Architecture
//!
//! Initialization cost is separated from per-request cost via two distinct types:
//!
//! - [`PeonSharedCore`] — expensive to create (fs I/O, Casbin loading, skill scan).
//!   Created **once** at startup and shared via `Arc` across all chat sessions.
//!
//! - [`ChatSession`] — per-chat state. Contains the conversation history and the
//!   `PeonEngine` (dynamic file/script path whitelist). Deep-cloned before each agent
//!   run so concurrent messages in the same chat use the last-write-wins model.
//!
//! - [`PeonAgent`] — a single fully-wired agent run. Cheap to build from a
//!   `PeonSharedCore` + `ChatSession` snapshot. Platform-specific tools
//!   (e.g., Telegram output tools) are injected at build time via `.extra_tool()`.

use crate::enforcer::{FileEnforcer, UserEnforcer};
use crate::scanner::{PeonEngine, generate_skills_xml, scan_skills};
use crate::tools::{ExecuteScriptTool, ListAllSkillsTool, ReadFileTool, ReadSkillTool};

use log::info;
use peon_runtime::message::Message;
use peon_runtime::providers::anthropic::AnthropicProvider;
use peon_runtime::providers::gemini::GeminiProvider;
use peon_runtime::providers::openai::OpenAiProvider;
use peon_runtime::{AgentLoop, AgentResponse, CompletionProvider, RequestContext};
use std::sync::Arc;

// ============================================================
// Provider factory
// ============================================================

fn create_provider() -> Arc<dyn CompletionProvider> {
    let provider = std::env::var("PROVIDER").unwrap_or_else(|_| "openai".into());
    let api_key = std::env::var("API_KEY").expect(
        "API_KEY environment variable not set. \
         Please set it in your .env file or shell environment.",
    );

    match provider.to_lowercase().as_str() {
        "openai" => {
            let model = std::env::var("MODEL").unwrap_or_else(|_| "gpt-4o-mini".into());
            info!("Provider: OpenAI ({})", model);
            Arc::new(OpenAiProvider::new(model, api_key))
        }
        "anthropic" => {
            let model =
                std::env::var("MODEL").unwrap_or_else(|_| "claude-sonnet-4-20250514".into());
            info!("Provider: Anthropic ({})", model);
            Arc::new(AnthropicProvider::new(model, api_key))
        }
        "gemini" => {
            let model = std::env::var("MODEL").unwrap_or_else(|_| "gemini-2.5-flash".into());
            info!("Provider: Gemini ({})", model);
            Arc::new(GeminiProvider::new(model, api_key))
        }
        "openrouter" => {
            let model = std::env::var("MODEL")
                .unwrap_or_else(|_| "anthropic/claude-sonnet-4-20250514".into());
            info!("Provider: OpenRouter ({})", model);
            Arc::new(OpenAiProvider::openrouter(model, api_key))
        }
        other => panic!(
            "Unsupported PROVIDER: '{}'. Supported: openai, anthropic, gemini, openrouter",
            other
        ),
    }
}

// ============================================================
// PeonSharedCore — expensive, built once, shared via Arc
// ============================================================

/// Long-lived, globally shared agent core.
///
/// Holds everything that is expensive to initialize but **stateless at runtime**:
/// the LLM provider, Casbin enforcers, and the discovered skills catalog.
///
/// Create once at application startup with [`PeonSharedCore::new()`] and wrap in `Arc`.
/// All per-chat sessions and per-request agents borrow from this shared core.
pub struct PeonSharedCore {
    provider: Arc<dyn CompletionProvider>,
    file_enforcer: Arc<FileEnforcer>,
    user_enforcer: Arc<UserEnforcer>,
    skills: Arc<Vec<crate::scanner::SkillMeta>>,
    skills_xml: String,
    system_prompt_template: Option<String>,
    max_turns: usize,
}

impl PeonSharedCore {
    /// Initialize from environment variables.
    ///
    /// Reads `PROVIDER`, `MODEL`, `API_KEY`, `PEON_SKILLS_DIR` from env.
    /// Scans the skills directory and boots the Casbin enforcers. (**Expensive — call once.**)
    pub async fn new() -> anyhow::Result<Self> {
        Self::with_provider(create_provider()).await
    }

    /// Initialize with an explicit provider (useful for testing).
    pub async fn with_provider(provider: Arc<dyn CompletionProvider>) -> anyhow::Result<Self> {
        info!("🚀 Peon shared core initializing...");

        let skills_dir = std::env::var("PEON_SKILLS_DIR").unwrap_or_else(|_| "skills".to_string());
        let skills = Arc::new(scan_skills(&skills_dir, None).await?);
        info!(
            "Discovered {} skill(s): {:?}",
            skills.len(),
            skills.iter().map(|s| &s.name).collect::<Vec<_>>()
        );

        let skills_xml = generate_skills_xml(&skills);

        let file_enforcer = FileEnforcer::new().await;
        let user_enforcer = UserEnforcer::new().await;

        info!("✅ Shared core ready.");
        Ok(Self {
            provider,
            file_enforcer,
            user_enforcer,
            skills,
            skills_xml,
            system_prompt_template: None,
            max_turns: 10,
        })
    }

    /// Apply the standard Peon system prompt (with skills XML injected).
    pub fn default_prompt(mut self) -> Self {
        let prompt = SYSTEM_PROMPT_TEMPLATE
            .replace("{skills}", &self.skills_xml)
            .replace("{custom_prompt}", "");
        self.system_prompt_template = Some(prompt);
        self
    }

    /// Append custom instructions after the standard system prompt.
    pub fn append_system_prompt(mut self, custom: &str) -> Self {
        let prompt = SYSTEM_PROMPT_TEMPLATE
            .replace("{skills}", &self.skills_xml)
            .replace("{custom_prompt}", custom);
        self.system_prompt_template = Some(prompt);
        self
    }

    /// Fully replace the system prompt template.
    /// Your template **must** contain `{skills}`.
    pub fn custom_system_prompt(mut self, template: &str, custom: Option<&str>) -> Self {
        let prompt = template
            .replace("{skills}", &self.skills_xml)
            .replace("{custom_prompt}", custom.unwrap_or(""));
        self.system_prompt_template = Some(prompt);
        self
    }

    /// Set the maximum number of agent turns per run.
    pub fn max_turns(mut self, turns: usize) -> Self {
        self.max_turns = turns;
        self
    }

    /// Create a fresh `ChatSession` for a new chat (new group, new DM, etc.).
    ///
    /// The session gets its own `PeonEngine` with empty path whitelists,
    /// sharing the enforcers from this core (static Casbin policy only).
    pub fn new_session(&self) -> ChatSession {
        let engine = Arc::new(PeonEngine::new(
            Arc::clone(&self.file_enforcer),
            Arc::clone(&self.user_enforcer),
        ));
        ChatSession {
            history: Vec::new(),
            engine,
        }
    }

    /// Build a ready-to-run `PeonAgent` from a session snapshot and optional extra tools.
    ///
    /// This is **cheap** — all Arc fields are cloned, base tools are re-created from
    /// the existing Arcs, and the session's engine is used directly.
    ///
    /// # Arguments
    /// - `session`: A snapshot of the chat's state (history + engine whitelist).
    ///   Typically obtained via `ChatSession::snapshot().await` before calling this.
    /// - `extra_tools`: Platform-specific tools (e.g., Telegram output tools) to inject.
    pub fn build_agent(
        &self,
        session: ChatSession,
        extra_tools: Vec<Box<dyn peon_runtime::PeonTool>>,
    ) -> PeonAgent {
        let engine = Arc::clone(&session.engine);
        let read_paths = Arc::clone(&engine.read_paths);
        let execute_paths = Arc::clone(&engine.execute_paths);

        let read_skill_tool = ReadSkillTool::new(Arc::clone(&self.skills), Arc::clone(&engine));
        let read_file_tool = ReadFileTool::new(
            Arc::clone(&self.file_enforcer),
            Arc::clone(&self.user_enforcer),
            Arc::clone(&read_paths),
        );
        let execute_script_tool = ExecuteScriptTool::new(
            Arc::clone(&self.file_enforcer),
            Arc::clone(&self.user_enforcer),
            Arc::clone(&execute_paths),
        );
        let list_all_skills_tool = ListAllSkillsTool::new(Arc::clone(&self.skills));

        let mut builder = AgentLoop::builder(Arc::clone(&self.provider));

        if let Some(ref prompt) = self.system_prompt_template {
            builder = builder.system_prompt(prompt);
        }

        builder = builder.max_turns(self.max_turns);

        // Base tools first
        for tool in [
            Box::new(read_skill_tool) as Box<dyn peon_runtime::PeonTool>,
            Box::new(read_file_tool),
            Box::new(execute_script_tool),
            Box::new(list_all_skills_tool),
        ] {
            builder = builder.tool_boxed(tool);
        }

        // Platform-specific tools
        for tool in extra_tools {
            builder = builder.tool_boxed(tool);
        }

        PeonAgent {
            agent: builder.build(),
            session,
        }
    }
}

// ============================================================
// ChatSession — per-chat mutable state
// ============================================================

/// All mutable state scoped to a single chat (group or DM).
///
/// Contains:
/// - `history`: the full conversation history, including tool calls and results.
/// - `engine`: the `PeonEngine` holding the dynamic file/script path whitelist,
///   which grows as skills are read and persists across messages in the same chat.
///
/// Before each agent run, call [`ChatSession::snapshot()`] to obtain an independent
/// deep copy. After the run, the returned session (from `PeonAgent::prompt()`) can
/// be stored back — last-write-wins for concurrent messages.
#[derive(Clone)]
pub struct ChatSession {
    /// Full conversation history (user, assistant, tool call, tool result).
    pub history: Vec<Message>,
    /// Per-chat `PeonEngine` with accumulated path whitelists.
    ///
    /// Stored as `Arc` so it can be passed into tools without cloning the whole session.
    /// `ChatSession::snapshot()` deep-copies the underlying HashSets to ensure isolation.
    pub engine: Arc<PeonEngine>,
}

impl ChatSession {
    /// Create an empty session with a fresh engine using the given enforcers.
    ///
    /// Prefer `PeonSharedCore::new_session()` in production.
    pub fn new(file_enforcer: Arc<FileEnforcer>, user_enforcer: Arc<UserEnforcer>) -> Self {
        Self {
            history: Vec::new(),
            engine: Arc::new(PeonEngine::new(file_enforcer, user_enforcer)),
        }
    }

    /// Create an independent deep copy of this session for use in a single agent run.
    ///
    /// - `history` is cloned (cheap — `Message` is `Clone`).
    /// - `engine` path whitelists are **copied by value** into new `Arc<RwLock<_>>`s,
    ///   so changes during the run don't affect the stored session until written back.
    pub async fn snapshot(&self) -> Self {
        Self {
            history: self.history.clone(),
            engine: Arc::new(self.engine.deep_clone().await),
        }
    }
}

// ============================================================
// PeonAgent — a single ready-to-run agent instance
// ============================================================

/// A fully-wired Peon agent for a single request execution.
///
/// Built cheaply from a [`PeonSharedCore`] + [`ChatSession`] snapshot.
///
/// After calling [`PeonAgent::prompt()`], the returned [`AgentResponse`] contains
/// the updated history and the consumed `ChatSession`. Store the session back into
/// your session store to persist the chat state.
pub struct PeonAgent {
    agent: AgentLoop<Arc<dyn CompletionProvider>>,
    session: ChatSession,
}

impl PeonAgent {
    /// Run the agent and return the response plus the updated session.
    ///
    /// Both `&str`, `String`, and `Vec<ContentPart>` implement `Into<Message>`,
    /// so this method handles text-only and multimodal input uniformly.
    ///
    /// # Arguments
    /// - `input`: The user's message (text or multimodal).
    /// - `uid`: The caller's user ID for zero-trust permission enforcement.
    ///   This value propagates into every tool call via `RequestContext`
    ///   and **cannot be forged by the LLM**.
    ///
    /// # Returns
    /// An `AgentResponse` with:
    /// - `output`: the agent's final text response.
    /// - `messages`: the full updated conversation (pass to `ChatSession::history`
    ///   when writing the session back to the store).
    ///
    /// It also returns the consumed `ChatSession` whose engine has the updated
    /// path whitelist from this run.
    pub async fn prompt(
        self,
        input: impl Into<Message>,
        uid: &str,
    ) -> anyhow::Result<(AgentResponse, ChatSession)> {
        let ctx = RequestContext::new(uid);
        let msg: Message = input.into();
        let response = self.agent.run(msg, &self.session.history, &ctx).await?;

        let updated_session = ChatSession {
            history: response.messages.clone(),
            engine: self.session.engine,
        };

        info!(
            "Agent response (uid={}): {} chars",
            uid,
            response.output.len()
        );
        Ok((response, updated_session))
    }
}

// ============================================================
// Legacy builder shim  (kept for peon-cli / tests / examples)
// ============================================================

/// Convenience builder kept for backward compatibility and single-shot use cases
/// (e.g., CLI tools that don't need session persistence).
///
/// For multi-turn chat applications, use [`PeonSharedCore`] + [`ChatSession`] directly.
pub struct PeonAgentBuilder {
    core: PeonSharedCore,
    extra_tools: Vec<Box<dyn peon_runtime::PeonTool>>,
}

impl PeonAgentBuilder {
    /// Initialize from environment variables. (**Expensive — avoid in hot paths.**)
    pub async fn new() -> anyhow::Result<Self> {
        Ok(Self {
            core: PeonSharedCore::new().await?,
            extra_tools: Vec::new(),
        })
    }

    /// Initialize with a custom provider.
    pub async fn with_provider(provider: Arc<dyn CompletionProvider>) -> anyhow::Result<Self> {
        Ok(Self {
            core: PeonSharedCore::with_provider(provider).await?,
            extra_tools: Vec::new(),
        })
    }

    pub fn default_prompt(mut self) -> Self {
        self.core = self.core.default_prompt();
        self
    }

    pub fn append_system_prompt(mut self, custom: &str) -> Self {
        self.core = self.core.append_system_prompt(custom);
        self
    }

    pub fn custom_system_prompt(mut self, template: &str, custom: Option<&str>) -> Self {
        self.core = self.core.custom_system_prompt(template, custom);
        self
    }

    pub fn preamble(mut self, preamble: &str) -> Self {
        self.core.system_prompt_template = Some(preamble.to_string());
        self
    }

    pub fn default_max_turns(mut self, turns: usize) -> Self {
        self.core = self.core.max_turns(turns);
        self
    }

    pub fn tool(mut self, tool: impl peon_runtime::PeonTool + 'static) -> Self {
        self.extra_tools.push(Box::new(tool));
        self
    }

    /// Build a `PeonAgent` with a fresh empty session (no history, clean whitelist).
    pub fn build(self) -> PeonAgent {
        let session = self.core.new_session();
        self.core.build_agent(session, self.extra_tools)
    }
}

// ============================================================
// System prompt template
// ============================================================

/// `{skills}` → discovered skills XML catalog.
/// `{custom_prompt}` → optional caller-supplied suffix.
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
