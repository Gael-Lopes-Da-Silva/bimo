use crate::config::providers::{CustomProviderConfig, ProvidersConfig};
use crate::error::{BimoError, Result};
use crate::provider::types::{ProviderCategory, ProviderInfo, ProviderRuntime, RequestBodyFormat};

fn local_providers() -> Vec<ProviderInfo> {
    vec![
        ProviderInfo {
            id: "lmstudio".into(),
            name: "LM Studio".into(),
            category: ProviderCategory::Local,
            requires_api_key: false,
            default_base_url: "http://localhost:1234/v1".into(),
            builtin: true,
            doc_url: None,
            env_vars: vec!["LMSTUDIO_API_KEY".into()],
        },
        ProviderInfo {
            id: "ollama".into(),
            name: "Ollama".into(),
            category: ProviderCategory::Local,
            requires_api_key: false,
            default_base_url: "http://localhost:11434/v1".into(),
            builtin: true,
            doc_url: None,
            env_vars: vec!["OLLAMA_API_KEY".into()],
        },
    ]
}

fn fallback_cloud_providers() -> Vec<ProviderInfo> {
    vec![
        ProviderInfo {
            id: "openai".into(),
            name: "OpenAI".into(),
            category: ProviderCategory::Cloud,
            requires_api_key: true,
            default_base_url: "https://api.openai.com/v1".into(),
            builtin: true,
            doc_url: None,
            env_vars: vec!["OPENAI_API_KEY".into()],
        },
        ProviderInfo {
            id: "anthropic".into(),
            name: "Anthropic".into(),
            category: ProviderCategory::Cloud,
            requires_api_key: true,
            default_base_url: "https://api.anthropic.com".into(),
            builtin: true,
            doc_url: None,
            env_vars: vec!["ANTHROPIC_API_KEY".into()],
        },
        ProviderInfo {
            id: "openrouter".into(),
            name: "OpenRouter".into(),
            category: ProviderCategory::Cloud,
            requires_api_key: true,
            default_base_url: "https://openrouter.ai/api/v1".into(),
            builtin: true,
            doc_url: None,
            env_vars: vec!["OPENROUTER_API_KEY".into()],
        },
        ProviderInfo {
            id: "opencode-zen".into(),
            name: "Opencode Zen".into(),
            category: ProviderCategory::Cloud,
            requires_api_key: true,
            default_base_url: "https://opencode.ai/zen/v1".into(),
            builtin: true,
            doc_url: None,
            env_vars: vec!["OPENCODE_API_KEY".into()],
        },
        ProviderInfo {
            id: "opencode-go".into(),
            name: "Opencode Go".into(),
            category: ProviderCategory::Cloud,
            requires_api_key: true,
            default_base_url: "https://opencode.ai/zen/go/v1".into(),
            builtin: true,
            doc_url: None,
            env_vars: vec!["OPENCODE_API_KEY".into()],
        },
    ]
}

