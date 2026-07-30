use std::time::Duration;

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
pub struct LocalProvider {
    pub name: String,
    pub base_url: String,
    #[serde(default)]
    pub models: Vec<String>,
}

impl LocalProvider {
    pub fn ollama() -> Self {
        Self {
            name: "ollama".to_string(),
            base_url: "http://localhost:11434/v1".to_string(),
            models: vec![],
        }
    }

    pub fn lmstudio() -> Self {
        Self {
            name: "lmstudio".to_string(),
            base_url: "http://localhost:1234/v1".to_string(),
            models: vec![],
        }
    }

    /// Fetch available models from the provider's OpenAI-compatible `/v1/models` endpoint.
    /// Works for Ollama, LM Studio, and any OpenAI-compatible local provider.
    pub async fn fetch_models(&self) -> crate::Result<Vec<String>> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .map_err(|e| crate::BimoError::Msg(format!("Failed to create HTTP client: {e}")))?;

        let base = self.base_url.trim_end_matches('/');
        let url = format!("{base}/models");
        let resp = client
            .get(&url)
            .send()
            .await
            .map_err(|e| crate::BimoError::Msg(format!("Failed to fetch models: {e}")))?;
        let data: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| crate::BimoError::Msg(format!("Failed to parse models response: {e}")))?;
        let models = data["data"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| m["id"].as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        Ok(models)
    }

    /// Auto-discover models and update `self.models` with the result.
    pub async fn auto_discover_models(&mut self) {
        match self.fetch_models().await {
            Ok(models) if !models.is_empty() => {
                self.models = models;
            }
            _ => {
                // Keep existing models list (user-provided or empty) if discovery fails
            }
        }
    }
}

fn default_local_providers() -> Vec<LocalProvider> {
    vec![LocalProvider::ollama(), LocalProvider::lmstudio()]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvidersFile {
    pub providers: Vec<ProviderConfig>,
    #[serde(default)]
    pub default: Option<String>,
    #[serde(default = "default_local_providers")]
    pub local_providers: Vec<LocalProvider>,
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
                local_providers: default_local_providers(),
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

    pub async fn resolve_from_registry(&mut self, registry: &ModelRegistry) {
        for provider in &mut self.providers {
            if provider.base_url.is_none() {
                provider.base_url = registry.provider_base_url(&provider.name).await;
            }
        }
    }
}
