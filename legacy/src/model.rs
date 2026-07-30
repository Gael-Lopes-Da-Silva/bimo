use std::collections::HashMap;

use crate::error::Result;
use crate::provider::{self, ProviderRuntime, RawModel};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub provider_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_window: Option<u64>,
    #[serde(default)]
    pub free: bool,
}

pub async fn fetch_models_for_provider(
    runtime: &ProviderRuntime,
    models_dev: Option<&serde_json::Value>,
) -> Result<Vec<ModelInfo>> {
    // If models.dev data is available and contains this provider, extract models from it
    if let Some(raw) = models_dev
        && let Some(models) = extract_models_from_dev(raw, &runtime.id)
    {
        return Ok(models);
    }

    // Fallback: fetch models from the provider's API
    let raw_models: Vec<RawModel> = provider::fetch_models(runtime).await?;
    let ctx_map = get_models_dev_contexts().await;
    let models = raw_models
        .into_iter()
        .map(|rm| {
            let id = rm.id.clone();
            let name = rm.name.unwrap_or_else(|| id.clone());
            let key = format!("{}:{}", runtime.id, id);
            let context_window = ctx_map.get(&key).copied();
            ModelInfo {
                id,
                name,
                provider_id: runtime.id.clone(),
                context_window,
                free: false,
            }
        })
        .collect();
    Ok(models)
}

/// Extract models from cached models.dev data for a specific provider.
/// Only returns models that support tool calls.
fn extract_models_from_dev(raw: &serde_json::Value, provider_id: &str) -> Option<Vec<ModelInfo>> {
    let provider = raw.get(provider_id)?;
    let models_obj = provider.get("models")?.as_object()?;
    let mut models = Vec::new();
    for (model_id, info) in models_obj {
        let tool_call = info
            .get("tool_call")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if !tool_call {
            continue;
        }
        let name = info
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or(model_id)
            .to_string();
        let context_window = info
            .get("limit")
            .and_then(|l| l.get("context"))
            .and_then(|c| c.as_u64());
        let cost_input = info
            .get("cost")
            .and_then(|c| c.get("input"))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let cost_output = info
            .get("cost")
            .and_then(|c| c.get("output"))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let free = cost_input == 0.0 && cost_output == 0.0;
        models.push(ModelInfo {
            id: model_id.clone(),
            name,
            provider_id: provider_id.to_string(),
            context_window,
            free,
        });
    }
    Some(models)
}

/// Fallback context window lookup from cached models.dev data.
/// Used when models are fetched from the provider's API rather than models.dev.
pub async fn get_models_dev_contexts() -> &'static HashMap<String, u64> {
    static CACHE: tokio::sync::OnceCell<HashMap<String, u64>> = tokio::sync::OnceCell::const_new();

    CACHE
        .get_or_init(|| async {
            let mut map = HashMap::new();
            let url = "https://models.dev/api.json";
            let Ok(resp) = reqwest::get(url).await else {
                return map;
            };
            let Ok(raw) = resp.json::<serde_json::Value>().await else {
                return map;
            };
            let Some(obj) = raw.as_object() else {
                return map;
            };
            for (provider_id, provider_val) in obj {
                let Some(models) = provider_val.get("models").and_then(|m| m.as_object()) else {
                    continue;
                };
                for (model_id, model_info) in models {
                    if let Some(ctx) = model_info
                        .get("limit")
                        .and_then(|l| l.get("context"))
                        .and_then(|c| c.as_u64())
                    {
                        map.insert(format!("{}:{}", provider_id, model_id), ctx);
                    }
                }
            }
            map
        })
        .await
}
