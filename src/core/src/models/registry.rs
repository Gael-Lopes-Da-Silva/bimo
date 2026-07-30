use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

use super::types::{ModelEntry, ProviderEntry, ProviderMap};
use crate::error::{BimoError, Result};

const MODELS_DEV_URL: &str = "https://models.dev/api.json";

pub struct ModelRegistry {
    providers: Arc<RwLock<ProviderMap>>,
    cache_path: PathBuf,
}

impl ModelRegistry {
    pub fn new() -> Self {
        let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("~/.config"));
        Self {
            providers: Arc::new(RwLock::new(HashMap::new())),
            cache_path: base.join("bimo").join("models_cache.json"),
        }
    }

    /// Fetch the registry from the network or load from disk cache.
    pub async fn load(&self) -> Result<()> {
        match self.fetch_remote().await {
            Ok(providers) => {
                let mut map = self.providers.write().await;
                *map = providers;
                info!("Loaded {} providers from models.dev", map.len());
            }
            Err(e) => {
                warn!("Failed to fetch models.dev: {e}; trying cache");
                self.load_cache().await?;
            }
        }
        Ok(())
    }

    /// Force a fresh fetch from the remote URL.
    pub async fn refresh(&self) -> Result<()> {
        let providers = self.fetch_remote().await.map_err(BimoError::Msg)?;
        let mut map = self.providers.write().await;
        *map = providers;
        self.save_cache().await?;
        Ok(())
    }

    async fn fetch_remote(&self) -> std::result::Result<ProviderMap, String> {
        let resp = reqwest::get(MODELS_DEV_URL)
            .await
            .map_err(|e| format!("HTTP request failed: {e}"))?;

        let bytes = resp
            .bytes()
            .await
            .map_err(|e| format!("Failed to read response body: {e}"))?;

        let providers: ProviderMap =
            serde_json::from_slice(&bytes).map_err(|e| format!("JSON parse failed: {e}"))?;

        // Persist to cache
        if let Err(e) = std::fs::write(&self.cache_path, &bytes) {
            warn!("Failed to write models cache: {e}");
        }

        Ok(providers)
    }

    async fn load_cache(&self) -> Result<()> {
        if !self.cache_path.exists() {
            return Err(crate::error::BimoError::Other(
                "No models.dev cache available; run refresh()".to_string(),
            ));
        }
        let content = tokio::fs::read_to_string(&self.cache_path).await?;
        let providers: ProviderMap = serde_json::from_str(&content)?;
        let mut map = self.providers.write().await;
        *map = providers;
        info!("Loaded {} providers from cache", map.len());
        Ok(())
    }

    async fn save_cache(&self) -> Result<()> {
        if let Some(parent) = self.cache_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let map = self.providers.read().await;
        let content = serde_json::to_string_pretty(&*map)?;
        tokio::fs::write(&self.cache_path, &content).await?;
        Ok(())
    }

    /// List all providers in the registry.
    pub async fn list_providers(&self) -> Vec<ProviderEntry> {
        let map = self.providers.read().await;
        let mut providers: Vec<ProviderEntry> = map.values().cloned().collect();
        providers.sort_by(|a, b| a.name.cmp(&b.name));
        providers
    }

    /// Find a provider by ID or name.
    pub async fn find_provider(&self, id_or_name: &str) -> Option<ProviderEntry> {
        let map = self.providers.read().await;
        let lower = id_or_name.to_lowercase();

        map.get(id_or_name).cloned().or_else(|| {
            map.values()
                .find(|p| p.name.to_lowercase() == lower)
                .cloned()
        })
    }

    /// List models for a given provider.
    pub async fn list_models(&self, provider_id: &str) -> Vec<ModelEntry> {
        let map = self.providers.read().await;
        map.get(provider_id)
            .map(|p| {
                let mut models: Vec<ModelEntry> = p.models.values().cloned().collect();
                models.sort_by(|a, b| a.name.cmp(&b.name));
                models
            })
            .unwrap_or_default()
    }

    /// Find a specific model across all providers.
    pub async fn find_model(&self, model_id: &str) -> Option<(String, ModelEntry)> {
        let map = self.providers.read().await;
        for (pid, provider) in map.iter() {
            if let Some(model) = provider.models.get(model_id) {
                return Some((pid.clone(), model.clone()));
            }
        }
        None
    }

    /// Get the base URL for a provider from the registry.
    pub async fn provider_base_url(&self, provider_id: &str) -> Option<String> {
        let map = self.providers.read().await;
        map.get(provider_id).and_then(|p| {
            p.api.as_ref().and_then(|v| match v {
                serde_json::Value::String(s) => Some(s.clone()),
                _ => None,
            })
        })
    }

    /// Get the environment variables needed for a provider.
    pub async fn provider_env_vars(&self, provider_id: &str) -> Vec<String> {
        let map = self.providers.read().await;
        map.get(provider_id)
            .map(|p| p.env.clone())
            .unwrap_or_default()
    }

    /// Number of loaded providers.
    pub async fn provider_count(&self) -> usize {
        let map = self.providers.read().await;
        map.len()
    }

    /// Number of total models across all providers.
    pub async fn model_count(&self) -> usize {
        let map = self.providers.read().await;
        map.values().map(|p| p.models.len()).sum()
    }
}

impl Default for ModelRegistry {
    fn default() -> Self {
        Self::new()
    }
}
