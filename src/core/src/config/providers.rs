use serde::{Deserialize, Serialize};

use crate::models::ModelRegistry;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub name: String,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvidersFile {
    pub providers: Vec<ProviderConfig>,
    #[serde(default)]
    pub default: Option<String>,
}

impl ProvidersFile {
    pub fn path() -> std::path::PathBuf {
        let base = dirs::config_dir().unwrap_or_else(|| std::path::PathBuf::from("~/.config"));
        base.join("bimo").join("providers.json")
    }

    pub fn load() -> crate::Result<Self> {
        let path = Self::path();
        if !path.exists() {
            return Ok(Self {
                providers: Vec::new(),
                default: None,
            });
        }
        let content = std::fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&content)?)
    }

    pub fn save(&self) -> crate::Result<()> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    pub fn default_provider(&self) -> Option<&ProviderConfig> {
        let default_name = self.default.as_deref()?;
        self.providers.iter().find(|p| p.name == default_name)
    }

    /// Resolve provider config against the models.dev registry:
    /// automatically fill missing `base_url` from the registry.
    pub async fn resolve_from_registry(&mut self, registry: &ModelRegistry) {
        for provider in &mut self.providers {
            if provider.base_url.is_none() {
                provider.base_url = registry.provider_base_url(&provider.name).await;
            }
        }
    }
}
