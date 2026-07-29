use crate::agent::{Agent, build_project_context};
use crate::config::CustomProviderConfig;
use crate::model::{ModelInfo, lookup_known_context_window};
use crate::session::Role;
use crate::session::manager::SessionManager;

use super::dto::*;

use tracing;

use tiktoken_rs::{bpe_for_model, o200k_base_singleton};

// ---------------------------------------------------------------------------
// BimoApi — the public interface
// ---------------------------------------------------------------------------

/// The top-level API object. Wrap this in any transport (HTTP, gRPC, stdin, etc.).
pub struct BimoApi {
    pub agent: Agent,
    pub(crate) session_manager: SessionManager,
}

impl Default for BimoApi {
    fn default() -> Self {
        Self::new()
    }
}

impl BimoApi {
    pub fn new() -> Self {
        tracing::info!("initializing BimoApi");
        let agent = Agent::new();
        let session_manager = SessionManager::new(agent.session.clone());
        Self {
            agent,
            session_manager,
        }
    }

    /// Convenience constructor from an existing agent (useful for testing).
    pub fn from_agent(agent: Agent) -> Self {
        let session_manager = SessionManager::new(agent.session.clone());
        Self {
            agent,
            session_manager,
        }
    }

    // -----------------------------------------------------------------------
    // Session management
    // -----------------------------------------------------------------------

    /// Create a new session and set it as active.
    /// Persists the new session to disk immediately.
    pub fn create_session(&mut self) -> ApiResponse {
        tracing::info!("create_session called");

        // Save the current active session back to the pool first
        self.sync_active_to_pool();

        // Create new session with system prompt
        let mut new_session = crate::session::Session::new();
        let tool_xml = self.agent.tool_registry.render_tool_xml();
        let now = chrono::Local::now().format("%Y-%m-%d").to_string();
        let cwd = std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "<unknown>".into());
        let project_context = build_project_context(&cwd);
        let system_prompt = crate::prompts::render(
            &crate::prompts::load(crate::prompts::SYSTEM),
            &[
                ("TOOLS", &tool_xml),
                ("DATE", &now),
                ("CWD", &cwd),
                ("PROJECT_CONTEXT", &project_context.rendered),
            ],
        );
        new_session.add_system_message(&system_prompt);

        let id = new_session.id.clone();

        // Persist to disk
        if let Err(e) = new_session.save() {
            tracing::error!(error = %e, "failed to persist new session");
        }

        // Insert into pool and set as active
        self.session_manager.insert(new_session);
        let _ = self.session_manager.set_active(&id);

        // Load into agent
        if let Some(session) = self.session_manager.active() {
            self.agent.session = session.clone();
        }

