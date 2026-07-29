use crate::api::facade::estimate_tokens;
use crate::command::{CommandContext, CommandRegistry, CommandResult};
use crate::config::{AppConfig, CustomProviderConfig, ProviderPersistedConfig, ThinkingConfig};
use crate::error::{BimoError, Result};
use crate::model::{self, ModelInfo};
use crate::prompts;
use crate::provider::{self, ProviderInfo, ProviderRegistry, ProviderRuntime, UsageInfo};
use crate::session::Session;
use crate::tool::{self, ToolCall, ToolRegistry, ToolResult};
use tracing;

/// Maximum number of tool call iterations per chat request.
const MAX_TOOL_ITERATIONS: usize = 20;

/// The response from a chat interaction.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChatResponse {
    pub content: String,
    pub thinking: Option<String>,
    pub model: Option<String>,
    pub usage: Option<UsageInfo>,
    pub session_id: String,
}

/// The core agent that holds all state and coordinates operations.
pub struct Agent {
    pub config: AppConfig,
    pub session: Session,
    pub provider_registry: ProviderRegistry,
    pub available_models: Vec<ModelInfo>,
    pub runtime: Option<ProviderRuntime>,
    pub command_registry: CommandRegistry,
    pub tool_registry: ToolRegistry,
    pub loaded_skills: Vec<String>,
}

impl Default for Agent {
    fn default() -> Self {
        Self::new()
    }
}

impl Agent {
    /// Create a new agent with no provider selected.
    pub fn new() -> Self {
        tracing::info!("creating new Agent");
        let config = AppConfig::load();
        let provider_registry = ProviderRegistry::new();
        let command_registry = CommandRegistry::new();
        let tool_registry = ToolRegistry::new();
        let mut session = Session::new();

        // Inject system prompt with tool descriptions and context
        let tool_xml = tool_registry.render_tool_xml();
        let now = chrono::Local::now().format("%Y-%m-%d").to_string();
        let cwd = std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "<unknown>".into());
        let project_context = build_project_context(&cwd);
        let system_prompt = prompts::render(
            &prompts::load(prompts::SYSTEM),
            &[
                ("TOOLS", &tool_xml),
                ("DATE", &now),
                ("CWD", &cwd),
                ("PROJECT_CONTEXT", &project_context.rendered),
            ],
        );
        session.add_system_message(&system_prompt);
        tracing::debug!(prompt_len = system_prompt.len(), "system prompt injected");

        tracing::debug!(
            session_id = %session.id,
            selected_provider = ?config.selected_provider,
            selected_model = ?config.selected_model,
            "agent state loaded"
        );

        let mut agent = Self {
            config,
            session,
            provider_registry,
            available_models: Vec::new(),
            runtime: None,
            command_registry,
            tool_registry,
            loaded_skills: Vec::new(),
        };

        if let Some(pid) = agent.config.selected_provider.clone() {
            tracing::info!(provider_id = %pid, "resolving provider runtime");
            match agent.provider_registry.resolve_runtime(&pid, &agent.config) {
                Ok(rt) => {
                    tracing::info!(provider_id = %pid, "provider runtime resolved");
                    agent.runtime = Some(rt);
                }
                Err(e) => {
                    tracing::warn!(error = %e, provider_id = %pid, "failed to resolve provider runtime");
                }
            }
        }

