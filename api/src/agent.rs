use crate::command::{CommandContext, CommandRegistry, CommandResult};
use crate::config::{AppConfig, CustomProviderConfig, ProviderPersistedConfig};
use crate::error::{BimoError, Result};
use crate::model::{self, ModelInfo};
use crate::provider::{self, ProviderInfo, ProviderRegistry, ProviderRuntime, UsageInfo};
use crate::session::Session;
use crate::tools::ToolRegistry;

/// The response from a chat interaction.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChatResponse {
    pub content: String,
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
}

impl Agent {
    /// Create a new agent with no provider selected.
    pub fn new() -> Self {
        let config = AppConfig::load();
        let provider_registry = ProviderRegistry::new();
        let command_registry = CommandRegistry::new();
        let tool_registry = ToolRegistry::new();
        let session = Session::new();

        let mut agent = Self {
            config,
            session,
            provider_registry,
            available_models: Vec::new(),
            runtime: None,
            command_registry,
            tool_registry,
        };

        if let Some(pid) = agent.config.selected_provider.clone() {
            if let Ok(rt) = agent.provider_registry.resolve_runtime(&pid, &agent.config) {
                agent.runtime = Some(rt);
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

        self.fetch_models().await?;

        Ok(info)
    }

    pub fn configure_provider(
        &mut self,
        provider_id: &str,
        base_url: Option<String>,
        api_key: Option<String>,
    ) -> Result<()> {
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
            entry.base_url = url;
        }
        if let Some(key) = api_key {
            entry.api_key = Some(key);
        }

        if entry.base_url.is_empty() {
            if let Some(url) = default_base_url {
                entry.base_url = url;
            }
        }

        self.config.save()?;

        if self.config.selected_provider.as_deref() == Some(provider_id) {
            self.runtime = Some(
                self.provider_registry
                    .resolve_runtime(provider_id, &self.config)?,
            );
        }

        Ok(())
    }

    pub fn add_custom_provider(&mut self, cp: CustomProviderConfig) -> Result<()> {
        if self
            .provider_registry
            .list_all(&self.config)
            .iter()
            .any(|p| p.id == cp.id)
        {
            return Err(BimoError::Provider(format!(
                "a provider with id '{}' already exists",
                cp.id
            )));
        }
        self.config.custom_providers.push(cp);
        self.config.save()?;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Model management
    // -----------------------------------------------------------------------

    pub async fn fetch_models(&mut self) -> Result<Vec<ModelInfo>> {
        let runtime = self
            .runtime
            .as_ref()
            .ok_or_else(|| BimoError::Provider("no provider selected".into()))?;

        let models = model::fetch_models_for_provider(runtime).await?;
        self.available_models = models.clone();
        Ok(models)
    }

    pub fn list_models(&self) -> &[ModelInfo] {
        &self.available_models
    }

    pub fn select_model(&mut self, model_id: &str) -> Result<()> {
        let exists = self.available_models.iter().any(|m| m.id == model_id);
        if !exists && !self.available_models.is_empty() {
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
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Chat
    // -----------------------------------------------------------------------

    pub async fn chat(&mut self, user_message: &str) -> Result<ChatResponse> {
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

        self.session.add_user_message(user_message);
        let messages = self.session.to_chat_messages();
        let response = provider::chat_completion(&runtime, &messages, model).await?;
        self.session.add_assistant_message(&response.content);

        Ok(ChatResponse {
            content: response.content,
            model: response.model,
            usage: response.usage,
            session_id: self.session.id.clone(),
        })
    }

    // -----------------------------------------------------------------------
    // Commands
    // -----------------------------------------------------------------------

    pub async fn execute_command(&mut self, input: &str) -> Result<CommandResult> {
        let mut ctx = self.build_command_context();
        let result = self.command_registry.dispatch_async(input, &mut ctx).await?;

        // Handle special post-command actions
        let command_name = result.command.clone();

        match command_name.as_str() {
            "session" => {
                self.handle_session_command(&result)?;
            }
            "compact" => {
                if ctx.compact_requested {
                    self.compact_session().await?;
                    return Ok(CommandResult {
                        command: "compact".into(),
                        output: "Session context compacted successfully.".into(),
                        data: None,
                    });
                }
            }
            _ => {}
        }

        self.sync_from_command_context(&ctx);
        Ok(result)
    }

    pub fn clear_session(&mut self) {
        self.session.clear();
    }

    // -----------------------------------------------------------------------
    // Session operations
    // -----------------------------------------------------------------------

    fn handle_session_command(&mut self, result: &CommandResult) -> Result<()> {
        let data = match &result.data {
            Some(d) => d,
            None => return Ok(()),
        };

        // Handle save
        if result.output == "Session saved." {
            return self.session.save();
        }

        // Handle purge
        if result.output == "All saved sessions purged." {
            return Session::delete_all_saved();
        }

        // Handle delete — extract session_id from data
        if let Some(id) = data.get("session_id").and_then(|v| v.as_str()) {
            if result.output.starts_with("Deleted session") {
                return Session::delete_saved(id);
            }
        }

        Ok(())
    }

    /// Compact the session by summarizing it via the provider.
    async fn compact_session(&mut self) -> Result<()> {
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
                };
                format!("{role}: {}", m.content)
            })
            .collect();

        let prompt = format!(
            "Summarize the following conversation concisely, preserving all key information, \
             decisions, code snippets, file paths, and important context. The summary will be \
             used as context for continuing the conversation.\n\n\
             Conversation:\n{}",
            conversation.join("\n\n")
        );

        let messages = vec![provider::ChatMessage {
            role: "user".into(),
            content: prompt,
        }];

        let response = provider::chat_completion(&runtime, &messages, model).await?;
        self.session.compact(&response.content);

        // Save the compacted session
        self.session.save()?;

        Ok(())
    }

    /// Save the current session to disk.
    pub fn save_session(&mut self) -> Result<()> {
        self.session.save()
    }

    /// Resume a saved session by id (supports prefix matching).
    pub fn resume_session(&mut self, id: &str) -> Result<()> {
        let sessions = Session::list_saved()?;
        let found = sessions
            .iter()
            .find(|s| s.id == id || s.id.starts_with(id))
            .ok_or_else(|| BimoError::Session(format!("session '{id}' not found")))?;

        let loaded = Session::load(&found.id)?;
        self.session = loaded;
        Ok(())
    }

    /// Delete a saved session by id.
    pub fn delete_session(&mut self, id: &str) -> Result<()> {
        Session::delete_saved(id)
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    fn build_command_context(&self) -> CommandContext {
        let providers = self.provider_registry.list_all(&self.config);
        let saved_sessions = Session::list_saved().unwrap_or_default();
        CommandContext {
            selected_provider: self.config.selected_provider.clone(),
            selected_model: self.config.selected_model.clone(),
            available_models: self.available_models.clone(),
            session_id: self.session.id.clone(),
            session_message_count: self.session.message_count(),
            provider_ids: providers.iter().map(|p| p.id.clone()).collect(),
            provider_names: providers.iter().map(|p| p.name.clone()).collect(),
            needs_configuration: self.needs_configuration(),
            tools: self.tool_registry.list().to_vec(),
            saved_sessions,
            compact_requested: false,
            has_runtime: self.runtime.is_some(),
        }
    }

    fn sync_from_command_context(&mut self, ctx: &CommandContext) {
        self.config.selected_provider = ctx.selected_provider.clone();
        self.config.selected_model = ctx.selected_model.clone();
    }
}
