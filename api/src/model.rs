use crate::error::Result;
use crate::provider::{self, ProviderRuntime, RawModel};
use serde::{Deserialize, Serialize};

/// A model that has been fetched and is available for selection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub provider_id: String,
}

/// Fetch available models for a provider and return them as [`ModelInfo`] entries.
pub async fn fetch_models_for_provider(
    runtime: &ProviderRuntime,
) -> Result<Vec<ModelInfo>> {
    let raw_models: Vec<RawModel> = provider::fetch_models(runtime).await?;

    let models = raw_models
        .into_iter()
        .map(|rm| ModelInfo {
            id: rm.id.clone(),
            name: rm.name.unwrap_or_else(|| rm.id),
            provider_id: runtime.id.clone(),
        })
        .collect();

    Ok(models)
}
