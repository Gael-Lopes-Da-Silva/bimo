use crate::config::providers::{CustomProviderConfig, ProvidersConfig};
use crate::error::{BimoError, Result};
use crate::provider::types::{ProviderCategory, ProviderInfo, ProviderRuntime, RequestBodyFormat};

pub fn builtin_providers() -> Vec<ProviderInfo> {
    vec![
        ProviderInfo {
            id: "openai".into(),
            name: "OpenAI".into(),
            category: ProviderCategory::Cloud,
            requires_api_key: true,
            default_base_url: "https://api.openai.com/v1".into(),
            builtin: true,
        },
        ProviderInfo {
            id: "anthropic".into(),
            name: "Anthropic".into(),
            category: ProviderCategory::Cloud,
            requires_api_key: true,
            default_base_url: "https://api.anthropic.com".into(),
            builtin: true,
        },
        ProviderInfo {
            id: "openrouter".into(),
            name: "OpenRouter".into(),
            category: ProviderCategory::Cloud,
            requires_api_key: true,
            default_base_url: "https://openrouter.ai/api/v1".into(),
            builtin: true,
        },
        ProviderInfo {
            id: "opencode-zen".into(),
            name: "Opencode Zen".into(),
            category: ProviderCategory::Cloud,
            requires_api_key: true,
            default_base_url: "https://opencode.ai/zen/v1".into(),
            builtin: true,
        },
        ProviderInfo {
            id: "opencode-go".into(),
            name: "Opencode Go".into(),
            category: ProviderCategory::Cloud,
            requires_api_key: true,
            default_base_url: "https://opencode.ai/zen/go/v1".into(),
            builtin: true,
        },
        ProviderInfo {
            id: "lmstudio".into(),
            name: "LM Studio".into(),
            category: ProviderCategory::Local,
            requires_api_key: false,
            default_base_url: "http://localhost:1234/v1".into(),
            builtin: true,
        },
        ProviderInfo {
            id: "ollama".into(),
            name: "Ollama".into(),
            category: ProviderCategory::Local,
            requires_api_key: false,
            default_base_url: "http://localhost:11434/v1".into(),
            builtin: true,
        },
    ]
}

fn env_api_key(provider_id: &str) -> Option<String> {
    match provider_id {
        "openai" => std::env::var("OPENAI_API_KEY").ok(),
        "anthropic" => std::env::var("ANTHROPIC_API_KEY").ok(),
        "openrouter" => std::env::var("OPENROUTER_API_KEY").ok(),
        "opencode-go" => std::env::var("OPENCODE_API_KEY").ok(),
        "opencode-zen" => std::env::var("OPENCODE_API_KEY").ok(),
        "lmstudio" => std::env::var("LMSTUDIO_API_KEY").ok(),
        "ollama" => std::env::var("OLLAMA_API_KEY").ok(),
        _ => None,
    }
}

pub struct ProviderRegistry {
    builtins: Vec<ProviderInfo>,
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self {
            builtins: builtin_providers(),
        }
    }

    pub fn list_all(&self, config: &ProvidersConfig) -> Vec<ProviderInfo> {
        let mut out = self.builtins.clone();
        for cp in &config.custom {
            out.push(ProviderInfo {
                id: cp.id.clone(),
                name: cp.name.clone(),
                category: if cp.category == "local" {
                    ProviderCategory::Local
                } else {
                    ProviderCategory::Cloud
                },
                requires_api_key: cp.api_key_required,
                default_base_url: cp.base_url.clone(),
                builtin: false,
            });
        }
        out
    }

    pub fn resolve_runtime(
        &self,
        provider_id: &str,
        config: &ProvidersConfig,
    ) -> Result<ProviderRuntime> {
        if let Some(info) = self.builtins.iter().find(|p| p.id == provider_id) {
            return self.resolve_builtin(info, config);
        }
        if let Some(cp) = config.custom.iter().find(|p| p.id == provider_id) {
            return self.resolve_custom(cp, config);
        }
        Err(BimoError::Provider(format!(
            "unknown provider '{provider_id}'"
        )))
    }

    fn resolve_builtin(
        &self,
        info: &ProviderInfo,
        config: &ProvidersConfig,
    ) -> Result<ProviderRuntime> {
        let persisted = config.configured.get(&info.id);
        let base_url = persisted
            .map(|p| p.base_url.clone())
            .unwrap_or_else(|| info.default_base_url.clone());
        let api_key = persisted.and_then(|p| p.api_key.clone());

        if info.requires_api_key && api_key.is_none() && env_api_key(&info.id).is_none() {
            return Err(BimoError::Provider(format!(
                "provider '{}' requires an API key",
                info.id
            )));
        }

        let (chat_endpoint, models_endpoint, auth_header, auth_prefix, format, free_models) =
            match info.id.as_str() {
                "anthropic" => (
                    "/v1/messages".into(),
                    None,
                    Some("x-api-key".into()),
                    None,
                    RequestBodyFormat::Anthropic,
                    Vec::new(),
                ),
                "opencode-zen" => (
                    "/chat/completions".into(),
                    Some("/models".into()),
                    Some("Authorization".into()),
                    Some("Bearer ".into()),
                    RequestBodyFormat::OpenAi,
                    vec![
                        "deepseek-v4-flash-free".into(),
                        "mimo-v2.5-free".into(),
                        "laguna-s-2.1-free".into(),
                        "ling-3.0-flash-free".into(),
                        "north-mini-code-free".into(),
                        "nemotron-3-ultra-free".into(),
                        "big-pickle".into(),
                    ],
                ),
                _ => (
                    "/chat/completions".into(),
                    Some("/models".into()),
                    Some("Authorization".into()),
                    Some("Bearer ".into()),
                    RequestBodyFormat::OpenAi,
                    Vec::new(),
                ),
            };

        let api_key = api_key.or_else(|| env_api_key(&info.id));

        Ok(ProviderRuntime {
            id: info.id.clone(),
            base_url,
            api_key,
            chat_endpoint,
            models_endpoint,
            auth_header,
            auth_prefix,
            request_body_format: format,
            free_models,
        })
    }

    fn resolve_custom(
        &self,
        cp: &CustomProviderConfig,
        config: &ProvidersConfig,
    ) -> Result<ProviderRuntime> {
        let persisted = config.configured.get(&cp.id);
        let api_key = persisted.and_then(|p| p.api_key.clone());

        if cp.api_key_required && api_key.is_none() {
            return Err(BimoError::Provider(format!(
                "custom provider '{}' requires an API key",
                cp.id
            )));
        }

        Ok(ProviderRuntime {
            id: cp.id.clone(),
            base_url: cp.base_url.clone(),
            api_key,
            chat_endpoint: cp.chat_endpoint.clone(),
            models_endpoint: cp.models_endpoint.clone(),
            auth_header: cp.auth_header.clone(),
            auth_prefix: cp.auth_prefix.clone(),
            request_body_format: RequestBodyFormat::OpenAi,
            free_models: Vec::new(),
        })
    }
}
