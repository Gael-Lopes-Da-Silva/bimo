use std::time::Duration;

use crate::config::{ApiFormat, Provider};
use crate::error::CustomError;

/// Registry of built-in local providers known to the system
/// (ollama, lmstudio, vllm, llamacpp).
#[derive(Debug, Clone, Copy, Default)]
pub struct LocalProviderRegistry;

impl LocalProviderRegistry {
    /// Creates a new registry.
    pub fn new() -> Self {
        Self
    }

    /// Returns the built-in local providers known to the system.
    pub fn builtin(&self) -> Vec<Provider> {
        vec![
            Provider::local(
                "ollama",
                "Ollama",
                "http://localhost:11434/v1",
                ApiFormat::OpenAICompatible,
            ),
            Provider::local(
                "lmstudio",
                "LM Studio",
                "http://localhost:1234/v1",
                ApiFormat::OpenAICompatible,
            ),
            Provider::local(
                "vllm",
                "vLLM",
                "http://localhost:8000/v1",
                ApiFormat::OpenAICompatible,
            ),
            Provider::local(
                "llamacpp",
                "llama.cpp",
                "http://localhost:8080/v1",
                ApiFormat::OpenAICompatible,
            ),
        ]
    }

    /// Looks up a built-in provider by id or name (case-insensitive).
    pub fn find(&self, id_or_name: &str) -> Option<Provider> {
        let lower = id_or_name.to_lowercase();
        self.builtin()
            .into_iter()
            .find(|p| p.id.to_lowercase() == lower || p.name.to_lowercase() == lower)
    }

    /// Fetches available models from a provider's `/v1/models` endpoint.
    pub async fn discover_models(&self, provider: &Provider) -> crate::Result<Vec<String>> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .map_err(|e| CustomError::Msg(format!("Failed to create HTTP client: {e}")))?;

        let base = provider.base_url.trim_end_matches('/');
        let url = format!("{base}/models");
        let resp = client
            .get(&url)
            .send()
            .await
            .map_err(|e| CustomError::Msg(format!("Failed to fetch models: {e}")))?;
        let data: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| CustomError::Msg(format!("Failed to parse models response: {e}")))?;
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

    /// Auto-discovers models and updates `provider.models` with the result.
    pub async fn auto_discover_models(&self, provider: &mut Provider) {
        match self.discover_models(provider).await {
            Ok(models) if !models.is_empty() => {
                provider.models = models;
            }
            _ => {}
        }
    }
}
