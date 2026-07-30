//! Provider registry — fetches and caches the models.dev provider catalogue.

use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::RwLock;
use tracing::{info, warn};

use super::types::{ProviderEntry, ProviderMap};
use crate::config::{ApiFormat, Provider};
use crate::error::{BimoError, Result};

const MODELS_DEV_URL: &str = "https://models.dev/api.json";

/// Thread-safe registry of providers from models.dev.
///
/// Data is fetched from the remote API on first load and cached to
/// `~/.config/bimo/models_cache.json` so it remains available offline.
#[derive(Debug, Clone)]
pub struct ProviderRegistry {
    providers: Arc<RwLock<ProviderMap>>,
    cache_path: PathBuf,
}

impl ProviderRegistry {
    /// Creates a new empty registry.
    ///
    /// Call [`load`](Self::load) or [`refresh`](Self::refresh) to populate it.
    pub fn new() -> Self {
        let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("~/.config"));
        Self {
            providers: Arc::new(RwLock::new(Default::default())),
            cache_path: base.join("bimo").join("models_cache.json"),
        }
    }

    /// Attempts to fetch from models.dev; falls back to the local cache on failure.
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

    /// Forces a fresh fetch from models.dev and updates the local cache.
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

        if let Err(e) = std::fs::write(&self.cache_path, &bytes) {
            warn!("Failed to write models cache: {e}");
        }

        Ok(providers)
    }

    async fn load_cache(&self) -> Result<()> {
        if !self.cache_path.exists() {
            return Err(BimoError::Other(
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

    /// Returns all providers from the registry as [`Provider`] instances.
    ///
    /// The returned providers have no API key set (the caller must supply it).
    pub async fn builtin_cloud_providers(&self) -> Vec<Provider> {
        let map = self.providers.read().await;
        let mut providers: Vec<Provider> = map.values().map(|e| e.to_provider()).collect();
        providers.sort_by(|a, b| a.id.cmp(&b.id));
        providers
    }

    /// Returns every provider entry from the registry, sorted by name.
    pub async fn list_providers(&self) -> Vec<ProviderEntry> {
        let map = self.providers.read().await;
        let mut providers: Vec<ProviderEntry> = map.values().cloned().collect();
        providers.sort_by(|a, b| a.name.cmp(&b.name));
        providers
    }

    /// Looks up a provider by id (exact) or name (case-insensitive).
    pub async fn find_provider(&self, id_or_name: &str) -> Option<ProviderEntry> {
        let map = self.providers.read().await;
        let lower = id_or_name.to_lowercase();
        map.get(id_or_name).cloned().or_else(|| {
            map.values()
                .find(|p| p.name.to_lowercase() == lower)
                .cloned()
        })
    }

    /// Returns the base URL for a given provider id, if known.
    pub async fn provider_base_url(&self, provider_id: &str) -> Option<String> {
        let map = self.providers.read().await;
        map.get(provider_id).and_then(|p| p.base_url())
    }

    /// Returns the environment variable names required by a provider.
    pub async fn provider_env_vars(&self, provider_id: &str) -> Vec<String> {
        let map = self.providers.read().await;
        map.get(provider_id)
            .map(|p| p.env.clone())
            .unwrap_or_default()
    }

    /// Returns the API format for a provider, if known.
    pub async fn provider_api_format(&self, provider_id: &str) -> Option<ApiFormat> {
        let map = self.providers.read().await;
        map.get(provider_id).map(|p| p.api_format())
    }

    /// Returns a reference to the inner provider map for sharing with `ModelRegistry`.
    pub fn providers_ref(&self) -> &Arc<RwLock<ProviderMap>> {
        &self.providers
    }

    /// Number of providers currently in the registry.
    pub async fn provider_count(&self) -> usize {
        let map = self.providers.read().await;
        map.len()
    }
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}
