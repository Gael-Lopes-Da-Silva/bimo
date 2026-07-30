use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::agent::Agent;
use crate::agent::executor::AgentRunner;
use crate::agent::instructions::load_instructions;
use crate::config::{LocalProvider, ProviderConfig, Settings};
use crate::error::Result;
use crate::models::ModelRegistry;
use crate::prompt::PromptEngine;
use crate::tools;

pub struct AgentBuilder {
    provider: Option<ProviderConfig>,
    settings: Settings,
    project_dir: Option<PathBuf>,
    session: Option<crate::session::Session>,
    user_prompt: Option<String>,
    local_providers: Vec<LocalProvider>,
    max_steps: Option<usize>,
    temperature: Option<f32>,
    max_tokens: Option<u32>,
    registry: Option<Arc<ModelRegistry>>,
}

impl AgentBuilder {
    pub fn new() -> Self {
        Self {
            provider: None,
            settings: Settings::default(),
            project_dir: None,
            session: None,
            user_prompt: None,
            local_providers: Vec::new(),
            max_steps: None,
            temperature: None,
            max_tokens: None,
            registry: None,
        }
    }

    pub fn with_provider(mut self, provider: ProviderConfig) -> Self {
        self.provider = Some(provider);
        self
    }

    pub fn with_settings(mut self, settings: Settings) -> Self {
        self.settings = settings;
        self
    }

    pub fn with_project_dir(mut self, dir: PathBuf) -> Self {
        self.project_dir = Some(dir);
        self
    }

    pub fn with_session(mut self, session: crate::session::Session) -> Self {
        self.session = Some(session);
        self
    }

    pub fn with_local_providers(mut self, providers: Vec<LocalProvider>) -> Self {
        self.local_providers = providers;
        self
    }

    pub fn with_max_steps(mut self, steps: usize) -> Self {
        self.max_steps = Some(steps);
        self
    }

    pub fn with_temperature(mut self, temp: f32) -> Self {
        self.temperature = Some(temp);
        self
    }

    pub fn with_max_tokens(mut self, tokens: u32) -> Self {
        self.max_tokens = Some(tokens);
        self
    }

    pub fn with_registry(mut self, registry: Arc<ModelRegistry>) -> Self {
        self.registry = Some(registry);
        self
    }

    pub fn with_user_prompt(mut self, prompt: String) -> Self {
        self.user_prompt = Some(prompt);
        self
    }

    pub fn build(self) -> Result<Agent> {
        let provider = self
            .provider
            .ok_or_else(|| crate::error::BimoError::Config("No provider configured".to_string()))?;

        let project_dir_str = self.project_dir.as_ref().and_then(|p| p.to_str());

        let instructions = load_instructions(project_dir_str);

        let tools_desc = tools::describe_tools();

        let system_prompt = PromptEngine::render_system(&HashMap::from([
            ("PROJECT_CONTEXT".to_string(), instructions),
            ("TOOLS".to_string(), tools_desc),
        ]));

        let user_prompt = self.user_prompt.ok_or_else(|| {
            crate::error::BimoError::Config("No user prompt provided".to_string())
        })?;

        let max_steps = self.max_steps.unwrap_or(self.settings.max_steps);

        let runner = AgentRunner {
            provider_name: provider.name,
            provider_model: provider.model,
            provider_api_key: provider.api_key,
            provider_base_url: provider.base_url,
            system_prompt,
            user_prompt,
            max_steps,
            temperature: self.temperature,
            max_tokens: self.max_tokens,
            local_providers: self.local_providers,
            registry: self.registry,
        };

        let session = self.session.unwrap_or_default();

        Ok(Agent {
            runner: Some(runner),
            session,
        })
    }
}

impl Default for AgentBuilder {
    fn default() -> Self {
        Self::new()
    }
}