        tracing::info!(session_id = %id, "create_session done");
        ApiResponse::ok(CreateSessionData { session_id: id })
    }

    /// List all sessions in the pool.
    pub fn list_sessions(&self) -> ApiResponse {
        tracing::debug!("list_sessions called");
        let sessions = self.session_manager.list();
        let active_id = self.session_manager.active_id().to_string();
        tracing::debug!(count = sessions.len(), "list_sessions done");
        ApiResponse::ok(SessionListData {
            sessions,
            active_session_id: active_id,
            context: current_session_context(),
        })
    }

    /// Switch to a different session by id.
    pub fn switch_session(&mut self, req: SwitchSessionRequest) -> ApiResponse {
        tracing::info!(session_id = %req.session_id, "switch_session called");

        // Save current active session back to pool
        self.sync_active_to_pool();

        // Find the session — first try the pool, then try loading from disk
        let target_id = req.session_id.clone();
        let found_in_pool = self.session_manager.get(&target_id).is_some();

        if !found_in_pool {
            // Try loading from disk
            match crate::session::Session::load(&target_id) {
                Ok(loaded) => {
                    self.session_manager.insert(loaded);
                }
                Err(e) => {
                    tracing::warn!(error = %e, session_id = %target_id, "session not found");
                    return ApiResponse::err(crate::error::BimoError::Session(format!(
                        "session '{}' not found",
                        target_id
                    )));
                }
            }
        }

        // Switch active id
        if let Err(e) = self.session_manager.set_active(&target_id) {
            return ApiResponse::err(e);
        }

        // Load the target session into the agent
        if let Some(session) = self.session_manager.active() {
            self.agent.session = session.clone();
        }

        let message_count = self.agent.session.message_count();
        tracing::info!(
            session_id = %target_id,
            message_count,
            "switch_session done"
        );
        ApiResponse::ok(SessionData {
            session_id: target_id,
            messages: self.agent.session.messages.clone(),
            message_count,
            context: current_session_context(),
        })
    }

    /// Delete a session from the pool (and from disk).
    /// Cannot delete the active session.
    pub fn delete_session_from_pool(&mut self, session_id: &str) -> ApiResponse {
        tracing::info!(session_id, "delete_session_from_pool called");

        // Remove from disk first
        if let Err(e) = crate::session::Session::delete_saved(session_id) {
            tracing::warn!(error = %e, "failed to delete from disk (may not exist)");
        }

        // Remove from pool
        match self.session_manager.remove(session_id) {
            Ok(_) => {
                tracing::info!(session_id, "session deleted from pool");
                ApiResponse::ok(serde_json::Value::Null)
            }
            Err(e) => ApiResponse::err(e),
        }
    }

    /// Get a specific session by id (from pool or disk).
    pub fn get_session_by_id(&self, session_id: &str) -> ApiResponse {
        tracing::debug!(session_id, "get_session_by_id called");

        let context = current_session_context();

        // Check the pool first
        if let Some(session) = self.session_manager.get(session_id) {
            let data = SessionData {
                session_id: session.id.clone(),
                messages: session.messages.clone(),
                message_count: session.message_count(),
                context: context.clone(),
            };
            return ApiResponse::ok(data);
        }

        // Try loading from disk
        match crate::session::Session::load(session_id) {
            Ok(session) => {
                let data = SessionData {
                    session_id: session.id.clone(),
                    messages: session.messages.clone(),
                    message_count: session.message_count(),
                    context: context.clone(),
                };
                ApiResponse::ok(data)
            }
            Err(e) => ApiResponse::err(e),
        }
    }

    /// Save the current agent session back to the pool.
    pub fn sync_active_to_pool(&mut self) {
        let id = self.agent.session.id.clone();
        self.session_manager.insert(self.agent.session.clone());
        let _ = self.session_manager.set_active(&id);
    }

    /// Switch to a session by id, loading from pool or disk.
    /// Returns Ok(()) on success, or an error if the session is not found.
    pub fn activate_session(&mut self, sid: &str) -> crate::error::Result<()> {
        if sid != self.agent.session.id {
            self.sync_active_to_pool();
            if self.session_manager.get(sid).is_none() {
                match crate::session::Session::load(sid) {
                    Ok(loaded) => {
                        self.session_manager.insert(loaded);
                    }
                    Err(e) => {
                        return Err(crate::error::BimoError::Session(format!(
                            "session '{sid}' not found: {e}"
                        )));
                    }
                }
            }
            self.session_manager.set_active(sid)?;
            if let Some(session) = self.session_manager.active() {
                self.agent.session = session.clone();
            }
        }
        Ok(())
    }

    /// Persist the active session to disk.
    pub fn persist_active_session(&self) {
        if let Err(e) = self.session_manager.save_active() {
            tracing::error!(error = %e, "failed to persist session");
        }
    }

    /// List all active sessions.
    pub fn list_sessions_data(&self) -> SessionListData {
        SessionListData {
            sessions: self.session_manager.list(),
            active_session_id: self.session_manager.active_id().to_string(),
            context: current_session_context(),
        }
    }

    // -----------------------------------------------------------------------
    // Provider endpoints
    // -----------------------------------------------------------------------

    pub fn list_providers(&self) -> ApiResponse {
        tracing::debug!("list_providers called");
        let resp = ApiResponse::ok(self.agent.list_providers());
        tracing::debug!(
            count = self.agent.list_providers().len(),
            "list_providers done"
        );
        resp
    }

    pub async fn select_provider(&mut self, req: SelectProviderRequest) -> ApiResponse {
        tracing::info!(provider_id = %req.provider_id, "select_provider called");
        match self.agent.select_provider(&req.provider_id).await {
            Ok(info) => {
                tracing::info!(provider = %info.name, "select_provider success");
                ApiResponse::ok(info)
            }
            Err(e) => {
                tracing::error!(error = %e, "select_provider failed");
                ApiResponse::err(e)
            }
        }
    }

    pub fn configure_provider(&mut self, req: ConfigureProviderRequest) -> ApiResponse {
        tracing::info!(provider_id = %req.provider_id, "configure_provider called");
        match self
            .agent
            .configure_provider(&req.provider_id, req.base_url, req.api_key)
        {
            Ok(()) => {
                tracing::info!("configure_provider success");
                ApiResponse::ok(serde_json::Value::Null)
            }
            Err(e) => {
                tracing::error!(error = %e, "configure_provider failed");
                ApiResponse::err(e)
            }
        }
    }

    pub fn add_custom_provider(&mut self, req: AddCustomProviderRequest) -> ApiResponse {
        tracing::info!(id = %req.id, name = %req.name, "add_custom_provider called");
        let cp = CustomProviderConfig {
            id: req.id,
            name: req.name,
            category: req.category,
            base_url: req.base_url,
            api_key_required: req.api_key_required,
            chat_endpoint: req.chat_endpoint,
            models_endpoint: req.models_endpoint,
            auth_header: req.auth_header,
            auth_prefix: req.auth_prefix,
        };
        match self.agent.add_custom_provider(cp) {
            Ok(()) => {
                tracing::info!("add_custom_provider success");
                ApiResponse::ok(serde_json::Value::Null)
            }
            Err(e) => {
                tracing::error!(error = %e, "add_custom_provider failed");
                ApiResponse::err(e)
            }
        }
    }

    // -----------------------------------------------------------------------
    // Model endpoints
    // -----------------------------------------------------------------------

    pub async fn list_models(&mut self) -> ApiResponse {
        tracing::debug!("list_models called");
        if self.agent.list_models().is_empty() && self.agent.runtime.is_some() {
            tracing::debug!("model list empty, fetching from provider");
            let _ = self.agent.fetch_models().await;
        }
        let count = self.agent.list_models().len();
        tracing::debug!(count, "list_models done");
        ApiResponse::ok(self.agent.list_models().to_vec())
    }

    pub fn select_model(&mut self, req: SelectModelRequest) -> ApiResponse {
        tracing::info!(model_id = %req.model_id, "select_model called");
        match self.agent.select_model(&req.model_id) {
            Ok(()) => {
                tracing::info!("select_model success");
                ApiResponse::ok(serde_json::Value::Null)
            }
            Err(e) => {
                tracing::error!(error = %e, "select_model failed");
                ApiResponse::err(e)
            }
        }
    }

    // -----------------------------------------------------------------------
    // Chat
    // -----------------------------------------------------------------------

    pub async fn chat(&mut self, req: ChatRequest) -> ApiResponse {
        tracing::info!(message_len = req.message.len(), "chat called");
        match self.agent.chat(&req.message).await {
            Ok(resp) => {
                tracing::info!(
                    model = ?resp.model,
                    content_len = resp.content.len(),
                    "chat success"
                );
                ApiResponse::ok(ChatData {
                    content: resp.content,
                    model: resp.model,
                    usage: resp.usage,
                    session_id: resp.session_id,
                })
            }
            Err(e) => {
                tracing::error!(error = %e, "chat failed");
                ApiResponse::err(e)
            }
        }
    }

    // -----------------------------------------------------------------------
    // Session
    // -----------------------------------------------------------------------

    pub fn get_session(&self) -> ApiResponse {
        tracing::debug!("get_session called");
        let data = SessionData {
            session_id: self.agent.session.id.clone(),
            messages: self.agent.session.messages.clone(),
            message_count: self.agent.session.message_count(),
            context: current_session_context(),
        };
        tracing::debug!(message_count = data.message_count, "get_session done");
        ApiResponse::ok(data)
    }

    pub fn clear_session(&mut self) -> ApiResponse {
        tracing::info!("clear_session called");
        self.agent.clear_session();
        // Sync the cleared session back to the pool
        self.sync_active_to_pool();
        if let Err(e) = self.session_manager.save_active() {
            tracing::error!(error = %e, "failed to persist cleared session");
        }
        tracing::info!("clear_session done");
        ApiResponse::ok(serde_json::Value::Null)
    }

    // -----------------------------------------------------------------------
    // Commands
    // -----------------------------------------------------------------------

    pub async fn execute_command(&mut self, req: CommandRequest) -> ApiResponse {
        let input = if req.command.starts_with('/') {
            req.command
        } else {
            format!("/{}", req.command)
        };

        tracing::info!(command = %input, "execute_command called");
        let active_id = self.session_manager.active_id().to_string();
        let all_sessions = self.session_manager.list();

        match self
            .agent
            .execute_command(&input, &active_id, &all_sessions)
            .await
        {
            Ok(result) => {
                tracing::info!(command = %result.command, output_len = result.output.len(), "execute_command success");

                // Handle session switch: check if this was a switch command
                if result.command == "session"
                    && let Some(data) = &result.data
                    && data.get("action").and_then(|v| v.as_str()) == Some("switch")
                    && let Some(sid) = data.get("session_id").and_then(|v| v.as_str())
                {
                    self.sync_active_to_pool();
                    if self.session_manager.get(sid).is_none()
                        && let Ok(loaded) = crate::session::Session::load(sid)
                    {
                        self.session_manager.insert(loaded);
                    }
                    if let Err(e) = self.session_manager.set_active(sid) {
                        return ApiResponse::err(e);
                    }
                    if let Some(session) = self.session_manager.active() {
                        self.agent.session = session.clone();
                    }
                }

                // Handle session resume: same as switch
                if result.command == "session"
                    && let Some(data) = &result.data
                    && data.get("action").and_then(|v| v.as_str()) == Some("resume")
                    && let Some(sid) = data.get("session_id").and_then(|v| v.as_str())
                {
                    self.sync_active_to_pool();
                    if self.session_manager.get(sid).is_none()
                        && let Ok(loaded) = crate::session::Session::load(sid)
                    {
                        self.session_manager.insert(loaded);
                    }
                    if let Err(e) = self.session_manager.set_active(sid) {
                        return ApiResponse::err(e);
                    }
                    if let Some(session) = self.session_manager.active() {
                        self.agent.session = session.clone();
                    }
                }

                // Handle tree fork: insert new forked session into pool and set active
                if result.command == "tree"
                    && let Some(data) = &result.data
                    && let Some(action) = data.get("action").and_then(|v| v.as_str())
                    && action == "fork"
                    && let Some(new_id) = data.get("new_session_id").and_then(|v| v.as_str())
                {
                    self.session_manager.insert(self.agent.session.clone());
                    let _ = self.session_manager.set_active(new_id);
                    self.persist_active_session();
                }

                // Handle tree revert: sync modified session back to pool
                if result.command == "tree"
                    && let Some(data) = &result.data
                    && let Some(action) = data.get("action").and_then(|v| v.as_str())
                    && action == "revert"
                {
                    self.sync_active_to_pool();
                    self.persist_active_session();
                }

                ApiResponse::ok(result)
            }
            Err(e) => {
                tracing::error!(error = %e, "execute_command failed");
                ApiResponse::err(e)
            }
        }
    }

    // -----------------------------------------------------------------------
    // Status & help
    // -----------------------------------------------------------------------

    pub fn status(&self) -> ApiResponse {
        tracing::debug!("status called");
        let data = StatusData {
            provider: self.agent.config.selected_provider.clone(),
            model: self.agent.config.selected_model.clone(),
            session_id: self.agent.session.id.clone(),
            active_session_id: self.session_manager.active_id().to_string(),
            session_count: self.session_manager.list().len(),
            message_count: self.agent.session.message_count(),
            needs_configuration: self.agent.needs_configuration(),
            context: current_session_context(),
        };
        tracing::debug!(provider = ?data.provider, model = ?data.model, "status done");
        ApiResponse::ok(data)
    }

    pub fn help(&self) -> ApiResponse {
        tracing::debug!("help called");
        let commands: Vec<CommandHelpEntry> = self
            .agent
            .command_registry
            .list()
            .into_iter()
            .map(|(name, desc)| CommandHelpEntry {
                name: name.to_string(),
                description: desc.to_string(),
            })
            .collect();
        tracing::debug!(count = commands.len(), "help done");
        ApiResponse::ok(HelpData { commands })
    }

    /// Return full command metadata for client autocompletion.
    pub fn list_commands(&self) -> ApiResponse {
        tracing::debug!("list_commands called");
        let commands = self.agent.command_registry.list_detailed();
        tracing::debug!(count = commands.len(), "list_commands done");
        ApiResponse::ok(CommandsData { commands })
    }

    // -----------------------------------------------------------------------
    // Context & thinking
    // -----------------------------------------------------------------------

    /// Return the full session context with token estimates.
    pub fn get_context(&self) -> ApiResponse {
        tracing::debug!("get_context called");
        let messages: Vec<ContextMessage> = self
            .agent
            .session
            .messages
            .iter()
            .map(|m| {
                let chars = m.content.len();
                let tokens =
                    estimate_tokens(&m.content, self.agent.config.selected_model.as_deref());
                let role = match m.role {
                    Role::System => "system",
                    Role::User => "user",
                    Role::Assistant => "assistant",
                    Role::Tool => "tool",
                };
                ContextMessage {
                    role: role.to_string(),
                    content: m.content.clone(),
                    characters: chars,
                    estimated_tokens: tokens,
                }
            })
            .collect();

        let total_chars: usize = messages.iter().map(|m| m.characters).sum();
        let total_tokens: usize = messages.iter().map(|m| m.estimated_tokens).sum();
        let max_tokens = estimate_max_context(
            &self.agent.config.selected_model,
            &self.agent.available_models,
        );
        let remaining = max_tokens.saturating_sub(total_tokens);
        let usage = if max_tokens > 0 {
            (total_tokens as f64 / max_tokens as f64) * 100.0
        } else {
            0.0
        };

        tracing::debug!(
            messages = messages.len(),
            total_tokens,
            max_tokens,
            "get_context done"
        );
        ApiResponse::ok(ContextData {
            session_id: self.agent.session.id.clone(),
            messages,
            total_characters: total_chars,
            estimated_tokens: total_tokens,
            max_context_tokens: max_tokens,
            remaining_tokens: remaining,
            usage_percentage: (usage * 100.0).round() / 100.0,
        })
    }

    /// Return the current thinking configuration.
    pub fn get_thinking(&self) -> ApiResponse {
        tracing::debug!("get_thinking called");
        let thinking = &self.agent.config.thinking;
        ApiResponse::ok(ThinkingData {
            enabled: thinking.enabled,
            budget_tokens: thinking.budget_tokens,
            reasoning_effort: thinking.reasoning_effort.clone(),
        })
    }
}

