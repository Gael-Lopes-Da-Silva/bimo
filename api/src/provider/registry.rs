use crate::config::AppConfig;
use crate::error::{BimoError, Result};

use super::types::{ProviderCategory, ProviderInfo, ProviderRuntime, RequestBodyFormat};
use crate::config::CustomProviderConfig;

// ---------------------------------------------------------------------------
// Built-in provider catalogue
// ---------------------------------------------------------------------------

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
            id: "ollama".into(),
            name: "Ollama".into(),
            category: ProviderCategory::Local,
            requires_api_key: false,
            default_base_url: "http://localhost:11434".into(),
            builtin: true,
        },
    ]
}

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

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

    /// List all available providers (built-in + custom).
    pub fn list_all(&self, config: &AppConfig) -> Vec<ProviderInfo> {
        let mut out = self.builtins.clone();
        for cp in &config.custom_providers {
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

    /// Resolve a provider id into its runtime configuration.
    pub fn resolve_runtime(
        &self,
        provider_id: &str,
        config: &AppConfig,
    ) -> Result<ProviderRuntime> {
        if let Some(info) = self.builtins.iter().find(|p| p.id == provider_id) {
            return self.resolve_builtin(info, config);
        }
        if let Some(cp) = config.custom_providers.iter().find(|p| p.id == provider_id) {
            return self.resolve_custom(cp, config);
        }
        Err(BimoError::Provider(format!(
            "unknown provider '{provider_id}'"
        )))
    }

    fn resolve_builtin(&self, info: &ProviderInfo, config: &AppConfig) -> Result<ProviderRuntime> {
        let persisted = config.provider_configs.get(&info.id);
        let base_url = persisted
            .map(|p| p.base_url.clone())
            .unwrap_or_else(|| info.default_base_url.clone());
        let api_key = persisted.and_then(|p| p.api_key.clone());

        if info.requires_api_key && api_key.is_none() {
            let env_key = match info.id.as_str() {
                "openai" => std::env::var("OPENAI_API_KEY").ok(),
                "anthropic" => std::env::var("ANTHROPIC_API_KEY").ok(),
                _ => None,
            };
            if env_key.is_none() {
                return Err(BimoError::Provider(format!(
                    "provider '{}' requires an API key. \
                     Set it via /provider configure or the environment variable.",
                    info.id
                )));
            }
        }

        let (chat_endpoint, models_endpoint, auth_header, auth_prefix, format) =
            match info.id.as_str() {
                "openai" => (
                    "/chat/completions".into(),
                    Some("/models".into()),
                    Some("Authorization".into()),
                    Some("Bearer ".into()),
                    RequestBodyFormat::OpenAi,
                ),
                "anthropic" => (
                    "/v1/messages".into(),
                    None,
                    Some("x-api-key".into()),
                    None,
                    RequestBodyFormat::Anthropic,
                ),
                "ollama" => (
                    "/api/chat".into(),
                    Some("/api/tags".into()),
                    None,
                    None,
                    RequestBodyFormat::Ollama,
                ),
                _ => return Err(BimoError::Provider("unsupported builtin".into())),
            };

        let api_key = api_key.or_else(|| match info.id.as_str() {
            "openai" => std::env::var("OPENAI_API_KEY").ok(),
            "anthropic" => std::env::var("ANTHROPIC_API_KEY").ok(),
            _ => None,
        });

        Ok(ProviderRuntime {
            id: info.id.clone(),
            base_url,
            api_key,
            chat_endpoint,
            models_endpoint,
            auth_header,
            auth_prefix,
            request_body_format: format,
        })
    }

    fn resolve_custom(
        &self,
        cp: &CustomProviderConfig,
        config: &AppConfig,
    ) -> Result<ProviderRuntime> {
        let persisted = config.provider_configs.get(&cp.id);
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
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_providers_count() {
        let providers = builtin_providers();
        assert_eq!(providers.len(), 3);
    }

    #[test]
    fn builtin_provider_ids() {
        let providers = builtin_providers();
        let ids: Vec<&str> = providers.iter().map(|p| p.id.as_str()).collect();
        assert!(ids.contains(&"openai"));
        assert!(ids.contains(&"anthropic"));
        assert!(ids.contains(&"ollama"));
    }

    #[test]
    fn openai_provider_metadata() {
        let providers = builtin_providers();
        let openai = providers.iter().find(|p| p.id == "openai").unwrap();
        assert_eq!(openai.category, ProviderCategory::Cloud);
        assert!(openai.requires_api_key);
        assert!(openai.builtin);
        assert_eq!(openai.default_base_url, "https://api.openai.com/v1");
    }

    #[test]
    fn ollama_provider_no_api_key() {
        let providers = builtin_providers();
        let ollama = providers.iter().find(|p| p.id == "ollama").unwrap();
        assert_eq!(ollama.category, ProviderCategory::Local);
        assert!(!ollama.requires_api_key);
    }

    #[test]
    fn anthropic_requires_api_key() {
        let providers = builtin_providers();
        let anthropic = providers.iter().find(|p| p.id == "anthropic").unwrap();
        assert!(anthropic.requires_api_key);
    }

    #[test]
    fn registry_list_includes_custom() {
        let reg = ProviderRegistry::new();
        let mut config = AppConfig::default();
        config.custom_providers.push(CustomProviderConfig {
            id: "custom-1".into(),
            name: "Custom One".into(),
            category: "cloud".into(),
            base_url: "https://custom.api".into(),
            api_key_required: false,
            chat_endpoint: "/chat".into(),
            models_endpoint: None,
            auth_header: None,
            auth_prefix: None,
        });

        let all = reg.list_all(&config);
        assert_eq!(all.len(), 4);
        assert!(all.iter().any(|p| p.id == "custom-1" && !p.builtin));
    }

    #[test]
    fn resolve_runtime_unknown_provider() {
        let reg = ProviderRegistry::new();
        let config = AppConfig::default();
        let result = reg.resolve_runtime("nonexistent", &config);
        assert!(result.is_err());
    }
}
