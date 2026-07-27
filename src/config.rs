use crate::error::{BimoError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Persisted application configuration, stored as JSON on disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// The id of the currently selected provider, if any.
    pub selected_provider: Option<String>,

    /// The id of the currently selected model, if any.
    pub selected_model: Option<String>,

    /// Per-provider configuration (base URLs, API keys, etc.).
    pub provider_configs: HashMap<String, ProviderPersistedConfig>,

    /// User-registered custom providers.
    pub custom_providers: Vec<CustomProviderConfig>,
}

/// Configuration that is persisted for a single provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderPersistedConfig {
    pub base_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
}

/// A user-registered custom provider definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomProviderConfig {
    pub id: String,
    pub name: String,
    pub category: String, // "local" or "cloud"
    pub base_url: String,
    pub api_key_required: bool,
    pub chat_endpoint: String,
    pub models_endpoint: Option<String>,
    /// Header name used for the API key, e.g. "Authorization".
    pub auth_header: Option<String>,
    /// Prefix before the key in the auth header, e.g. "Bearer ".
    pub auth_prefix: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            selected_provider: None,
            selected_model: None,
            provider_configs: HashMap::new(),
            custom_providers: Vec::new(),
        }
    }
}

impl AppConfig {
    /// Returns the configuration directory path (`~/.bimo`).
    fn config_dir() -> Result<PathBuf> {
        let home = dirs::home_dir()
            .ok_or_else(|| BimoError::Config("cannot determine home directory".into()))?;
        Ok(home.join(".bimo"))
    }

    /// Returns the path to the config file (`~/.bimo/config.json`).
    fn config_path() -> Result<PathBuf> {
        Ok(Self::config_dir()?.join("config.json"))
    }

    /// Load configuration from disk, or return defaults if the file does not exist.
    pub fn load() -> Self {
        match Self::try_load() {
            Ok(cfg) => cfg,
            Err(_) => Self::default(),
        }
    }

    fn try_load() -> Result<Self> {
        let path = Self::config_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let data = fs::read_to_string(&path)
            .map_err(|e| BimoError::Config(format!("failed to read config: {e}")))?;
        let cfg: AppConfig = serde_json::from_str(&data)
            .map_err(|e| BimoError::Config(format!("failed to parse config: {e}")))?;
        Ok(cfg)
    }

    /// Persist the current configuration to disk.
    pub fn save(&self) -> Result<()> {
        let dir = Self::config_dir()?;
        fs::create_dir_all(&dir)
            .map_err(|e| BimoError::Config(format!("failed to create config dir: {e}")))?;
        let path = Self::config_path()?;
        let data = serde_json::to_string_pretty(self)
            .map_err(|e| BimoError::Config(format!("failed to serialize config: {e}")))?;
        fs::write(&path, data)
            .map_err(|e| BimoError::Config(format!("failed to write config: {e}")))?;
        Ok(())
    }
}
