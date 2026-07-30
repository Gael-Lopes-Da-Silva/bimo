use crate::config::{read_json, write_json, ensure_config_dir};
use crate::error::{BimoError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const PROVIDERS_FILE: &str = "providers.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderPersistedConfig {
    pub base_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomProviderConfig {
    pub id: String,
    pub name: String,
    pub category: String,
    pub base_url: String,
    pub api_key_required: bool,
    pub chat_endpoint: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub models_endpoint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_header: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_prefix: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProvidersConfig {
    /// Per-provider configuration (base URLs, API keys, etc.).
    #[serde(default)]
    pub configured: HashMap<String, ProviderPersistedConfig>,
    /// User-registered custom providers.
    #[serde(default)]
    pub custom: Vec<CustomProviderConfig>,
}

impl ProvidersConfig {
    pub fn load() -> Self {
        match read_json(PROVIDERS_FILE) {
            Ok(p) => p,
            Err(_) => {
                let p = Self::default();
                let _ = p.save();
                p
            }
        }
    }

    pub fn save(&self) -> Result<()> {
        ensure_config_dir()?;
        write_json(PROVIDERS_FILE, self)
    }

    pub fn configure_provider(&mut self, id: &str, base_url: Option<String>, api_key: Option<String>, default_base_url: &str) -> Result<()> {
        let entry = self.configured.entry(id.to_string()).or_insert_with(|| ProviderPersistedConfig {
            base_url: default_base_url.to_string(),
            api_key: None,
        });
        if let Some(url) = base_url {
            entry.base_url = url;
        }
        if entry.base_url.is_empty() {
            entry.base_url = default_base_url.to_string();
        }
        if let Some(key) = api_key {
            entry.api_key = Some(key);
        }
        self.save()
    }

    pub fn add_custom(&mut self, cp: CustomProviderConfig) -> Result<()> {
        if self.custom.iter().any(|p| p.id == cp.id) {
            return Err(BimoError::Provider(format!("a provider with id '{}' already exists", cp.id)));
        }
        self.custom.push(cp);
        self.save()
    }
}
