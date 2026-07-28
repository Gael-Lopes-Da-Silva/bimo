use crate::agent::Agent;
use crate::config::CustomProviderConfig;
use crate::session::Role;
use crate::session::manager::SessionManager;

use super::dto::*;

use tracing;

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
        let system_prompt = crate::prompts::render(
            &crate::prompts::load(crate::prompts::SYSTEM),
            &[
                ("TOOLS", &tool_xml),
                ("DATE", &now),
                ("CWD", &cwd),
                (
                    "PROJECT_CONTEXT",
                    &crate::agent::build_project_context(&cwd),
                ),
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

        // Check the pool first
        if let Some(session) = self.session_manager.get(session_id) {
            let data = SessionData {
                session_id: session.id.clone(),
                messages: session.messages.clone(),
                message_count: session.message_count(),
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
                // Check if the command requested a session switch
                // (We need to re-dispatch to get the context, but we can check the result data)
                // Actually, the switch_session_id is in the context which was consumed.
                // We need to extract it differently. Let's handle it via the result data.
                // For now, the switch is handled by the command setting data with session_id.
                tracing::info!(command = %result.command, output_len = result.output.len(), "execute_command success");

                // Handle session switch: check if this was a switch command
                if result.command == "session"
                    && result.output.starts_with("Switching to session")
                    && let Some(data) = &result.data
                    && let Some(sid) = data.get("session_id").and_then(|v| v.as_str())
                {
                    // Perform the actual switch
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
                let tokens = estimate_tokens(&m.content);
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
        let max_tokens = estimate_max_context(&self.agent.config.selected_model);
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

/// Rough token estimate: ~4 characters per token (common for English text).
fn estimate_tokens(text: &str) -> usize {
    text.len().div_ceil(4)
}

/// Estimate the max context window for a model based on its id/name.
fn estimate_max_context(model: &Option<String>) -> usize {
    let model_str = model.as_deref().unwrap_or("");
    let lower = model_str.to_lowercase();

    if lower.contains("claude") {
        if lower.contains("opus") || lower.contains("sonnet-4") || lower.contains("3.5") {
            return 200_000;
        }
        if lower.contains("haiku") {
            return 200_000;
        }
        return 100_000;
    }
    if lower.contains("o3") || lower.contains("o4") {
        return 200_000;
    }
    if lower.contains("o1") {
        return 200_000;
    }
    if lower.contains("gpt-4o") || lower.contains("gpt-4-turbo") {
        return 128_000;
    }
    if lower.contains("gpt-4") {
        return 8_192;
    }
    if lower.contains("gpt-3.5") {
        return 16_385;
    }
    if lower.contains("gemini") {
        return 1_000_000;
    }
    if lower.contains("llama") || lower.contains("qwen") || lower.contains("deepseek") {
        return 128_000;
    }

    128_000
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
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_tokens("ab"), 1);
        assert_eq!(estimate_tokens("abcdefgh"), 2);
        assert_eq!(estimate_tokens("hello world"), 3);
    }

    #[test]
    fn estimate_max_context_models() {
        assert_eq!(estimate_max_context(&Some("gpt-4".into())), 8_192);
        assert_eq!(estimate_max_context(&Some("gpt-4o".into())), 128_000);
        assert_eq!(
            estimate_max_context(&Some("claude-sonnet-4-20250514".into())),
            200_000
        );
        assert_eq!(estimate_max_context(&Some("o3-mini".into())), 200_000);
        assert_eq!(
            estimate_max_context(&Some("gemini-2.5-pro".into())),
            1_000_000
        );
        assert_eq!(estimate_max_context(&Some("llama3".into())), 128_000);
        assert_eq!(estimate_max_context(&Some("mystery-model".into())), 128_000);
        assert_eq!(estimate_max_context(&None), 128_000);
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
        let mut api = BimoApi::new();
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
        let mut api = BimoApi::new();
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
}