        agent
    }

    /// Whether the agent requires initial configuration before it can function.
    pub fn needs_configuration(&self) -> bool {
        self.config.selected_provider.is_none()
    }

    // -----------------------------------------------------------------------
    // Provider management
    // -----------------------------------------------------------------------

    pub fn list_providers(&self) -> Vec<ProviderInfo> {
        self.provider_registry.list_all(&self.config)
    }

    pub async fn select_provider(&mut self, provider_id: &str) -> Result<ProviderInfo> {
        tracing::info!(provider_id, "select_provider called");
        let info = self
            .provider_registry
            .list_all(&self.config)
            .into_iter()
            .find(|p| p.id == provider_id)
            .ok_or_else(|| BimoError::Provider(format!("unknown provider '{provider_id}'")))?;

        let runtime = self
            .provider_registry
            .resolve_runtime(provider_id, &self.config)?;

        self.runtime = Some(runtime);
        self.config.selected_provider = Some(provider_id.to_string());
        self.available_models.clear();
        self.config.selected_model = None;
        self.config.save()?;

        tracing::debug!(provider_id, "fetching models");
        self.fetch_models().await?;
        tracing::info!(
            provider_id,
            model_count = self.available_models.len(),
            "select_provider done"
        );

        Ok(info)
    }

    pub fn configure_provider(
        &mut self,
        provider_id: &str,
        base_url: Option<String>,
        api_key: Option<String>,
    ) -> Result<()> {
        tracing::info!(provider_id, "configure_provider called");
        let default_base_url = self
            .provider_registry
            .list_all(&self.config)
            .iter()
            .find(|p| p.id == provider_id)
            .map(|p| p.default_base_url.clone());

        let entry = self
            .config
            .provider_configs
            .entry(provider_id.to_string())
            .or_insert_with(|| ProviderPersistedConfig {
                base_url: String::new(),
                api_key: None,
            });

        if let Some(url) = base_url {
            tracing::debug!(base_url = %url, "setting base_url");
            entry.base_url = url;
        }
        if let Some(key) = api_key {
            tracing::debug!("setting api_key");
            entry.api_key = Some(key);
        }

        if entry.base_url.is_empty()
            && let Some(url) = default_base_url
        {
            entry.base_url = url;
        }

        self.config.save()?;

        if self.config.selected_provider.as_deref() == Some(provider_id) {
            tracing::debug!(
                provider_id,
                "rebuilding runtime for currently selected provider"
            );
            self.runtime = Some(
                self.provider_registry
                    .resolve_runtime(provider_id, &self.config)?,
            );
        }

        tracing::info!(provider_id, "configure_provider done");
        Ok(())
    }

    pub fn add_custom_provider(&mut self, cp: CustomProviderConfig) -> Result<()> {
        tracing::info!(id = %cp.id, name = %cp.name, "add_custom_provider called");
        if self
            .provider_registry
            .list_all(&self.config)
            .iter()
            .any(|p| p.id == cp.id)
        {
            tracing::warn!(id = %cp.id, "provider id already exists");
            return Err(BimoError::Provider(format!(
                "a provider with id '{}' already exists",
                cp.id
            )));
        }
        self.config.custom_providers.push(cp);
        self.config.save()?;
        tracing::info!("add_custom_provider done");
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Model management
    // -----------------------------------------------------------------------

    pub async fn fetch_models(&mut self) -> Result<Vec<ModelInfo>> {
        tracing::debug!("fetch_models called");
        let runtime = self
            .runtime
            .as_ref()
            .ok_or_else(|| BimoError::Provider("no provider selected".into()))?;

        let models = model::fetch_models_for_provider(runtime).await?;
        tracing::info!(provider = %runtime.id, count = models.len(), "fetch_models done");
        self.available_models = models.clone();
        Ok(models)
    }

    pub fn list_models(&self) -> &[ModelInfo] {
        &self.available_models
    }

    pub fn select_model(&mut self, model_id: &str) -> Result<()> {
        tracing::info!(model_id, "select_model called");
        let exists = self.available_models.iter().any(|m| m.id == model_id);
        if !exists && !self.available_models.is_empty() {
            tracing::warn!(model_id, "model not found in available models");
            return Err(BimoError::Model(format!(
                "model '{model_id}' not found. Available models: {}",
                self.available_models
                    .iter()
                    .map(|m| m.id.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        }
        self.config.selected_model = Some(model_id.to_string());
        self.config.save()?;
        tracing::info!(model_id, "select_model done");
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Chat
    // -----------------------------------------------------------------------

    pub async fn chat(&mut self, user_message: &str) -> Result<ChatResponse> {
        tracing::info!(message_len = user_message.len(), "chat called");
        let runtime = self
            .runtime
            .as_ref()
            .ok_or_else(|| BimoError::Provider("no provider selected".into()))?
            .clone();

        let model = self
            .config
            .selected_model
            .clone()
            .ok_or_else(|| BimoError::Model("no model selected".into()))?;

        tracing::debug!(provider = %runtime.id, model = %model, session_id = %self.session.id, "sending chat completion");

        // Inject todo context before the user message
        self.inject_todo_context();

        let estimated_tokens = Some(estimate_tokens(user_message, None));
        self.session
            .add_user_message_with_tokens(user_message, estimated_tokens);

        let mut total_tool_calls: Vec<ToolCall> = Vec::new();
        let mut total_tool_results: Vec<ToolResult> = Vec::new();

        // Tool calling loop
        for iteration in 0..=MAX_TOOL_ITERATIONS {
            let messages = self.session.to_chat_messages();
            let response =
                provider::chat_completion(&runtime, &messages, &model, &self.config.thinking)
                    .await?;

            // Parse tool calls from the response
            let tool_calls = tool::call::parse_tool_calls(&response.content);

            if tool_calls.is_empty() || iteration == MAX_TOOL_ITERATIONS {
                // No tool calls or max iterations reached - return final response
                let estimated_tokens = Some(estimate_tokens(&response.content, Some(&model)));
                self.session.add_assistant_response(
                    &response.content,
                    response.model.clone(),
                    Some(runtime.id.clone()),
                    estimated_tokens,
                );

                tracing::info!(
                    model = ?response.model,
                    content_len = response.content.len(),
                    total_tool_calls = total_tool_calls.len(),
                    "chat done"
                );
                return Ok(ChatResponse {
                    content: response.content,
                    thinking: response.thinking,
                    model: response.model,
                    usage: response.usage,
                    session_id: self.session.id.clone(),
                });
            }

            // Execute tool calls
            tracing::info!(
                iteration,
                tool_count = tool_calls.len(),
                "executing tool calls"
            );

            // Add the assistant's response (with tool calls) to the session
            let estimated_tokens = Some(estimate_tokens(&response.content, Some(&model)));
            self.session.add_assistant_response(
                &response.content,
                response.model.clone(),
                Some(runtime.id.clone()),
                estimated_tokens,
            );

            for call in &tool_calls {
                tracing::debug!(tool = %call.name, args = ?call.arguments, "executing tool");
                let result = tool::call::execute_tool_call(call, &self.tool_registry).await;
                tracing::debug!(tool = %call.name, is_error = result.is_error, "tool executed");

                // Handle todo actions
                if call.name == "manage_todo"
                    && !result.is_error
                    && let Ok(action) = tool::call::parse_todo_action(&call.arguments)
                {
                    let todo_result =
                        tool::call::apply_todo_action(&action, &mut self.session.todos);
                    tracing::debug!(todo_action = ?action, "todo action applied");
                    // Add the todo result as an additional tool message
                    let todo_msg = format!("[Todo: {}]", todo_result);
                    self.session.add_tool_message(&todo_msg);
                }

                // Add tool result to session
                let result_msg = tool::call::format_tool_result_message(&result);
                self.session.add_tool_message(&result_msg);

                total_tool_calls.push(call.clone());
                total_tool_results.push(result);
            }
        }

        // This should never be reached, but just in case
        Err(BimoError::Provider(
            "tool call loop exceeded maximum iterations".into(),
        ))
    }

    // -----------------------------------------------------------------------
    // Commands
    // -----------------------------------------------------------------------

    pub async fn execute_command(
        &mut self,
        input: &str,
        active_session_id: &str,
        all_sessions: &[crate::session::SessionInfo],
    ) -> Result<CommandResult> {
        tracing::info!(input, "execute_command called");

        // Lazy-fetch models if runtime exists but none loaded yet
        if self.available_models.is_empty() && self.runtime.is_some() {
            tracing::info!("lazy-fetching models before command");
            let _ = self.fetch_models().await;
        }

        let mut ctx = self.build_command_context();
        ctx.active_session_id = active_session_id.to_string();
        ctx.all_sessions = all_sessions.to_vec();
        let mut result = self
            .command_registry
            .dispatch_async(input, &mut ctx)
            .await?;

        // Handle special post-command actions
        let command_name = result.command.clone();
        tracing::debug!(command = %command_name, "post-command processing");

        match command_name.as_str() {
            "session" => {
                self.handle_session_command(&result)?;
            }
            "compact" => {
                if ctx.compact_requested {
                    tracing::info!("compacting session via LLM");
                    self.compact_session().await?;
                    return Ok(CommandResult {
                        command: "compact".into(),
                        output: "Session context compacted successfully.".into(),
                        data: None,
                    });
                }
            }
            "provider" => {
                if let Some(cp) = ctx.provider_add_request.take() {
                    let id = cp.id.clone();
                    match self.add_custom_provider(cp) {
                        Ok(()) => {
                            result.output = format!("Added custom provider '{id}'.");
                        }
                        Err(e) => {
                            result.output = format!("Failed to add custom provider: {e}");
                        }
                    }
                }
                if let Some((ref pid, ref api_key, ref base_url)) =
                    ctx.provider_configure_request.take()
                {
                    match self.configure_provider(pid, base_url.clone(), api_key.clone()) {
                        Ok(()) => {
                            let mut details = String::new();
                            if api_key.is_some() {
                                details.push_str(" with API key");
                            }
                            if let Some(url) = base_url {
                                if !details.is_empty() {
                                    details.push(',');
                                }
                                details.push_str(&format!(" base URL: {url}"));
                            }
                            result.output = format!("Configured '{pid}'{details}.");
                        }
                        Err(e) => {
                            result.output = format!("Failed to configure '{pid}': {e}");
                        }
                    }
                }
            }
            "tree" => {
                if let Some(index) = ctx.tree_fork_index {
                    tracing::info!(index, "forking session");
                    let old_id = self.session.id.clone();
                    let forked = self.fork_session(index)?;
                    let new_id = forked.id.clone();
                    self.session = forked;
                    return Ok(CommandResult {
                        command: "tree".into(),
                        output: format!(
                            "Forked to new session {} at message {}.",
                            &new_id[..8.min(new_id.len())],
                            index
                        ),
                        data: Some(serde_json::json!({
                            "action": "fork",
                            "old_session_id": old_id,
                            "new_session_id": new_id,
                        })),
                    });
                }
                if let Some(index) = ctx.tree_revert_index {
                    tracing::info!(index, "reverting session");
                    self.revert_session(index)?;
                    return Ok(CommandResult {
                        command: "tree".into(),
                        output: format!(
                            "Reverted to message {}. {} messages remaining.",
                            index,
                            self.session.message_count()
                        ),
                        data: Some(serde_json::json!({
                            "action": "revert",
                            "session_id": self.session.id,
                            "message_count": self.session.message_count(),
                        })),
                    });
                }
            }
            _ => {}
        }

        // Handle pending user message (e.g. from /prompt command)
        if let Some(msg) = ctx.pending_user_message.take() {
            tracing::info!(
                message_len = msg.len(),
                "adding pending user message from command"
            );
            self.session.add_user_message(&msg);
        }

        // Handle pending system message (e.g. from /skill command)
        if let Some(msg) = ctx.pending_system_message.take() {
            tracing::info!(
                message_len = msg.len(),
                "adding pending system message from command"
            );
            self.session.add_system_message(&msg);
        }

        self.sync_from_command_context(&ctx);

        // If a provider is selected and runtime is missing or stale, re-init
        if let Some(ref pid) = self.config.selected_provider.clone() {
            let current_id = self.runtime.as_ref().map(|r| &r.id);
            if current_id != Some(pid) {
                // Auto-configure local providers that don't need an API key
                if !self.config.provider_configs.contains_key(pid) {
                    let needs_key = self
                        .provider_registry
                        .list_all(&self.config)
                        .into_iter()
                        .find(|p| p.id == *pid)
                        .map(|p| p.requires_api_key)
                        .unwrap_or(true);
                    if !needs_key {
                        tracing::info!(provider_id = %pid, "auto-configuring local provider");
                        let _ = self.configure_provider(pid, None, None);
                    }
                }

                match self.select_provider(pid).await {
                    Ok(_info) => {
                        tracing::info!(provider_id = %pid, "provider re-initialised after command");
                        if self.available_models.is_empty() {
                            let warn = "\nWarning: provider returned no models.";
                            if !result.output.contains(warn.trim()) {
                                result.output.push_str(warn);
                            }
                        }
                    }
                    Err(e) => {
                        let warn = format!("\nWarning: {e}");
                        if !result.output.contains(&warn) {
                            result.output.push_str(&warn);
                        }
                    }
                }
            }
        }

        Ok(result)
    }

    pub fn clear_session(&mut self) {
        tracing::info!(session_id = %self.session.id, messages = self.session.message_count(), "clearing session");
        self.session.clear();
    }

    /// Inject a todo context message so the LLM sees the current todo state.
    fn inject_todo_context(&mut self) {
        if !self.session.todos.is_empty() {
            let context = self.session.todos.render_context();
            self.session
                .add_tool_message(&format!("[Current Todo State]\n{}", context));
        }
    }

    // -----------------------------------------------------------------------
    // Session operations
    // -----------------------------------------------------------------------

    fn handle_session_command(&mut self, result: &CommandResult) -> Result<()> {
        let data = match &result.data {
            Some(d) => d,
            None => return Ok(()),
        };

        let action = data.get("action").and_then(|v| v.as_str());

        match action {
            Some("save") => {
                tracing::info!(session_id = %self.session.id, "saving session to disk");
                return self.session.save();
            }
            Some("purge") => {
                tracing::info!("purging all saved sessions");
                return Session::delete_all_saved();
            }
            Some("delete") => {
                if let Some(id) = data.get("session_id").and_then(|v| v.as_str()) {
                    tracing::info!(session_id = id, "deleting saved session");
                    return Session::delete_saved(id);
                }
            }
            _ => {}
        }

        Ok(())
    }

    /// Compact the session by summarizing it via the provider.
    async fn compact_session(&mut self) -> Result<()> {
        tracing::info!(
            message_count = self.session.message_count(),
            "compact_session called"
        );
        let runtime = self
            .runtime
            .as_ref()
            .ok_or_else(|| BimoError::Provider("no provider selected".into()))?
            .clone();

        let model = self
            .config
            .selected_model
            .as_deref()
            .ok_or_else(|| BimoError::Model("no model selected".into()))?;

        // Build a summarization prompt
        let conversation: Vec<String> = self
            .session
            .messages
            .iter()
            .map(|m| {
                let role = match m.role {
                    crate::session::Role::User => "User",
                    crate::session::Role::Assistant => "Assistant",
                    crate::session::Role::System => "System",
                    crate::session::Role::Tool => "Tool",
                };
                format!("{role}: {}", m.content)
            })
            .collect();

        let prompt_template = prompts::load(prompts::COMPACT);
        let prompt = prompts::render(
            &prompt_template,
            &[("CONVERSATION", &conversation.join("\n\n"))],
        );

        tracing::debug!(prompt_len = prompt.len(), "sending summarization request");
        let messages = vec![provider::ChatMessage {
            role: "user".into(),
            content: prompt,
        }];

        let response =
            provider::chat_completion(&runtime, &messages, model, &ThinkingConfig::default())
                .await?;
        tracing::debug!(
            summary_len = response.content.len(),
            "compaction summary received"
        );
        self.session.compact(&response.content);

        // Save the compacted session
        self.session.save()?;
        tracing::info!("compact_session done");
        Ok(())
    }

    /// Save the current session to disk.
    pub fn save_session(&mut self) -> Result<()> {
        tracing::info!(session_id = %self.session.id, "save_session called");
        self.session.save()
    }

    /// Resume a saved session by id (supports prefix matching).
    pub fn resume_session(&mut self, id: &str) -> Result<()> {
        tracing::info!(id, "resume_session called");
        let sessions = Session::list_saved()?;
        let found = sessions
            .iter()
            .find(|s| s.id == id || s.id.starts_with(id))
            .ok_or_else(|| BimoError::Session(format!("session '{id}' not found")))?;

        let loaded = Session::load(&found.id)?;
        tracing::info!(loaded_id = %found.id, message_count = loaded.message_count(), "resume_session done");
        self.session = loaded;
        Ok(())
    }

    /// Delete a saved session by id.
    pub fn delete_session(&mut self, id: &str) -> Result<()> {
        tracing::info!(id, "delete_session called");
        Session::delete_saved(id)
    }

    /// Fork the current session at the given message index.
    /// Creates a new session with messages 0..=index and switches to it.
    pub fn fork_session(&mut self, index: usize) -> Result<Session> {
        tracing::info!(index, "fork_session called");
        let forked = self.session.fork(index)?;
        tracing::info!(new_session_id = %forked.id, "fork_session done");
        Ok(forked)
    }

    /// Revert the current session by discarding all messages after the given index.
    /// Saves the truncated session to disk.
    pub fn revert_session(&mut self, index: usize) -> Result<()> {
        tracing::info!(index, "revert_session called");
        self.session.revert(index)?;
        tracing::info!(
            remaining = self.session.message_count(),
            "revert_session done"
        );
        self.session.save()
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    fn build_command_context(&self) -> CommandContext {
        let providers = self.provider_registry.list_all(&self.config);
        let saved_sessions = Session::list_saved().unwrap_or_default();
        let command_descriptions: Vec<(String, String)> = self
            .command_registry
            .list_detailed()
            .iter()
            .map(|c| (c.name.clone(), c.description.clone()))
            .collect();
        CommandContext {
            selected_provider: self.config.selected_provider.clone(),
            selected_model: self.config.selected_model.clone(),
            available_models: self.available_models.clone(),
            session_id: self.session.id.clone(),
            session_message_count: self.session.message_count(),
            session_messages: self.session.messages.clone(),
            provider_ids: providers.iter().map(|p| p.id.clone()).collect(),
            provider_names: providers.iter().map(|p| p.name.clone()).collect(),
            providers: providers.clone(),
            configured_providers: self.config.provider_configs.keys().cloned().collect(),
            needs_configuration: self.needs_configuration(),
            tools: self.tool_registry.list().to_vec(),
            command_descriptions,
            saved_sessions,
            compact_requested: false,
            has_runtime: self.runtime.is_some(),
            tree_fork_index: None,
            tree_revert_index: None,
            thinking: self.config.thinking.clone(),
            todos: self.session.todos.clone(),
            active_session_id: self.session.id.clone(),
            all_sessions: vec![],
            switch_session_id: None,
            provider_configure_request: None,
            provider_add_request: None,
            pending_user_message: None,
            pending_system_message: None,
            loaded_skills: self.loaded_skills.clone(),
        }
    }

    fn sync_from_command_context(&mut self, ctx: &CommandContext) {
        self.config.selected_provider = ctx.selected_provider.clone();
        self.config.selected_model = ctx.selected_model.clone();
        self.config.thinking = ctx.thinking.clone();
        self.session.todos = ctx.todos.clone();
        self.loaded_skills = ctx.loaded_skills.clone();
    }
}

/// Structured project context with both the rendered string and metadata.
pub(crate) struct ProjectContext {
    /// The rendered context string for the system prompt.
    pub rendered: String,
    /// The active git branch, if any.
    pub git_branch: Option<String>,
    /// Filenames of agent instruction files found and loaded.
    pub agent_instruction_files: Vec<String>,
}

/// Build project context for the system prompt.
/// Includes git branch (if available), top-level directory listing,
/// and contents of standard agent instruction files (AGENTS.md,
/// CLAUDE.md, GEMINI.md, .github/copilot-instructions.md).
pub(crate) fn build_project_context(cwd: &str) -> ProjectContext {
    let mut parts: Vec<String> = Vec::new();
    let mut git_branch: Option<String> = None;

    // Git branch
    if let Ok(out) = std::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(cwd)
        .output()
        && out.status.success()
    {
        let branch = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !branch.is_empty() {
            git_branch = Some(branch.clone());
            parts.push(format!("Git branch: {branch}"));
        }
    }

    // Top-level directory listing
    if let Ok(entries) = std::fs::read_dir(cwd) {
        let mut names: Vec<String> = entries
            .filter_map(|e| e.ok())
            .map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    format!("{name}/")
                } else {
                    name
                }
            })
            .collect();
        names.sort();
        if !names.is_empty() {
            parts.push(format!("Project files: {}", names.join(", ")));
        }
    }

    // Agent instruction files
    let (instruction_files, instruction_chunks) = load_agent_instructions(cwd);
    for chunk in instruction_chunks {
        parts.push(chunk);
    }

    let rendered = if parts.is_empty() {
        "No project context available.".into()
    } else {
        parts.join("\n")
    };

    ProjectContext {
        rendered,
        git_branch,
        agent_instruction_files: instruction_files,
    }
}

/// Read contents of standard agent instruction files from the project root.
/// Returns (list of filenames found, list of rendered content chunks).
/// Supports: AGENTS.md, CLAUDE.md, GEMINI.md, .agents/AGENTS.md
fn load_agent_instructions(cwd: &str) -> (Vec<String>, Vec<String>) {
    const CANDIDATES: &[&str] = &["AGENTS.md", "CLAUDE.md", "GEMINI.md", ".agents/AGENTS.md"];

    let mut filenames: Vec<String> = Vec::new();
    let mut chunks: Vec<String> = Vec::new();

    for &file in CANDIDATES {
        let path = std::path::Path::new(cwd).join(file);
        if path.exists()
            && let Ok(content) = std::fs::read_to_string(&path)
        {
            let trimmed = content.trim().to_string();
            if !trimmed.is_empty() {
                filenames.push(file.to_string());
                chunks.push(format!("Instructions from {file}:\n{trimmed}"));
            }
        }
    }

    (filenames, chunks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::Role;

    #[test]
    fn new_agent_has_no_provider() {
        let agent = Agent::new();
        assert!(agent.runtime.is_none());
        assert!(agent.needs_configuration());
        assert_eq!(agent.list_models().len(), 0);
    }

    #[test]
    fn new_agent_has_system_message() {
        let agent = Agent::new();
        assert_eq!(agent.session.message_count(), 1);
        assert_eq!(agent.session.messages[0].role, Role::System);
    }

    #[test]
    fn new_agent_has_builtin_providers() {
        let agent = Agent::new();
        let providers = agent.list_providers();
        assert!(!providers.is_empty());
        assert!(providers.iter().any(|p| p.id == "openai"));
        assert!(providers.iter().any(|p| p.id == "ollama"));
    }

    #[test]
    fn add_custom_provider_duplicate_errors() {
        let mut agent = Agent::new();
        let cp = CustomProviderConfig {
            id: "openai".into(),
            name: "My OpenAI".into(),
            category: "cloud".into(),
            base_url: "https://my.openai.com".into(),
            api_key_required: true,
            chat_endpoint: "/chat".into(),
            models_endpoint: None,
            auth_header: None,
            auth_prefix: None,
        };
        let result = agent.add_custom_provider(cp);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already exists"));
    }

    #[test]
    fn add_custom_provider_succeeds() {
        // Clean any leftover state from previous test runs
        let mut cfg = crate::config::AppConfig::load();
        cfg.custom_providers.clear();
        let _ = cfg.save();

        let mut agent = Agent::new();
        let cp = CustomProviderConfig {
            id: "my-test-provider".into(),
            name: "Test Provider".into(),
            category: "cloud".into(),
            base_url: "https://test.api.com".into(),
            api_key_required: false,
            chat_endpoint: "/v1/chat".into(),
            models_endpoint: Some("/v1/models".into()),
            auth_header: Some("Authorization".into()),
            auth_prefix: Some("Bearer ".into()),
        };
        if let Err(e) = agent.add_custom_provider(cp) {
            panic!("add_custom_provider failed: {}", e);
        }
        let providers = agent.list_providers();
        assert!(providers.iter().any(|p| p.id == "my-test-provider"));
        // Cleanup: remove the custom provider and save clean config
        agent
            .config
            .custom_providers
            .retain(|p| p.id != "my-test-provider");
        let _ = agent.config.save();
    }

    #[test]
    fn select_model_without_provider_succeeds() {
        let mut agent = Agent::new();
        assert!(agent.runtime.is_none());
        let result = agent.select_model("test-model");
        assert!(result.is_ok());
        assert_eq!(agent.config.selected_model.as_deref(), Some("test-model"));
        // Reset
        agent.config.selected_model = None;
    }

    #[test]
    fn select_model_unknown_when_models_populated_errors() {
        let mut agent = Agent::new();
        agent.available_models = vec![ModelInfo {
            id: "gpt-4".into(),
            name: "GPT-4".into(),
            provider_id: "openai".into(),
            tier: None,
            context_window: None,
        }];
        let result = agent.select_model("nonexistent-model");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn clear_session_removes_messages() {
        let mut agent = Agent::new();
        assert_eq!(agent.session.message_count(), 1);
        agent.session.add_user_message("hello");
        assert_eq!(agent.session.message_count(), 2);
        agent.clear_session();
        assert_eq!(agent.session.message_count(), 0);
    }

    #[test]
    fn needs_configuration_when_no_provider() {
        let agent = Agent::new();
        assert!(agent.needs_configuration());
    }

    #[test]
    fn configure_provider_adds_config() {
        let mut agent = Agent::new();
        let result = agent.configure_provider("openai", None, Some("sk-test-key".into()));
        assert!(result.is_ok());
        let config = agent.config.provider_configs.get("openai").unwrap();
        assert_eq!(config.api_key.as_deref(), Some("sk-test-key"));
        // Cleanup
        agent.config.provider_configs.remove("openai");
    }

    #[test]
    fn save_session_persists_to_disk() {
        let mut agent = Agent::new();
        let id = agent.session.id.clone();
        agent.save_session().unwrap();
        let loaded = Session::load(&id).unwrap();
        assert_eq!(loaded.id, id);
        Session::delete_saved(&id).unwrap();
    }
}
