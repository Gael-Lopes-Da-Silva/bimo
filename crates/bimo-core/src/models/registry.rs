//! Model registry — queries models from the shared provider map.

use std::sync::Arc;

use tokio::sync::RwLock;

use crate::ModelEntry;
use crate::error::Result;
use crate::providers::{CloudProviderRegistry, ProviderMap};

/// Queries model metadata from a shared [`ProviderMap`].
///
/// Created via [`from_registry`](Self::from_registry) so it shares the same
/// in-memory data as the [`CloudProviderRegistry`] without duplicating it.
#[derive(Debug, Clone)]
pub struct ModelRegistry {
    providers: Arc<RwLock<ProviderMap>>,
    registry: CloudProviderRegistry,
}

impl ModelRegistry {
    /// Creates a `ModelRegistry` that shares the given registry's provider data.
    pub fn from_registry(registry: &CloudProviderRegistry) -> Self {
        Self {
            providers: registry.providers_ref().clone(),
            registry: registry.clone(),
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

    /// Checks if a model supports given input/output modalities (e.g. text, image).
    pub async fn model_capabilities(&self, model_id: &str) -> Option<(Vec<String>, Vec<String>)> {
        let (_pid, model) = self.find_model(model_id).await?;
        model
            .modalities
            .as_ref()
            .map(|m| (m.input.clone(), m.output.clone()))
    }

    /// Returns true if the model supports text input and output.
    pub async fn supports_text(&self, model_id: &str) -> bool {
        if let Some((inputs, outputs)) = self.model_capabilities(model_id).await {
            inputs.contains(&"text".to_string()) && outputs.contains(&"text".to_string())
        } else {
            false
        }
    }

    /// Returns true if the model supports image input.
    pub async fn supports_image_input(&self, model_id: &str) -> bool {
        if let Some((inputs, _)) = self.model_capabilities(model_id).await {
            inputs.contains(&"image".to_string())
        } else {
            false
        }
    }

    /// Total number of models across all providers.
    pub async fn model_count(&self) -> usize {
        let map = self.providers.read().await;
        map.values().map(|p| p.models.len()).sum()
    }

    /// Refreshes the models list of the given provider from a fresh fetch to
    /// models.dev, updating the shared provider data and local cache.
    pub async fn refresh(&self, provider_id: &str) -> Result<()> {
        self.registry.refresh_provider(provider_id).await
    }
}
