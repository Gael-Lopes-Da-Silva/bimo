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
}

const MODELS_DEV_URL: &str = "https://models.dev/api.json";

static MODELS_DEV_CACHE: tokio::sync::OnceCell<HashMap<String, u64>> =
    tokio::sync::OnceCell::const_new();

pub async fn get_models_dev_contexts() -> &'static HashMap<String, u64> {
    MODELS_DEV_CACHE
        .get_or_init(|| async {
            let mut map = HashMap::new();

            let Ok(resp) = reqwest::get(MODELS_DEV_URL).await else {
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

pub async fn fetch_models_for_provider(runtime: &ProviderRuntime) -> Result<Vec<ModelInfo>> {
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
            }
        })
        .collect();
    Ok(models)
}
