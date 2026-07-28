use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProviderCategory {
    Local,
    Cloud,
}

impl std::fmt::Display for ProviderCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Local => write!(f, "local"),
            Self::Cloud => write!(f, "cloud"),
        }
    }
}

/// Metadata describing a provider (returned to clients).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderInfo {
    pub id: String,
    pub name: String,
    pub category: ProviderCategory,
    pub requires_api_key: bool,
    pub default_base_url: String,
    pub builtin: bool,
}

/// Runtime configuration needed to talk to a provider.
#[derive(Debug, Clone)]
pub struct ProviderRuntime {
    pub id: String,
    pub base_url: String,
    pub api_key: Option<String>,
    pub chat_endpoint: String,
    pub models_endpoint: Option<String>,
    pub auth_header: Option<String>,
    pub auth_prefix: Option<String>,
    pub request_body_format: RequestBodyFormat,
}

/// Describes how to format the chat-completion request body.
#[derive(Debug, Clone)]
pub enum RequestBodyFormat {
    OpenAi,
    Anthropic,
    Ollama,
}

/// A minimal model listing entry returned by `/models`-style endpoints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawModel {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    /// Tier label (e.g. "free", "paid") inferred from pricing metadata.
    #[serde(default)]
    pub tier: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug)]
pub struct ChatCompletionResponse {
    pub content: String,
    pub thinking: Option<String>,
    pub model: Option<String>,
    pub usage: Option<UsageInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageInfo {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_category_display() {
        assert_eq!(ProviderCategory::Local.to_string(), "local");
        assert_eq!(ProviderCategory::Cloud.to_string(), "cloud");
    }

    #[test]
    fn raw_model_is_serializable() {
        let model = RawModel {
            id: "gpt-4".into(),
            name: Some("GPT-4".into()),
            tier: None,
        };
        let json = serde_json::to_string(&model).unwrap();
        let deserialized: RawModel = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, "gpt-4");
    }

    #[test]
    fn usage_info_is_serializable() {
        let usage = UsageInfo {
            prompt_tokens: 10,
            completion_tokens: 20,
            total_tokens: 30,
        };
        let json = serde_json::to_string(&usage).unwrap();
        let deserialized: UsageInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.total_tokens, 30);
    }
}
