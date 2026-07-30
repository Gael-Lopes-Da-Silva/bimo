use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::config::{ApiFormat, Provider};
use crate::models::ModelEntry;

pub type ProviderMap = HashMap<String, ProviderEntry>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderEntry {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub doc: Option<String>,
    #[serde(default)]
    pub env: Vec<String>,
    #[serde(default)]
    pub npm: Option<String>,
    #[serde(default)]
    pub api: Option<serde_json::Value>,
    pub models: HashMap<String, ModelEntry>,
}

impl ProviderEntry {
    pub fn base_url(&self) -> Option<String> {
        self.api.as_ref().and_then(|v| match v {
            serde_json::Value::String(s) => Some(s.clone()),
            _ => None,
        })
    }

    pub fn api_format(&self) -> ApiFormat {
        match self.npm.as_deref() {
            Some("@ai-sdk/openai-compatible") => ApiFormat::OpenAICompatible,
            Some("@ai-sdk/anthropic") => ApiFormat::Anthropic,
            Some("@ai-sdk/openai") => ApiFormat::OpenAI,
            Some("@ai-sdk/google") | Some("@ai-sdk/google-vertex") => ApiFormat::Google,
            Some(other) => ApiFormat::Other(other.to_string()),
            None => ApiFormat::OpenAICompatible,
        }
    }

    pub fn to_provider(&self) -> Provider {
        let mut p = Provider::cloud(&self.id, &self.name, &self.base_url().unwrap_or_default());
        p.api_format = Some(self.api_format());
        p
    }
}
