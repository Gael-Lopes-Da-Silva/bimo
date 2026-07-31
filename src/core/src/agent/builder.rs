//! Builder for constructing a configured [`Agent`].

use std::collections::HashMap;
use std::path::PathBuf;
use tracing::info;

use crate::agent::Agent;
use crate::agent::executor::AgentRunner;
use crate::config::Provider;
use crate::config::Settings;
use crate::error::Result;
use crate::prompt::PromptEngine;
use crate::skill;
use crate::tools;

/// Builder-pattern constructor for [`Agent`].
///
/// At minimum a provider and a user prompt must be supplied before calling
/// [`build`](Self::build).
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
    retry_attempts: Option<usize>,
    retry_timeout_secs: Option<u64>,
}

impl AgentBuilder {
    /// Creates a new builder with default settings.
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
            retry_attempts: None,
            retry_timeout_secs: None,
        }
    }

    /// Sets the provider to use.
    pub fn with_provider(mut self, provider: Provider) -> Self {
        self.provider = Some(provider);
        self
    }

    /// Sets the model string (e.g. `"claude-sonnet-4-20250514"`).
    ///
    /// Defaults to the provider's id when not specified.
    pub fn with_model(mut self, model: String) -> Self {
        self.model = Some(model);
        self
    }

    /// Overrides the default settings.
    pub fn with_settings(mut self, settings: Settings) -> Self {
        self.settings = settings;
        self
    }

    /// Sets the project root directory for instruction loading.
    pub fn with_project_dir(mut self, dir: PathBuf) -> Self {
        self.project_dir = Some(dir);
        self
    }

    /// Attaches an existing session to resume.
    pub fn with_session(mut self, session: crate::session::Session) -> Self {
        self.session = Some(session);
        self
    }

    /// Sets the maximum number of tool-call steps.
    pub fn with_max_steps(mut self, steps: usize) -> Self {
        self.max_steps = Some(steps);
        self
    }

    /// Sets the model temperature on a 0.0–1.0 scale
    /// (e.g. `0.1` for deterministic coding output).
    pub fn with_temperature(mut self, temp: f32) -> Self {
        self.temperature = Some(temp);
        self
    }

    /// Sets the maximum output tokens.
    pub fn with_max_tokens(mut self, tokens: u32) -> Self {
        self.max_tokens = Some(tokens);
        self
    }

    /// Sets the user prompt — required before calling [`build`](Self::build).
    pub fn with_user_prompt(mut self, prompt: String) -> Self {
        self.user_prompt = Some(prompt);
        self
    }

    /// Sets retry attempts for failed steps.
    pub fn with_retry_attempts(mut self, attempts: usize) -> Self {
        self.retry_attempts = Some(attempts);
        self
    }

    /// Sets retry timeout in seconds.
    pub fn with_retry_timeout(mut self, timeout: u64) -> Self {
        self.retry_timeout_secs = Some(timeout);
        self
    }

    /// Loads instruction content from project files in the following order:
    ///   1. Project root files: AGENTS.md, CLAUDE.md, CODEX.md, GEMINI.md, CONTRIBUTING.md
    ///   2. `.github/copilot-instructions.md`
    ///   3. `.agents/instructions.md`
    ///   4. `.ai/` subdirectories: `rules/`, `context/`, `workflows/`
    fn load_instructions(&self) -> String {
        let Some(project) = self.project_dir.as_deref() else {
            return String::new();
        };

        let mut instructions = String::new();

        for filename in &[
            "AGENTS.md",
            "CLAUDE.md",
            "CODEX.md",
            "GEMINI.md",
            "CONTRIBUTING.md",
        ] {
            let path = project.join(filename);
            if path.is_file()
                && let Ok(content) = std::fs::read_to_string(&path)
            {
                info!("Loaded instructions from {filename}");
                instructions.push_str(&content);
                instructions.push('\n');
            }
        }

        let copilot = project.join(".github").join("copilot-instructions.md");
        if copilot.is_file()
            && let Ok(content) = std::fs::read_to_string(&copilot)
        {
            info!("Loaded instructions from .github/copilot-instructions.md");
            instructions.push_str(&content);
            instructions.push('\n');
        }

        let agents_instructions = project.join(".agents").join("instructions.md");
        if agents_instructions.is_file()
            && let Ok(content) = std::fs::read_to_string(&agents_instructions)
        {
            info!("Loaded instructions from .agents/instructions.md");
            instructions.push_str(&content);
            instructions.push('\n');
        }

        for subdir in &["rules", "context", "workflows"] {
            let dir = project.join(".ai").join(subdir);
            if dir.is_dir()
                && let Ok(entries) = std::fs::read_dir(&dir)
            {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().is_some_and(|e| e == "md")
                        && let Ok(content) = std::fs::read_to_string(&path)
                    {
                        info!("Loaded instructions from {:?}", path);
                        instructions.push_str(&content);
                        instructions.push('\n');
                    }
                }
            }
        }

        instructions
    }

    /// Consumes the builder and produces an [`Agent`].
    ///
    /// # Errors
    ///
    /// Returns a `BimoError::Config` if no provider or user prompt was set.
    pub fn build(self) -> Result<Agent> {
        let instructions = self.load_instructions();

        let disabled_tools = self
            .session
            .as_ref()
            .map(|s| s.disabled_tools().clone())
            .unwrap_or_default();
        let disabled_skills = self
            .session
            .as_ref()
            .map(|s| s.disabled_skills().clone())
            .unwrap_or_default();

        let provider = self
            .provider
            .ok_or_else(|| crate::error::BimoError::Config("No provider configured".to_string()))?;

        let model = self.model.unwrap_or_else(|| provider.id.clone());

        let tools_desc = tools::describe_tools(&disabled_tools);

        let skill_dirs = skill::default_skill_dirs(self.project_dir.as_deref());
        let mut skills = skill::load_skills(&skill_dirs);
        for skill in &mut skills {
            if disabled_skills.contains(&skill.id) {
                skill.enabled = false;
            }
        }
        let skills_rendered = skill::render_skills(&skills);

        let system_prompt = PromptEngine::render_system(&HashMap::from([
            ("PROJECT_CONTEXT".to_string(), instructions),
            ("TOOLS".to_string(), tools_desc),
            ("SKILLS".to_string(), skills_rendered),
        ]));

        let user_prompt = self.user_prompt.ok_or_else(|| {
            crate::error::BimoError::Config("No user prompt provided".to_string())
        })?;

        let max_steps = self.max_steps.unwrap_or(self.settings.max_steps);

        let api_format = provider.api_format;

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
            debug: self.settings.debug,
            session_id: self
                .session
                .as_ref()
                .map(|s| s.id.clone())
                .unwrap_or_default(),
            disabled_tools,
            retry_attempts: self.retry_attempts.unwrap_or(self.settings.retry_attempts),
            retry_timeout_secs: self
                .retry_timeout_secs
                .unwrap_or(self.settings.retry_timeout_secs),
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
