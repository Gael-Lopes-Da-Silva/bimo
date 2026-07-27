use crate::command::{CommandContext, CommandRegistry, CommandResult};
use crate::config::{AppConfig, CustomProviderConfig};
use crate::error::{BimoError, Result};
use crate::model::{self, ModelInfo};
use crate::provider::{self, ProviderInfo, ProviderRegistry, ProviderRuntime};
use crate::session::Session;

/// The core agent that holds all state and coordinates operations.
pub struct Agent {
    pub config: AppConfig,
    pub session: Session,
    pub provider_registry: ProviderRegistry,
    pub available_models: Vec<ModelInfo>,
    pub runtime: Option<ProviderRuntime>,
    pub command_registry: CommandRegistry,
}

impl Agent {
    /// Create a new agent with no provider selected.
    pub fn new() -> Self {
        let config = AppConfig::load();
        let provider_registry = ProviderRegistry::new();
        let command_registry = CommandRegistry::new();
        let session = Session::new();

        // If a provider was previously selected, try to restore it.
        let mut agent = Self {
            config,
            session,
            provider_registry,
            available_models: Vec::new(),
            runtime: None,
            command_registry,
        };

        if let Some(pid) = agent.config.selected_provider.clone() {
            if agent
                .provider_registry
                .resolve_runtime(&pid, &agent.config)
                .is_ok()
            {
                agent.runtime = Some(
                    agent
                        .provider_registry
                        .resolve_runtime(&pid, &agent.config)
                        .unwrap(),
                );
                // Try to load saved models (we don't persist them, so re-fetch
                // will happen lazily).
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

    /// List all available providers.
    pub fn list_providers(&self) -> Vec<ProviderInfo> {
        self.provider_registry.list_all(&self.config)
    }

    /// Select a provider by id. Clears any cached runtime and available models.
    pub async fn select_provider(&mut self, provider_id: &str) -> Result<ProviderInfo> {
        let info = self
            .provider_registry
            .list_all(&self.config)
            .into_iter()
            .find(|p| p.id == provider_id)
            .ok_or_else(|| BimoError::Provider(format!("unknown provider '{provider_id}'")))?;

        // Try to build the runtime (may fail if API key is missing).
        let runtime = self
            .provider_registry
            .resolve_runtime(provider_id, &self.config)?;

        self.runtime = Some(runtime);
        self.config.selected_provider = Some(provider_id.to_string());
        self.available_models.clear();
        self.config.selected_model = None;
        self.config.save()?;

        // Fetch models for the newly selected provider.
        self.fetch_models().await?;

        Ok(info)
    }

    /// Configure a provider with a custom base URL and/or API key.
    pub fn configure_provider(
        &mut self,
        provider_id: &str,
        base_url: Option<String>,
        api_key: Option<String>,
    ) -> Result<()> {
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

        // If the builtin default base URL is not set yet, fill it.
        if entry.base_url.is_empty() {
            if let Some(info) = self
                .provider_registry
                .list_all(&self.config)
                .iter()
                .find(|p| p.id == provider_id)
            {
                entry.base_url = info.default_base_url.clone();
            }
        }

        self.config.save()?;

        // Rebuild runtime if this is the currently selected provider.
        if self.config.selected_provider.as_deref() == Some(provider_id) {
            self.runtime = Some(
                self.provider_registry
                    .resolve_runtime(provider_id, &self.config)?,
            );
        }

        Ok(())
    }

    /// Register a custom provider.
    pub fn add_custom_provider(&mut self, cp: CustomProviderConfig) -> Result<()> {
        // Reject duplicate id with builtins
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

    /// Fetch available models for the currently selected provider.
    pub async fn fetch_models(&mut self) -> Result<Vec<ModelInfo>> {
        let runtime = self
            .runtime
            .as_ref()
            .ok_or_else(|| BimoError::Provider("no provider selected".into()))?;

        let models = model::fetch_models_for_provider(runtime).await?;
        self.available_models = models.clone();
        Ok(models)
    }

    /// List currently cached models.
    pub fn list_models(&self) -> &[ModelInfo] {
        &self.available_models
    }

    /// Select a model by id.
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

    /// Send a user message and get a response from the provider.
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

        // Add the user message to the session
        self.session.add_user_message(user_message);

        // Build messages for the API call
        let messages = self.session.to_chat_messages();

        // Call the provider
        let response = provider::chat_completion(&runtime, &messages, model).await?;

        // Add the assistant response to the session
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

    /// Parse and execute a slash command.
    pub fn execute_command(&mut self, input: &str) -> Result<CommandResult> {
        let mut ctx = self.build_command_context();
        let result = self.command_registry.dispatch(input, &mut ctx)?;
        // Sync state back from context
        self.sync_from_command_context(&ctx);
        Ok(result)
    }

    /// Clear the current session.
    pub fn clear_session(&mut self) {
        self.session.clear();
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    fn build_command_context(&self) -> CommandContext {
        CommandContext {
            selected_provider: self.config.selected_provider.clone(),
            selected_model: self.config.selected_model.clone(),
            available_models: self.available_models.clone(),
            session_id: self.session.id.clone(),
            session_message_count: self.session.message_count(),
            provider_ids: self
                .provider_registry
                .list_all(&self.config)
                .iter()
                .map(|p| p.id.clone())
                .collect(),
            provider_names: self
                .provider_registry
                .list_all(&self.config)
                .iter()
                .map(|p| p.name.clone())
                .collect(),
            needs_configuration: self.needs_configuration(),
        }
    }

    fn sync_from_command_context(&mut self, ctx: &CommandContext) {
        self.config.selected_provider = ctx.selected_provider.clone();
        self.config.selected_model = ctx.selected_model.clone();
    }
}

use crate::provider::UsageInfo;

/// The response from a chat interaction.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChatResponse {
    pub content: String,
    pub model: Option<String>,
    pub usage: Option<UsageInfo>,
    pub session_id: String,
}

use crate::config::ProviderPersistedConfig;