fn parse_models_dev(raw: &serde_json::Value) -> Vec<ProviderInfo> {
    let Some(obj) = raw.as_object() else {
        return Vec::new();
    };
    let mut providers = Vec::new();
    for (id, val) in obj {
        let name = val
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or(id)
            .to_string();
        let api = val
            .get("api")
            .and_then(|a| a.as_str())
            .unwrap_or("")
            .to_string();
        let doc = val.get("doc").and_then(|d| d.as_str()).map(String::from);
        let env_vars = val
            .get("env")
            .and_then(|e| e.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        providers.push(ProviderInfo {
            id: id.clone(),
            name,
            category: ProviderCategory::Cloud,
            requires_api_key: true,
            default_base_url: api,
            builtin: true,
            doc_url: doc,
            env_vars,
        });
    }
    providers
}

fn check_env_vars(vars: &[String]) -> Option<String> {
    for v in vars {
        if let Ok(val) = std::env::var(v) {
            return Some(val);
        }
    }
    None
}

pub struct ProviderRegistry {
    local: Vec<ProviderInfo>,
    cloud: Vec<ProviderInfo>,
    pub models_dev: Option<serde_json::Value>,
}

impl ProviderRegistry {
    pub async fn new() -> Self {
        let local = local_providers();

        let (cloud, models_dev) = match Self::fetch_models_dev().await {
            Some(raw) => (parse_models_dev(&raw), Some(raw)),
            None => (fallback_cloud_providers(), None),
        };

        Self {
            local,
            cloud,
            models_dev,
        }
    }

    async fn fetch_models_dev() -> Option<serde_json::Value> {
        let url = "https://models.dev/api.json";
        let Ok(resp) = reqwest::get(url).await else {
            return None;
        };
        let Ok(raw) = resp.json::<serde_json::Value>().await else {
            return None;
        };
        Some(raw)
    }

    pub fn list_all(&self, config: &ProvidersConfig) -> Vec<ProviderInfo> {
        let mut out = Vec::new();
        out.extend(self.local.clone());
        out.extend(self.cloud.clone());
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
                doc_url: None,
                env_vars: Vec::new(),
            });
        }
        out
    }

    pub fn supports_models_dev(&self, provider_id: &str) -> bool {
        self.models_dev
            .as_ref()
            .and_then(|m| m.as_object())
            .is_some_and(|o| o.contains_key(provider_id))
    }

    pub fn resolve_runtime(
        &self,
        provider_id: &str,
        config: &ProvidersConfig,
    ) -> Result<ProviderRuntime> {
        // Check local providers
        if let Some(info) = self.local.iter().find(|p| p.id == provider_id) {
            return self.resolve_local(info, config);
        }
        // Check cloud providers (from models.dev or fallback)
        if let Some(info) = self.cloud.iter().find(|p| p.id == provider_id) {
            return self.resolve_cloud(info, config);
        }
        // Check custom providers
        if let Some(cp) = config.custom.iter().find(|p| p.id == provider_id) {
            return self.resolve_custom(cp, config);
        }
        Err(BimoError::Provider(format!(
            "unknown provider '{provider_id}'"
        )))
    }

    fn resolve_local(
        &self,
        info: &ProviderInfo,
        config: &ProvidersConfig,
    ) -> Result<ProviderRuntime> {
        let persisted = config.configured.get(&info.id);
        let base_url = persisted
            .map(|p| p.base_url.clone())
            .unwrap_or_else(|| info.default_base_url.clone());
        let api_key = persisted.and_then(|p| p.api_key.clone());

        let api_key = api_key.or_else(|| check_env_vars(&info.env_vars));

        Ok(ProviderRuntime {
            id: info.id.clone(),
            base_url,
            api_key,
            chat_endpoint: "/chat/completions".into(),
            models_endpoint: Some("/models".into()),
            auth_header: Some("Authorization".into()),
            auth_prefix: Some("Bearer ".into()),
            request_body_format: RequestBodyFormat::OpenAi,
        })
    }

    fn resolve_cloud(
        &self,
        info: &ProviderInfo,
        config: &ProvidersConfig,
    ) -> Result<ProviderRuntime> {
        let persisted = config.configured.get(&info.id);
        let base_url = persisted
            .map(|p| p.base_url.clone())
            .unwrap_or_else(|| info.default_base_url.clone());
        let api_key = persisted.and_then(|p| p.api_key.clone());

        // Check persisted key first, then env vars from the provider's definition
        let api_key = api_key.or_else(|| check_env_vars(&info.env_vars));

        if info.requires_api_key && api_key.is_none() {
            return Err(BimoError::Provider(format!(
                "provider '{}' requires an API key",
                info.id
            )));
        }

        if info.id == "anthropic" {
            return Ok(ProviderRuntime {
                id: info.id.clone(),
                base_url,
                api_key,
                chat_endpoint: "/v1/messages".into(),
                models_endpoint: None,
                auth_header: Some("x-api-key".into()),
                auth_prefix: None,
                request_body_format: RequestBodyFormat::Anthropic,
            });
        }

        Ok(ProviderRuntime {
            id: info.id.clone(),
            base_url,
            api_key,
            chat_endpoint: "/chat/completions".into(),
            models_endpoint: Some("/models".into()),
            auth_header: Some("Authorization".into()),
            auth_prefix: Some("Bearer ".into()),
            request_body_format: RequestBodyFormat::OpenAi,
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
        })
    }
}
