use std::sync::Arc;

use tokio::sync::RwLock;

use super::types::ModelEntry;
use crate::providers::{ProviderMap, ProviderRegistry};

#[derive(Debug, Clone)]
pub struct ModelRegistry {
    providers: Arc<RwLock<ProviderMap>>,
}

impl ModelRegistry {
    pub fn from_registry(registry: &ProviderRegistry) -> Self {
        Self {
            providers: registry.providers_ref().clone(),
        }
    }

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

    pub async fn find_model(&self, model_id: &str) -> Option<(String, ModelEntry)> {
        let map = self.providers.read().await;
        for (pid, provider) in map.iter() {
            if let Some(model) = provider.models.get(model_id) {
                return Some((pid.clone(), model.clone()));
            }
        }
        None
    }

    pub async fn model_count(&self) -> usize {
        let map = self.providers.read().await;
        map.values().map(|p| p.models.len()).sum()
    }
}
