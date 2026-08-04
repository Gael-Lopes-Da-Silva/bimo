use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::config::{ApiFormat, Provider};
use crate::models::ModelEntry;

/// Map of provider id → [`ProviderEntry`] from the models.dev registry.
pub type ProviderMap = HashMap<String, ProviderEntry>;

/// A single provider entry from the models.dev API.
///
/// Each entry describes a known AI provider: its base URL, SDK package,
/// environment variables, and available models.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderEntry {
    /// Provider identifier (e.g. `"anthropic"`).
    pub id: String,
    /// Human-readable provider name.
    pub name: String,
    /// Link to the provider's documentation, if known.
    #[serde(default)]
    pub doc: Option<String>,
    /// Names of environment variables carrying the API key.
    #[serde(default)]
    pub env: Vec<String>,
    /// SDK package name (e.g. `"@ai-sdk/anthropic"`), used to derive the
    /// [`ApiFormat`].
    #[serde(default)]
    pub npm: Option<String>,
    /// Free-form API metadata from models.dev; `base_url` reads a string
    /// value out of this when present.
    #[serde(default)]
    pub api: Option<serde_json::Value>,
    /// Models served by this provider, keyed by model id.
    pub models: HashMap<String, ModelEntry>,
}

impl ProviderEntry {
    /// Returns the base URL from the `api` field, if it is a string.
    pub fn base_url(&self) -> Option<String> {
        self.api.as_ref().and_then(|v| match v {
            serde_json::Value::String(s) => Some(s.clone()),
            _ => None,
        })
    }

    /// Derives the [`ApiFormat`] from this entry's `npm` field.
    pub fn api_format(&self) -> ApiFormat {
        match self.npm.as_deref() {
            Some("@ai-sdk/openai-compatible") => ApiFormat::OpenAICompatible,
            Some("@ai-sdk/anthropic") => ApiFormat::Anthropic,
            Some("@ai-sdk/openai") => ApiFormat::OpenAI,
            Some("@ai-sdk/google") | Some("@ai-sdk/google-vertex") => ApiFormat::Google,
            Some(other) => ApiFormat::Other(other.to_string()),
            None => ApiFormat::Other("unknown".to_string()),
        }
    }

    /// Converts this registry entry into a [`Provider`] consumable by the
    /// agent builder. The returned provider has no API key set.
    pub fn to_provider(&self) -> Provider {
        Provider::cloud(
            &self.id,
            &self.name,
            &self.base_url().unwrap_or_default(),
            self.api_format(),
        )
    }
}