// ---------------------------------------------------------------------------
// Context estimation helpers
// ---------------------------------------------------------------------------

/// Build a SessionContext from the current project environment.
fn current_session_context() -> crate::session::SessionContext {
    let cwd = std::env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "<unknown>".into());
    let ctx = build_project_context(&cwd);
    crate::session::SessionContext {
        cwd,
        git_branch: ctx.git_branch,
        agent_instructions: ctx.agent_instruction_files,
    }
}

/// Estimate the number of tokens in text using the model's tokenizer.
///
/// Uses `tiktoken-rs` to select the correct BPE tokenizer for the given model.
/// Falls back to `o200k_base` (GPT-4o / o-series) when the model is unknown.
pub fn estimate_tokens(text: &str, model: Option<&str>) -> usize {
    let bpe = match model.and_then(|m| bpe_for_model(m).ok()) {
        Some(bpe) => bpe,
        None => o200k_base_singleton(),
    };
    bpe.encode_with_special_tokens(text).len()
}

/// Estimate the max context window for a model.
///
/// Priority:
/// 1. Stored `context_window` from the fetched model metadata (provider API).
/// 2. Static lookup table of known model ids / family patterns.
/// 3. Sensible default (128K).
fn estimate_max_context(model: &Option<String>, available_models: &[ModelInfo]) -> usize {
    let model_id = match model {
        Some(id) => id,
        None => return 128_000,
    };

    // 1. Stored metadata from provider API
    if let Some(info) = available_models.iter().find(|m| m.id == *model_id)
        && let Some(ctx) = info.context_window
    {
        return ctx as usize;
    }

    // 2. Static lookup
    lookup_known_context_window(model_id).unwrap_or(128_000) as usize
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::Role;

    #[test]
    fn list_providers_returns_success() {
        let api = BimoApi::new();
        let resp = api.list_providers();
        assert!(resp.success);
        assert!(resp.data.is_some());
        let data = resp.data.unwrap();
        let providers = data.as_array().unwrap();
        assert_eq!(providers.len(), 7);
    }

    #[test]
    fn status_returns_current_state() {
        let api = BimoApi::new();
        let resp = api.status();
        assert!(resp.success);
        let data = resp.data.unwrap();
        assert!(data.get("session_id").is_some());
        assert!(data.get("active_session_id").is_some());
        assert!(data.get("session_count").is_some());
        assert!(data.get("message_count").is_some());
        assert_eq!(data["message_count"], 1);
        assert_eq!(data["session_count"], 1);
    }

    #[test]
    fn help_returns_commands() {
        let api = BimoApi::new();
        let resp = api.help();
        assert!(resp.success);
        let data = resp.data.unwrap();
        let commands = data.get("commands").unwrap().as_array().unwrap();
        assert!(!commands.is_empty());
    }

    #[test]
    fn list_commands_returns_metadata() {
        let api = BimoApi::new();
        let resp = api.list_commands();
        assert!(resp.success);
        let data = resp.data.unwrap();
        let commands = data.get("commands").unwrap().as_array().unwrap();
        assert!(!commands.is_empty());
        for cmd in commands {
            assert!(cmd.get("name").is_some());
            assert!(cmd.get("description").is_some());
        }
    }

    #[test]
    fn clear_session_resets_messages() {
        let mut api = BimoApi::new();
        assert_eq!(api.agent.session.message_count(), 1);
        api.agent.session.add_user_message("test");
        assert_eq!(api.agent.session.message_count(), 2);

        let resp = api.clear_session();
        assert!(resp.success);
        assert_eq!(api.agent.session.message_count(), 0);
    }

    #[test]
    fn get_session_returns_data() {
        let mut api = BimoApi::new();
        api.agent.session.add_user_message("hello");

        let resp = api.get_session();
        assert!(resp.success);
        let data = resp.data.unwrap();
        assert_eq!(data["message_count"], 2);
        let messages = data.get("messages").unwrap().as_array().unwrap();
        assert_eq!(messages.len(), 2);
    }

    #[test]
    fn execute_command_prepends_slash() {
        let mut api = BimoApi::new();
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let resp = api
                .execute_command(CommandRequest {
                    command: "help".into(),
                })
                .await;
            assert!(resp.success);
        });
    }

    #[test]
    fn execute_command_with_slash() {
        let mut api = BimoApi::new();
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let resp = api
                .execute_command(CommandRequest {
                    command: "/status".into(),
                })
                .await;
            assert!(resp.success);
        });
    }

    #[test]
    fn select_model_without_provider_succeeds_when_models_empty() {
        let mut api = BimoApi::new();
        let resp = api.select_model(SelectModelRequest {
            model_id: "test-model".into(),
        });
        assert!(resp.success);
        assert_eq!(
            api.agent.config.selected_model.as_deref(),
            Some("test-model")
        );
    }

    #[test]
    fn session_starts_with_system_prompt() {
        let api = BimoApi::new();
        let session = &api.agent.session;
        assert_eq!(session.message_count(), 1);
        assert_eq!(session.messages[0].role, Role::System);
        assert!(session.messages[0].content.contains("Bimo"));
        assert!(session.messages[0].content.contains("read_file"));
        assert!(session.messages[0].content.contains("write_file"));
    }

    #[test]
    fn get_context_returns_data() {
        let mut api = BimoApi::new();
        api.agent.session.add_user_message("hello world");

        let resp = api.get_context();
        assert!(resp.success);
        let data = resp.data.unwrap();
        assert!(data.get("session_id").is_some());
        assert!(data.get("messages").is_some());
        assert!(data.get("estimated_tokens").is_some());
        assert!(data.get("max_context_tokens").is_some());
        assert!(data.get("remaining_tokens").is_some());
        assert!(data.get("usage_percentage").is_some());
        let messages = data["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0]["role"], "system");
        assert_eq!(messages[1]["role"], "user");
    }

    #[test]
    fn get_context_token_estimates() {
        let api = BimoApi::new();
        let resp = api.get_context();
        let data = resp.data.unwrap();
        let total_tokens = data["estimated_tokens"].as_u64().unwrap();
        let max_tokens = data["max_context_tokens"].as_u64().unwrap();
        assert!(total_tokens > 0);
        assert!(max_tokens > 0);
        assert!(max_tokens >= total_tokens);
        assert!(data["remaining_tokens"].as_u64().unwrap() > 0);
    }

    #[test]
    fn get_thinking_returns_defaults() {
        let api = BimoApi::new();
        let resp = api.get_thinking();
        assert!(resp.success);
        let data = resp.data.unwrap();
        assert_eq!(data["enabled"], false);
        assert!(data.get("budget_tokens").is_none() || data["budget_tokens"].is_null());
        assert!(data.get("reasoning_effort").is_none() || data["reasoning_effort"].is_null());
    }

    #[test]
    fn get_thinking_after_toggle() {
        let mut api = BimoApi::new();
        api.agent.config.thinking.enabled = true;
        api.agent.config.thinking.reasoning_effort = Some("high".into());

        let resp = api.get_thinking();
        let data = resp.data.unwrap();
        assert_eq!(data["enabled"], true);
        assert_eq!(data["reasoning_effort"], "high");
    }

    #[test]
    fn estimate_tokens_basic() {
        let count = |s: &str| estimate_tokens(s, None);
        assert_eq!(count(""), 0);
        assert!(count("ab") > 0);
        assert!(count("abcdefgh") > 0);
        assert!(count("hello world") > 0);
    }

    #[test]
    fn estimate_tokens_longer_text() {
        let count = |s: &str| estimate_tokens(s, None);
        // Longer texts should have more tokens than shorter ones
        assert!(count("hello") < count("hello world this is a longer text"));
        // Non-ASCII text should produce tokens
        assert!(count("你好世界") > 0);
        assert!(count("🚀") > 0);
    }

    #[test]
    fn estimate_tokens_uses_model_tokenizer() {
        // Different models may use different tokenizers;
        // verify the function doesn't panic with any model name
        let text = "hello world";
        assert!(estimate_tokens(text, Some("gpt-4o")) > 0);
        assert!(estimate_tokens(text, Some("gpt-4")) > 0);
        assert!(estimate_tokens(text, Some("o1")) > 0);
        assert!(estimate_tokens(text, Some("unknown-model")) > 0);
        assert!(estimate_tokens(text, None) > 0);
    }

    #[test]
    fn estimate_max_context_models() {
        let empty = &[];

        assert_eq!(estimate_max_context(&Some("gpt-4".into()), empty), 8_192);
        assert_eq!(estimate_max_context(&Some("gpt-4o".into()), empty), 128_000);
        assert_eq!(
            estimate_max_context(&Some("gpt-4-turbo".into()), empty),
            128_000
        );
        assert_eq!(
            estimate_max_context(&Some("gpt-4.1-nano".into()), empty),
            1_000_000
        );
        assert_eq!(
            estimate_max_context(&Some("gpt-4.5-preview".into()), empty),
            128_000
        );
        assert_eq!(
            estimate_max_context(&Some("o3-mini".into()), empty),
            200_000
        );
        assert_eq!(
            estimate_max_context(&Some("o4-mini".into()), empty),
            200_000
        );
        assert_eq!(estimate_max_context(&Some("o1".into()), empty), 200_000);
        assert_eq!(
            estimate_max_context(&Some("claude-sonnet-4-20250514".into()), empty),
            200_000
        );
        assert_eq!(
            estimate_max_context(&Some("claude-3-opus".into()), empty),
            200_000
        );
        assert_eq!(
            estimate_max_context(&Some("claude-2".into()), empty),
            100_000
        );
        assert_eq!(
            estimate_max_context(&Some("gemini-2.5-pro".into()), empty),
            1_000_000
        );
        assert_eq!(
            estimate_max_context(&Some("gemini-1.5-pro".into()), empty),
            2_000_000
        );
        assert_eq!(
            estimate_max_context(&Some("llama3.1".into()), empty),
            128_000
        );
        assert_eq!(estimate_max_context(&Some("llama3".into()), empty), 8_192);
        assert_eq!(
            estimate_max_context(&Some("deepseek-chat".into()), empty),
            128_000
        );
        assert_eq!(
            estimate_max_context(&Some("mistral-large".into()), empty),
            128_000
        );
        assert_eq!(
            estimate_max_context(&Some("codestral".into()), empty),
            256_000
        );
        assert_eq!(
            estimate_max_context(&Some("qwen2.5".into()), empty),
            128_000
        );
        assert_eq!(
            estimate_max_context(&Some("command-r-plus".into()), empty),
            128_000
        );
        assert_eq!(
            estimate_max_context(&Some("mystery-model".into()), empty),
            128_000
        );
        assert_eq!(estimate_max_context(&None, empty), 128_000);
    }

    #[test]
    fn estimate_max_context_prefers_stored_metadata() {
        let models = &[ModelInfo {
            id: "my-custom-model".into(),
            name: "My Custom Model".into(),
            provider_id: "openai".into(),
            tier: None,
            context_window: Some(42_000),
        }];
        // Stored value takes priority over heuristic
        assert_eq!(
            estimate_max_context(&Some("my-custom-model".into()), models),
            42_000
        );
        // Unknown model with no stored data falls back to heuristic default
        assert_eq!(
            estimate_max_context(&Some("mystery-model".into()), models),
            128_000
        );
    }

    #[test]
    fn create_session_creates_new_active_session() {
        let mut api = BimoApi::new();
        let original_id = api.agent.session.id.clone();

        let resp = api.create_session();
        assert!(resp.success);
        let data = resp.data.unwrap();
        let new_id = data["session_id"].as_str().unwrap().to_string();
        assert_ne!(new_id, original_id);
        assert_eq!(api.agent.session.id, new_id);
        assert_eq!(api.session_manager.active_id(), &new_id);
    }

    #[test]
    fn list_sessions_returns_all() {
        let api = BimoApi::new();
        let resp = api.list_sessions();
        assert!(resp.success);
        let data = resp.data.unwrap();
        let sessions = data["sessions"].as_array().unwrap();
        assert_eq!(sessions.len(), 1);
        assert!(data.get("active_session_id").is_some());
    }

    #[test]
    fn switch_session_changes_active() {
        let mut api = BimoApi::new();
        let id1 = api.agent.session.id.clone();

        // Create a second session
        let resp = api.create_session();
        let id2 = resp
            .data
            .unwrap()
            .get("session_id")
            .unwrap()
            .as_str()
            .unwrap()
            .to_string();

        // Switch back to first
        let resp = api.switch_session(SwitchSessionRequest {
            session_id: id1.clone(),
        });
        assert!(resp.success);
        assert_eq!(api.session_manager.active_id(), &id1);
        assert_eq!(api.agent.session.id, id1);

        // Switch to second again
        let resp = api.switch_session(SwitchSessionRequest {
            session_id: id2.clone(),
        });
        assert!(resp.success);
        assert_eq!(api.session_manager.active_id(), &id2);

        // cleanup
        let _ = crate::session::Session::delete_saved(&id1);
        let _ = crate::session::Session::delete_saved(&id2);
    }

    #[test]
    fn switch_session_unknown_id_errors() {
        let mut api = BimoApi::new();
        let resp = api.switch_session(SwitchSessionRequest {
            session_id: "nonexistent".into(),
        });
        assert!(!resp.success);
    }

    #[test]
    fn get_session_by_id_returns_session() {
        let api = BimoApi::new();
        let id = api.agent.session.id.clone();
        let resp = api.get_session_by_id(&id);
        assert!(resp.success);
        let data = resp.data.unwrap();
        assert_eq!(data["session_id"].as_str().unwrap(), &id);
    }

    #[test]
    fn delete_session_from_pool_removes_session() {
        let mut api = BimoApi::new();
        let id1 = api.agent.session.id.clone();

        // Create second session
        let resp = api.create_session();
        let id2 = resp
            .data
            .unwrap()
            .get("session_id")
            .unwrap()
            .as_str()
            .unwrap()
            .to_string();

        // Switch back to first session so we can delete the second
        let resp = api.switch_session(SwitchSessionRequest {
            session_id: id1.clone(),
        });
        assert!(resp.success);

        // Delete the second session
        let resp = api.delete_session_from_pool(&id2);
        assert!(resp.success);

        // List should only have one
        let resp = api.list_sessions();
        let data = resp.data.unwrap();
        let sessions = data["sessions"].as_array().unwrap();
        assert_eq!(sessions.len(), 1);

        // cleanup
        let _ = crate::session::Session::delete_saved(&id1);
        let _ = crate::session::Session::delete_saved(&id2);
    }

    #[test]
    fn delete_active_session_errors() {
        let mut api = BimoApi::new();
        let id = api.agent.session.id.clone();
        let resp = api.delete_session_from_pool(&id);
        assert!(!resp.success);
    }

    #[tokio::test]
    async fn tree_fork_adds_new_session_to_pool() {
        let mut api = BimoApi::new();
        let old_id = api.agent.session.id.clone();

        // Add messages to the session so fork has something to work with
        // (session already has a system message from Agent::new())
        api.agent.session.add_user_message("hello");
        api.agent.session.add_assistant_message("hi");
        assert_eq!(api.agent.session.message_count(), 3);
        api.sync_active_to_pool();

        // Fork at index 1 — keeps system + user (2 messages)
        let resp = api
            .execute_command(CommandRequest {
                command: "/tree fork 1".into(),
            })
            .await;
        assert!(resp.success);
        let result = resp.data.unwrap();
        let data = result["data"].as_object().unwrap();
        assert_eq!(data["action"].as_str().unwrap(), "fork");
        let new_id = data["new_session_id"].as_str().unwrap().to_string();

        // New session should be in the pool and active
        assert_ne!(new_id, old_id);
        assert_eq!(api.session_manager.active_id(), &new_id);
        assert_eq!(api.agent.session.id, new_id);

        // Forked session should have 2 messages (index 0..=1)
        assert_eq!(api.agent.session.message_count(), 2);

        // Old session should still be in the pool
        assert!(api.session_manager.get(&old_id).is_some());

        // cleanup
        let _ = crate::session::Session::delete_saved(&old_id);
        let _ = crate::session::Session::delete_saved(&new_id);
    }

    #[tokio::test]
    async fn tree_revert_syncs_to_pool() {
        let mut api = BimoApi::new();
        let id = api.agent.session.id.clone();

        // Add messages (session already has 1 system message from Agent::new())
        api.agent.session.add_user_message("a");
        api.agent.session.add_user_message("b");
        api.agent.session.add_user_message("c");
        assert_eq!(api.agent.session.message_count(), 4);
        api.sync_active_to_pool();

        // Revert at index 1 — keeps system + first user message (2 messages)
        let resp = api
            .execute_command(CommandRequest {
                command: "/tree revert 1".into(),
            })
            .await;
        assert!(resp.success);
        let result = resp.data.unwrap();
        let data = result["data"].as_object().unwrap();
        assert_eq!(data["action"].as_str().unwrap(), "revert");
        assert_eq!(data["message_count"].as_u64().unwrap(), 2);

        // Session should now have 2 messages
        assert_eq!(api.agent.session.message_count(), 2);

        // Pool should also reflect the change
        let pooled = api.session_manager.get(&id).unwrap();
        assert_eq!(pooled.message_count(), 2);

        // cleanup
        let _ = crate::session::Session::delete_saved(&id);
    }

    #[tokio::test]
    async fn session_resume_switches_session() {
        let mut api = BimoApi::new();
        let id1 = api.agent.session.id.clone();

        // Create a second session (persists to disk)
        let resp = api.create_session();
        let id2 = resp
            .data
            .unwrap()
            .get("session_id")
            .unwrap()
            .as_str()
            .unwrap()
            .to_string();

        // Switch back to first
        let resp = api.switch_session(SwitchSessionRequest {
            session_id: id1.clone(),
        });
        assert!(resp.success);

        // Execute /session resume <id2>
        let resp = api
            .execute_command(CommandRequest {
                command: format!("/session resume {id2}"),
            })
            .await;
        assert!(resp.success);
        assert!(
            resp.data.unwrap()["output"]
                .as_str()
                .unwrap()
                .contains("Resumed session")
        );

        // Should now be active on id2
        assert_eq!(api.session_manager.active_id(), &id2);
        assert_eq!(api.agent.session.id, id2);

        // cleanup
        let _ = crate::session::Session::delete_saved(&id1);
        let _ = crate::session::Session::delete_saved(&id2);
    }
}
