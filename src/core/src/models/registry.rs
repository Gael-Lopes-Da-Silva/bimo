//! Model registry — queries models from the shared provider map.

use std::sync::Arc;

use tokio::sync::RwLock;

use super::types::ModelEntry;
use crate::providers::{ProviderMap, ProviderRegistry};

/// Queries model metadata from a shared [`ProviderMap`].
///
/// Created via [`from_registry`](Self::from_registry) so it shares the same
/// in-memory data as the [`ProviderRegistry`] without duplicating it.
#[derive(Debug, Clone)]
pub struct ModelRegistry {
    providers: Arc<RwLock<ProviderMap>>,
}

impl ModelRegistry {
    /// Creates a `ModelRegistry` that shares the given registry's provider data.
    pub fn from_registry(registry: &ProviderRegistry) -> Self {
        Self {
            providers: registry.providers_ref().clone(),
        }
    }

    /// Lists all models for a given provider, sorted by name.
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

    /// Searches all providers for a model by id, returning `(provider_id, model)`.
    pub async fn find_model(&self, model_id: &str) -> Option<(String, ModelEntry)> {
        let map = self.providers.read().await;
        for (pid, provider) in map.iter() {
            if let Some(model) = provider.models.get(model_id) {
                return Some((pid.clone(), model.clone()));
            }
        }
        None
    }

    /// Total number of models across all providers.
    pub async fn model_count(&self) -> usize {
        let map = self.providers.read().await;
        map.values().map(|p| p.models.len()).sum()
    }
}
