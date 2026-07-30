use crate::config::{read_json, write_json, ensure_config_dir};
use crate::error::Result;
use serde::{Deserialize, Serialize};

const SETTINGS_FILE: &str = "settings.json";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ThinkingConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub budget_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_model: Option<String>,
    #[serde(default)]
    pub thinking: ThinkingConfig,
    /// Max tool call iterations per chat request. None = no limit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tool_iterations: Option<usize>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            selected_provider: None,
            selected_model: None,
            thinking: ThinkingConfig::default(),
            max_tool_iterations: None,
        }
    }
}

impl Settings {
    pub fn load() -> Self {
        match read_json(SETTINGS_FILE) {
            Ok(s) => s,
            Err(_) => {
                let s = Self::default();
                let _ = s.save();
                s
            }
        }
    }

    pub fn save(&self) -> Result<()> {
        ensure_config_dir()?;
        write_json(SETTINGS_FILE, self)
    }
}
