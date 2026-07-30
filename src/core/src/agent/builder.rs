use std::collections::HashMap;
use std::path::PathBuf;

use crate::agent::Agent;
use crate::agent::executor::AgentRunner;
use crate::agent::instructions::load_instructions;
use crate::config::Provider;
use crate::config::Settings;
use crate::error::Result;
use crate::prompt::PromptEngine;
use crate::tools;

pub struct AgentBuilder {
    provider: Option<Provider>,
    model: Option<String>,
    settings: Settings,
    project_dir: Option<PathBuf>,
    session: Option<crate::session::Session>,
    user_prompt: Option<String>,
    max_steps: Option<usize>,
    temperature: Option<f32>,
    max_tokens: Option<u32>,
}

impl AgentBuilder {
    pub fn new() -> Self {
        Self {
            provider: None,
            model: None,
            settings: Settings::default(),
            project_dir: None,
            session: None,
            user_prompt: None,
            max_steps: None,
            temperature: None,
            max_tokens: None,
        }
    }

    pub fn with_provider(mut self, provider: Provider) -> Self {
        self.provider = Some(provider);
        self
    }

    pub fn with_model(mut self, model: String) -> Self {
        self.model = Some(model);
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

    pub fn with_user_prompt(mut self, prompt: String) -> Self {
        self.user_prompt = Some(prompt);
        self
    }

    pub fn build(self) -> Result<Agent> {
        let provider = self
            .provider
            .ok_or_else(|| crate::error::BimoError::Config("No provider configured".to_string()))?;

        let model = self.model.unwrap_or_else(|| provider.id.clone());

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

        let api_format = provider.effective_api_format();

        let runner = AgentRunner {
            provider_name: provider.name,
            provider_model: model,
            provider_api_key: provider.api_key,
            provider_base_url: provider.base_url,
            api_format,
            system_prompt,
            user_prompt,
            max_steps,
            temperature: self.temperature,
            max_tokens: self.max_tokens,
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
