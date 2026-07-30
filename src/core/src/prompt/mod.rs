mod template;

use std::collections::HashMap;

pub use template::render_template;

const SYSTEM_PROMPT: &str = include_str!("prompts/SYSTEM.md");
const SUMMARY_PROMPT: &str = include_str!("prompts/SUMMARY.md");
const COMPACT_PROMPT: &str = include_str!("prompts/COMPACT.md");

/// Loads and renders system prompt templates embedded at compile time.
/// Templates use `{{PLACEHOLDER}}` notation for variable substitution.
pub struct PromptEngine;

impl PromptEngine {
    pub fn new() -> Self {
        Self
    }

    /// Get the SYSTEM prompt template.
    pub fn system_template() -> &'static str {
        SYSTEM_PROMPT
    }

    /// Get the SUMMARY prompt template.
    pub fn summary_template() -> &'static str {
        SUMMARY_PROMPT
    }

    /// Get the COMPACT prompt template.
    pub fn compact_template() -> &'static str {
        COMPACT_PROMPT
    }

    /// Render the system prompt with all variables.
    /// Expected vars: `PROJECT_CONTEXT`, `TOOLS`.
    pub fn render_system(vars: &HashMap<String, String>) -> String {
        let mut default_vars = HashMap::new();
        default_vars.insert(
            "DATE".to_string(),
            chrono::Utc::now().date_naive().to_string(),
        );
        default_vars.insert(
            "CWD".to_string(),
            std::env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| "unknown".to_string()),
        );

        for (k, v) in vars {
            default_vars.insert(k.clone(), v.clone());
        }

        render_template(SYSTEM_PROMPT, &default_vars)
    }

    /// Render the SUMMARY template.
    pub fn render_summary(summary: &str) -> String {
        let mut vars = HashMap::new();
        vars.insert("SUMMARY".to_string(), summary.to_string());
        render_template(SUMMARY_PROMPT, &vars)
    }

    /// Render the COMPACT template.
    pub fn render_compact(conversation: &str) -> String {
        let mut vars = HashMap::new();
        vars.insert("CONVERSATION".to_string(), conversation.to_string());
        render_template(COMPACT_PROMPT, &vars)
    }
}

impl Default for PromptEngine {
    fn default() -> Self {
        Self::new()
    }
}
