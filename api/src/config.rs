use crate::error::{BimoError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tracing;

/// Persisted application configuration, stored as JSON on disk.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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
            Ok(cfg) => {
                tracing::debug!(provider = ?cfg.selected_provider, model = ?cfg.selected_model, "config loaded from disk");
                cfg
            }
            Err(e) => {
                tracing::debug!(error = %e, "using default config");
                Self::default()
            }
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
        tracing::debug!("saving config to disk");
        let dir = Self::config_dir()?;
        fs::create_dir_all(&dir)
            .map_err(|e| BimoError::Config(format!("failed to create config dir: {e}")))?;
        let path = Self::config_path()?;
        let data = serde_json::to_string_pretty(self)
            .map_err(|e| BimoError::Config(format!("failed to serialize config: {e}")))?;
        fs::write(&path, data)
            .map_err(|e| BimoError::Config(format!("failed to write config: {e}")))?;
        tracing::debug!("config saved");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let cfg = AppConfig::default();
        assert!(cfg.selected_provider.is_none());
        assert!(cfg.selected_model.is_none());
        assert!(cfg.provider_configs.is_empty());
        assert!(cfg.custom_providers.is_empty());
    }

    #[test]
    fn config_is_serializable() {
        let cfg = AppConfig::default();
        let json = serde_json::to_string(&cfg).unwrap();
        let deserialized: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.selected_provider, None);
        assert_eq!(deserialized.selected_model, None);
    }

    #[test]
    fn config_with_values_is_serializable() {
        let mut provider_configs = HashMap::new();
        provider_configs.insert(
            "openai".to_string(),
            ProviderPersistedConfig {
                base_url: "https://api.openai.com/v1".to_string(),
                api_key: Some("sk-test".to_string()),
            },
        );

        let cfg = AppConfig {
            selected_provider: Some("openai".to_string()),
            selected_model: Some("gpt-4".to_string()),
            provider_configs,
            custom_providers: vec![],
        };

        let json = serde_json::to_string(&cfg).unwrap();
        let deserialized: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.selected_provider.as_deref(), Some("openai"));
        assert_eq!(deserialized.selected_model.as_deref(), Some("gpt-4"));
        assert_eq!(
            deserialized
                .provider_configs
                .get("openai")
                .unwrap()
                .api_key
                .as_deref(),
            Some("sk-test")
        );
    }

    #[test]
    fn custom_provider_config_is_serializable() {
        let cp = CustomProviderConfig {
            id: "my-custom".to_string(),
            name: "My Custom".to_string(),
            category: "cloud".to_string(),
            base_url: "https://custom.api".to_string(),
            api_key_required: true,
            chat_endpoint: "/chat".to_string(),
            models_endpoint: Some("/models".to_string()),
            auth_header: Some("Authorization".to_string()),
            auth_prefix: Some("Bearer ".to_string()),
        };

        let cfg = AppConfig {
            custom_providers: vec![cp],
            ..Default::default()
        };

        let json = serde_json::to_string(&cfg).unwrap();
        let deserialized: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.custom_providers.len(), 1);
        assert_eq!(deserialized.custom_providers[0].id, "my-custom");
        assert!(deserialized.custom_providers[0].api_key_required);
    }

    #[test]
    fn api_key_can_be_none() {
        let cfg = AppConfig {
            provider_configs: {
                let mut m = HashMap::new();
                m.insert(
                    "ollama".to_string(),
                    ProviderPersistedConfig {
                        base_url: "http://localhost:11434".to_string(),
                        api_key: None,
                    },
                );
                m
            },
            ..Default::default()
        };

        let json = serde_json::to_string(&cfg).unwrap();
        // api_key: None should be skipped in serialization
        assert!(!json.contains("api_key"));

        let deserialized: AppConfig = serde_json::from_str(&json).unwrap();
        assert!(deserialized
            .provider_configs
            .get("ollama")
            .unwrap()
            .api_key
            .is_none());
    }
}
