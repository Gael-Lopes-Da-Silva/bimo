use crate::error::Result;
use crate::provider::{self, ProviderRuntime, RawModel};
use serde::{Deserialize, Serialize};

/// A model that has been fetched and is available for selection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub provider_id: String,
    /// Tier label (e.g. "free", "paid") for providers that distinguish model tiers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tier: Option<String>,
}

/// Fetch available models for a provider and return them as [`ModelInfo`] entries.
pub async fn fetch_models_for_provider(runtime: &ProviderRuntime) -> Result<Vec<ModelInfo>> {
    let raw_models: Vec<RawModel> = provider::fetch_models(runtime).await?;

    let models = raw_models
        .into_iter()
        .map(|rm| ModelInfo {
            id: rm.id.clone(),
            name: rm.name.unwrap_or(rm.id),
            provider_id: runtime.id.clone(),
            tier: rm.tier,
        })
        .collect();

    Ok(models)
}
