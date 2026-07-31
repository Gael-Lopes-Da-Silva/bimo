//! Provider resolution — built-in locals, cloud registries, model discovery.

mod cloud;
mod entry;
mod local;

use std::time::Duration;

pub use cloud::CloudProviderRegistry;
pub use entry::{ProviderEntry, ProviderMap};
pub use local::LocalProviderRegistry;

use crate::config::Provider;
use crate::error::BimoError;

/// Look up a provider by id or name — checks configured providers first,
/// then falls back to built-in local providers, then the registry.
pub async fn resolve_provider(
    id_or_name: &str,
    configured: &[Provider],
    registry: Option<&CloudProviderRegistry>,
) -> Option<Provider> {
    let lower = id_or_name.to_lowercase();
    let from_config = configured
        .iter()
        .find(|p| p.id.to_lowercase() == lower || p.name.to_lowercase() == lower)
        .cloned();
    if from_config.is_some() {
        return from_config;
    }
    let from_builtin = LocalProviderRegistry::new().find(id_or_name);
    if from_builtin.is_some() {
        return from_builtin;
    }
    if let Some(registry) = registry {
        return registry
            .find_provider(id_or_name)
            .await
            .filter(|e| !matches!(e.api_format(), crate::config::ApiFormat::Other(_)))
            .map(|e| e.to_provider());
    }
    None
}

/// Fetch available models from a provider's `/v1/models` endpoint.
pub async fn discover_models(provider: &Provider) -> crate::Result<Vec<String>> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|e| BimoError::Msg(format!("Failed to create HTTP client: {e}")))?;

    let base = provider.base_url.trim_end_matches('/');
    let url = format!("{base}/models");
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| BimoError::Msg(format!("Failed to fetch models: {e}")))?;
    let data: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| BimoError::Msg(format!("Failed to parse models response: {e}")))?;
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

/// Auto-discover models and update `provider.models` with the result.
pub async fn auto_discover_models(provider: &mut Provider) {
    match discover_models(provider).await {
        Ok(models) if !models.is_empty() => {
            provider.models = models;
        }
        _ => {}
    }
}

/// Resolve base URLs for cloud providers from the models.dev registry.
pub async fn resolve_from_registry(configured: &mut [Provider], registry: &CloudProviderRegistry) {
    for provider in configured.iter_mut() {
        if provider.is_cloud()
            && provider.base_url.is_empty()
            && let Some(url) = registry.provider_base_url(&provider.id).await
        {
            provider.base_url = url;
        }
    }
}
