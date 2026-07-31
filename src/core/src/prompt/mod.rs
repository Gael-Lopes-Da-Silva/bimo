//! Prompt template rendering — embeds and renders system prompts at compile time.

use std::collections::HashMap;

const SYSTEM_PROMPT: &str = include_str!("prompts/SYSTEM.md");
const SUMMARY_PROMPT: &str = include_str!("prompts/SUMMARY.md");
const SESSION_NAME_PROMPT: &str = include_str!("prompts/SESSION_NAME.md");
const COMPACT_PROMPT: &str = include_str!("prompts/COMPACT.md");

/// Renders a template string by replacing `{{KEY}}` placeholders with values.
/// Unknown placeholders are left as-is.
pub fn render_template(template: &str, vars: &HashMap<String, String>) -> String {
    let mut result = template.to_string();
    let mut keys: Vec<&String> = vars.keys().collect();
    // Sort by key length descending so longer placeholders are replaced first,
    // preventing partial replacements.
    keys.sort_by(|a, b| b.len().cmp(&a.len()).then(a.cmp(b)));
    for key in keys {
        let value = vars.get(key).unwrap();
        let placeholder = format!("{{{{{key}}}}}");
        result = result.replace(&placeholder, value);
    }
    result
}

/// Loads and renders system prompt templates embedded at compile time.
/// Templates use `{{PLACEHOLDER}}` notation for variable substitution.
pub struct PromptEngine;

impl PromptEngine {
    /// Creates a new prompt engine.
    pub fn new() -> Self {
        Self
    }

    /// Returns the embedded system prompt template.
    pub fn system_template() -> &'static str {
        SYSTEM_PROMPT
    }

    /// Returns the embedded summary prompt template.
    pub fn summary_template() -> &'static str {
        SUMMARY_PROMPT
    }

    /// Returns the embedded compaction prompt template.
    pub fn compact_template() -> &'static str {
        COMPACT_PROMPT
    }

    /// Formats a slice of session messages as a single conversation block,
    /// one `[role] timestamp\ncontent` entry per message.
    pub fn format_messages(messages: &[crate::session::Message]) -> String {
        messages
            .iter()
            .map(|m| {
                format!(
                    "[{}] {}\n{}",
                    m.role,
                    m.timestamp.format("%Y-%m-%d %H:%M:%S"),
                    m.content
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    /// Renders the system prompt, injecting `vars` on top of the built-in
    /// `{{DATE}}` and `{{CWD}}` values.
    pub fn render_system(vars: &HashMap<String, String>) -> String {
        let mut default_vars = HashMap::new();
        default_vars.insert(
            "DATE".to_string(),
            chrono::Utc::now().date_naive().to_string(),
        );

        let cwd = match std::env::current_dir() {
            Ok(p) => p.to_string_lossy().to_string(),
            Err(e) => {
                tracing::warn!("Failed to get current directory: {}", e);
                "unknown".to_string()
            }
        };
        default_vars.insert("CWD".to_string(), cwd);

        for (k, v) in vars {
            default_vars.insert(k.clone(), v.clone());
        }

        render_template(SYSTEM_PROMPT, &default_vars)
    }

    /// Renders the summary prompt for a given summary text.
    pub fn render_summary(summary: &str) -> String {
        let mut vars = HashMap::new();
        vars.insert("SUMMARY".to_string(), summary.to_string());
        render_template(SUMMARY_PROMPT, &vars)
    }

    /// Renders the session-naming prompt for a given conversation context.
    pub fn render_session_name(context: &str) -> String {
        let mut vars = HashMap::new();
        vars.insert("CONTEXT".to_string(), context.to_string());
        render_template(SESSION_NAME_PROMPT, &vars)
    }

    /// Renders the compaction prompt for a given conversation history.
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
