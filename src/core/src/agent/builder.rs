use std::path::PathBuf;

use crate::agent::agent::AgentRunner;
use crate::agent::instructions::load_instructions;
use crate::agent::Agent;
use crate::config::{ProviderConfig, Settings};
use crate::error::Result;
use crate::prompt::PromptEngine;

pub struct AgentBuilder {
    provider: Option<ProviderConfig>,
    settings: Settings,
    project_dir: Option<PathBuf>,
    session: Option<crate::session::Session>,
    max_steps: Option<usize>,
    temperature: Option<f32>,
    max_tokens: Option<u32>,
}

impl AgentBuilder {
    pub fn new() -> Self {
        Self {
            provider: None,
            settings: Settings::default(),
            project_dir: None,
            session: None,
            max_steps: None,
            temperature: None,
            max_tokens: None,
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

    pub fn build(self) -> Result<Agent> {
        let provider = self.provider.ok_or_else(|| {
            crate::error::BimoError::Config("No provider configured".to_string())
        })?;

        let project_dir_str = self
            .project_dir
            .as_ref()
            .and_then(|p| p.to_str());

        let instructions = load_instructions(project_dir_str);

        let prompt_engine = PromptEngine::new(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("prompts"),
        );

        let system_prompt = prompt_engine.render_default(&std::collections::HashMap::from([
            ("INSTRUCTIONS".to_string(), instructions),
        ]));

        let max_steps = self.max_steps.unwrap_or(self.settings.max_steps);

        let runner = AgentRunner {
            provider_name: provider.name,
            provider_model: provider.model,
            provider_api_key: provider.api_key,
            provider_base_url: provider.base_url,
            system_prompt,
            max_steps,
            temperature: self.temperature,
            max_tokens: self.max_tokens,
        };

        let session = self.session.unwrap_or_else(crate::session::Session::new);

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
