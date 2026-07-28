use crate::config::{AppConfig, CustomProviderConfig, ThinkingConfig};
use crate::error::{BimoError, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

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
    /// OpenAI-compatible format.
    OpenAi,
    /// Anthropic Messages API format.
    Anthropic,
    /// Ollama format.
    Ollama,
}

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
        // Check builtins first
        if let Some(info) = self.builtins.iter().find(|p| p.id == provider_id) {
            return self.resolve_builtin(info, config);
        }
        // Then custom providers
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
            // Check environment variable
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
                    None, // Anthropic doesn't have a public models endpoint
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
            request_body_format: RequestBodyFormat::OpenAi, // custom defaults to OpenAI-compatible
        })
    }
}

// ---------------------------------------------------------------------------
// HTTP helpers — fetching models and sending chat completions
// ---------------------------------------------------------------------------

/// A minimal model listing entry returned by `/models`-style endpoints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawModel {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
}

/// Fetch available models from the given provider.
pub async fn fetch_models(runtime: &ProviderRuntime) -> Result<Vec<RawModel>> {
    let endpoint = match &runtime.models_endpoint {
        Some(ep) => ep,
        None => {
            tracing::debug!(provider = %runtime.id, "no models endpoint, skipping fetch");
            return Ok(Vec::new());
        }
    };

    let client = Client::new();
    let url = format!("{}{}", runtime.base_url.trim_end_matches('/'), endpoint);
    tracing::debug!(provider = %runtime.id, url = %url, "fetching models");
    let mut req = client.get(&url);

    req = apply_auth(req, runtime)?;

    let resp = req.send().await.map_err(|e| {
        tracing::error!(provider = %runtime.id, error = %e, "model fetch request failed");
        BimoError::Network(format!("failed to fetch models from {}: {e}", runtime.id))
    })?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        tracing::error!(provider = %runtime.id, status = %status, "model fetch failed");
        return Err(BimoError::Network(format!(
            "model fetch failed ({}): {}",
            status, body
        )));
    }

    tracing::debug!(provider = %runtime.id, "model fetch response received, parsing");
    let raw: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| BimoError::Serialization(format!("failed to parse model list: {e}")))?;

    let models = parse_models_response(runtime, &raw)?;
    tracing::info!(provider = %runtime.id, count = models.len(), "models fetched");
    Ok(models)
}

fn parse_models_response(
    runtime: &ProviderRuntime,
    raw: &serde_json::Value,
) -> Result<Vec<RawModel>> {
    match runtime.request_body_format {
        RequestBodyFormat::OpenAi | RequestBodyFormat::Anthropic => {
            // OpenAI format: { "data": [ { "id": "gpt-4", ... } ] }
            let arr = raw
                .get("data")
                .and_then(|d| d.as_array())
                .cloned()
                .or_else(|| raw.as_array().cloned())
                .unwrap_or_default();
            let models = arr
                .into_iter()
                .filter_map(|v| {
                    let id = v.get("id")?.as_str()?.to_string();
                    let name = v.get("name").and_then(|n| n.as_str()).map(String::from);
                    Some(RawModel { id, name })
                })
                .collect();
            Ok(models)
        }
        RequestBodyFormat::Ollama => {
            // Ollama: { "models": [ { "name": "llama3", ... } ] }
            let arr = raw
                .get("models")
                .and_then(|d| d.as_array())
                .cloned()
                .unwrap_or_default();
            let models = arr
                .into_iter()
                .filter_map(|v| {
                    let id = v.get("name")?.as_str()?.to_string();
                    Some(RawModel { id, name: None })
                })
                .collect();
            Ok(models)
        }
    }
}

fn apply_auth(
    mut req: reqwest::RequestBuilder,
    runtime: &ProviderRuntime,
) -> Result<reqwest::RequestBuilder> {
    if let (Some(header), Some(prefix)) = (&runtime.auth_header, &runtime.auth_prefix) {
        if let Some(key) = &runtime.api_key {
            req = req.header(header.as_str(), format!("{prefix}{key}"));
        }
    } else if let Some(header) = &runtime.auth_header
        && let Some(key) = &runtime.api_key
    {
        req = req.header(header.as_str(), key.as_str());
    }
    Ok(req)
}

// ---------------------------------------------------------------------------
// Chat completion request / response types
// ---------------------------------------------------------------------------

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

/// Send a chat completion request to the provider.
pub async fn chat_completion(
    runtime: &ProviderRuntime,
    messages: &[ChatMessage],
    model: &str,
    thinking: &ThinkingConfig,
) -> Result<ChatCompletionResponse> {
    let client = Client::new();
    let url = format!(
        "{}{}",
        runtime.base_url.trim_end_matches('/'),
        runtime.chat_endpoint
    );

    tracing::debug!(provider = %runtime.id, model, url = %url, message_count = messages.len(), thinking_enabled = thinking.enabled, "sending chat completion request");
    let body = build_request_body(runtime, messages, model, thinking)?;

    let mut req = client.post(&url).json(&body);
    req = apply_auth(req, runtime)?;

    let resp = req.send().await.map_err(|e| {
        tracing::error!(provider = %runtime.id, error = %e, "chat completion request failed");
        BimoError::Network(format!("chat request to {} failed: {e}", runtime.id))
    })?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        tracing::error!(provider = %runtime.id, status = %status, body = %body, "chat completion failed");
        return Err(BimoError::Provider(format!(
            "chat completion failed ({}): {}",
            status, body
        )));
    }

    tracing::debug!(provider = %runtime.id, "chat completion response received, parsing");
    let raw: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| BimoError::Serialization(format!("failed to parse chat response: {e}")))?;

    let result = parse_chat_response(runtime, &raw)?;
    tracing::info!(
        provider = %runtime.id,
        model = ?result.model,
        content_len = result.content.len(),
        "chat completion done"
    );
    Ok(result)
}

fn build_request_body(
    runtime: &ProviderRuntime,
    messages: &[ChatMessage],
    model: &str,
    thinking: &ThinkingConfig,
) -> Result<serde_json::Value> {
    match runtime.request_body_format {
        RequestBodyFormat::OpenAi => {
            let msgs: Vec<serde_json::Value> = messages
                .iter()
                .map(|m| {
                    serde_json::json!({
                        "role": m.role,
                        "content": m.content,
                    })
                })
                .collect();
            let mut body = serde_json::json!({
                "model": model,
                "messages": msgs,
            });
            // OpenAI o-series: reasoning_effort parameter
            if thinking.enabled
                && let Some(ref effort) = thinking.reasoning_effort
            {
                body.as_object_mut()
                    .unwrap()
                    .insert("reasoning_effort".into(), serde_json::json!(effort));
            }
            Ok(body)
        }
        RequestBodyFormat::Anthropic => {
            let msgs: Vec<serde_json::Value> = messages
                .iter()
                .map(|m| {
                    serde_json::json!({
                        "role": m.role,
                        "content": m.content,
                    })
                })
                .collect();
            let mut body = serde_json::json!({
                "model": model,
                "messages": msgs,
                "max_tokens": 8192,
            });
            // Anthropic thinking parameter
            if thinking.enabled {
                let budget = thinking.budget_tokens.unwrap_or(10000);
                body.as_object_mut().unwrap().insert(
                    "thinking".into(),
                    serde_json::json!({
                        "type": "enabled",
                        "budget_tokens": budget,
                    }),
                );
            }
            Ok(body)
        }
        RequestBodyFormat::Ollama => {
            let msgs: Vec<serde_json::Value> = messages
                .iter()
                .map(|m| {
                    serde_json::json!({
                        "role": m.role,
                        "content": m.content,
                    })
                })
                .collect();
            let mut body = serde_json::json!({
                "model": model,
                "messages": msgs,
                "stream": false,
            });
            // Ollama think parameter
            if thinking.enabled
                && let Some(obj) = body.as_object_mut()
            {
                obj.insert("think".into(), serde_json::json!(true));
            }
            Ok(body)
        }
    }
}

fn parse_chat_response(
    _runtime: &ProviderRuntime,
    raw: &serde_json::Value,
) -> Result<ChatCompletionResponse> {
    // OpenAI-compatible response: { "choices": [{ "message": { "content": "..." } }], ... }
    // Anthropic response: { "content": [{ "type": "text", "text": "..." }, { "type": "thinking", "thinking": "..." }], ... }
    // Ollama response: { "message": { "content": "..." } }

    // Try OpenAI format first
    if let Some(content) = raw
        .get("choices")
        .and_then(|c| c.as_array())
        .and_then(|a| a.first())
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
    {
        let model = raw.get("model").and_then(|m| m.as_str()).map(String::from);
        let usage = raw.get("usage").and_then(|u| {
            Some(UsageInfo {
                prompt_tokens: u.get("prompt_tokens")?.as_u64()? as u32,
                completion_tokens: u.get("completion_tokens")?.as_u64()? as u32,
                total_tokens: u.get("total_tokens")?.as_u64()? as u32,
            })
        });
        return Ok(ChatCompletionResponse {
            content: content.to_string(),
            thinking: None,
            model,
            usage,
        });
    }

    // Try Anthropic format — content is an array that may include thinking blocks
    if let Some(content_array) = raw.get("content").and_then(|c| c.as_array()) {
        let mut text_parts = Vec::new();
        let mut thinking_parts = Vec::new();

        for block in content_array {
            match block.get("type").and_then(|t| t.as_str()) {
                Some("thinking") => {
                    if let Some(thinking) = block.get("thinking").and_then(|t| t.as_str()) {
                        thinking_parts.push(thinking);
                    }
                }
                Some("text") => {
                    if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                        text_parts.push(text);
                    }
                }
                _ => {}
            }
        }

        let model = raw.get("model").and_then(|m| m.as_str()).map(String::from);
        let usage = raw.get("usage").and_then(|u| {
            let input = u.get("input_tokens")?.as_u64()? as u32;
            let output = u.get("output_tokens")?.as_u64()? as u32;
            Some(UsageInfo {
                prompt_tokens: input,
                completion_tokens: output,
                total_tokens: input + output,
            })
        });
        return Ok(ChatCompletionResponse {
            content: text_parts.join(""),
            thinking: if thinking_parts.is_empty() {
                None
            } else {
                Some(thinking_parts.join("\n"))
            },
            model,
            usage,
        });
    }

    // Try Ollama format
    if let Some(content) = raw
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
    {
        let model = raw.get("model").and_then(|m| m.as_str()).map(String::from);
        return Ok(ChatCompletionResponse {
            content: content.to_string(),
            thinking: None,
            model,
            usage: None,
        });
    }

    Err(BimoError::Serialization(format!(
        "unable to parse chat response: {}",
        serde_json::to_string(raw).unwrap_or_default()
    )))
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
        assert_eq!(all.len(), 4); // 3 builtins + 1 custom
        assert!(all.iter().any(|p| p.id == "custom-1" && !p.builtin));
    }

    #[test]
    fn resolve_runtime_unknown_provider() {
        let reg = ProviderRegistry::new();
        let config = AppConfig::default();
        let result = reg.resolve_runtime("nonexistent", &config);
        assert!(result.is_err());
    }

    #[test]
    fn parse_openai_models_response() {
        let runtime = ProviderRuntime {
            id: "openai".into(),
            base_url: "https://api.openai.com/v1".into(),
            api_key: None,
            chat_endpoint: "/chat/completions".into(),
            models_endpoint: Some("/models".into()),
            auth_header: Some("Authorization".into()),
            auth_prefix: Some("Bearer ".into()),
            request_body_format: RequestBodyFormat::OpenAi,
        };

        let raw = serde_json::json!({
            "data": [
                { "id": "gpt-4", "name": "GPT-4" },
                { "id": "gpt-3.5-turbo", "name": "GPT-3.5 Turbo" }
            ]
        });

        let models = parse_models_response(&runtime, &raw).unwrap();
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].id, "gpt-4");
        assert_eq!(models[0].name.as_deref(), Some("GPT-4"));
    }

    #[test]
    fn parse_ollama_models_response() {
        let runtime = ProviderRuntime {
            id: "ollama".into(),
            base_url: "http://localhost:11434".into(),
            api_key: None,
            chat_endpoint: "/api/chat".into(),
            models_endpoint: Some("/api/tags".into()),
            auth_header: None,
            auth_prefix: None,
            request_body_format: RequestBodyFormat::Ollama,
        };

        let raw = serde_json::json!({
            "models": [
                { "name": "llama3" },
                { "name": "codellama" }
            ]
        });

        let models = parse_models_response(&runtime, &raw).unwrap();
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].id, "llama3");
    }

    #[test]
    fn parse_openai_chat_response() {
        let runtime = ProviderRuntime {
            id: "openai".into(),
            base_url: "".into(),
            api_key: None,
            chat_endpoint: "".into(),
            models_endpoint: None,
            auth_header: None,
            auth_prefix: None,
            request_body_format: RequestBodyFormat::OpenAi,
        };

        let raw = serde_json::json!({
            "choices": [{
                "message": { "content": "Hello!" }
            }],
            "model": "gpt-4",
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 5,
                "total_tokens": 15
            }
        });

        let resp = parse_chat_response(&runtime, &raw).unwrap();
        assert_eq!(resp.content, "Hello!");
        assert_eq!(resp.model.as_deref(), Some("gpt-4"));
        assert!(resp.usage.is_some());
        assert_eq!(resp.usage.unwrap().total_tokens, 15);
    }

    #[test]
    fn parse_anthropic_chat_response() {
        let runtime = ProviderRuntime {
            id: "anthropic".into(),
            base_url: "".into(),
            api_key: None,
            chat_endpoint: "".into(),
            models_endpoint: None,
            auth_header: None,
            auth_prefix: None,
            request_body_format: RequestBodyFormat::Anthropic,
        };

        let raw = serde_json::json!({
            "content": [{ "type": "text", "text": "Hi there!" }],
            "model": "claude-3",
            "usage": {
                "input_tokens": 8,
                "output_tokens": 3
            }
        });

        let resp = parse_chat_response(&runtime, &raw).unwrap();
        assert_eq!(resp.content, "Hi there!");
        assert_eq!(resp.model.as_deref(), Some("claude-3"));
        let usage = resp.usage.unwrap();
        assert_eq!(usage.prompt_tokens, 8);
        assert_eq!(usage.completion_tokens, 3);
        assert_eq!(usage.total_tokens, 11);
    }

    #[test]
    fn parse_ollama_chat_response() {
        let runtime = ProviderRuntime {
            id: "ollama".into(),
            base_url: "".into(),
            api_key: None,
            chat_endpoint: "".into(),
            models_endpoint: None,
            auth_header: None,
            auth_prefix: None,
            request_body_format: RequestBodyFormat::Ollama,
        };

        let raw = serde_json::json!({
            "message": { "content": "Ollama says hi" },
            "model": "llama3"
        });

        let resp = parse_chat_response(&runtime, &raw).unwrap();
        assert_eq!(resp.content, "Ollama says hi");
        assert_eq!(resp.model.as_deref(), Some("llama3"));
        assert!(resp.usage.is_none());
    }

    #[test]
    fn parse_chat_response_unknown_format() {
        let runtime = ProviderRuntime {
            id: "test".into(),
            base_url: "".into(),
            api_key: None,
            chat_endpoint: "".into(),
            models_endpoint: None,
            auth_header: None,
            auth_prefix: None,
            request_body_format: RequestBodyFormat::OpenAi,
        };

        let raw = serde_json::json!({ "unknown": "format" });
        let result = parse_chat_response(&runtime, &raw);
        assert!(result.is_err());
    }

    #[test]
    fn build_openai_request_body() {
        let runtime = ProviderRuntime {
            id: "openai".into(),
            base_url: "".into(),
            api_key: None,
            chat_endpoint: "".into(),
            models_endpoint: None,
            auth_header: None,
            auth_prefix: None,
            request_body_format: RequestBodyFormat::OpenAi,
        };

        let messages = vec![ChatMessage {
            role: "user".into(),
            content: "hello".into(),
        }];

        let body = build_request_body(&runtime, &messages, "gpt-4", &ThinkingConfig::default()).unwrap();
        assert_eq!(body["model"], "gpt-4");
        assert!(body["messages"].is_array());
        assert_eq!(body["messages"][0]["role"], "user");
    }

    #[test]
    fn build_ollama_request_body_has_stream_false() {
        let runtime = ProviderRuntime {
            id: "ollama".into(),
            base_url: "".into(),
            api_key: None,
            chat_endpoint: "".into(),
            models_endpoint: None,
            auth_header: None,
            auth_prefix: None,
            request_body_format: RequestBodyFormat::Ollama,
        };

        let messages = vec![];
        let body = build_request_body(&runtime, &messages, "llama3", &ThinkingConfig::default()).unwrap();
        assert_eq!(body["stream"], false);
    }

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

    #[test]
    fn parse_anthropic_response_with_thinking() {
        let runtime = ProviderRuntime {
            id: "anthropic".into(),
            base_url: "".into(),
            api_key: None,
            chat_endpoint: "".into(),
            models_endpoint: None,
            auth_header: None,
            auth_prefix: None,
            request_body_format: RequestBodyFormat::Anthropic,
        };

        let raw = serde_json::json!({
            "content": [
                { "type": "thinking", "thinking": "Let me consider this..." },
                { "type": "text", "text": "The answer is 42." }
            ],
            "model": "claude-sonnet-4-20250514",
            "usage": {
                "input_tokens": 10,
                "output_tokens": 20
            }
        });

        let resp = parse_chat_response(&runtime, &raw).unwrap();
        assert_eq!(resp.content, "The answer is 42.");
        assert_eq!(resp.thinking.as_deref(), Some("Let me consider this..."));
        assert_eq!(resp.model.as_deref(), Some("claude-sonnet-4-20250514"));
    }

    #[test]
    fn parse_anthropic_response_without_thinking() {
        let runtime = ProviderRuntime {
            id: "anthropic".into(),
            base_url: "".into(),
            api_key: None,
            chat_endpoint: "".into(),
            models_endpoint: None,
            auth_header: None,
            auth_prefix: None,
            request_body_format: RequestBodyFormat::Anthropic,
        };

        let raw = serde_json::json!({
            "content": [{ "type": "text", "text": "Hello!" }],
            "model": "claude-3",
            "usage": { "input_tokens": 5, "output_tokens": 3 }
        });

        let resp = parse_chat_response(&runtime, &raw).unwrap();
        assert_eq!(resp.content, "Hello!");
        assert!(resp.thinking.is_none());
    }

    #[test]
    fn build_openai_request_body_with_reasoning_effort() {
        let runtime = ProviderRuntime {
            id: "openai".into(),
            base_url: "".into(),
            api_key: None,
            chat_endpoint: "".into(),
            models_endpoint: None,
            auth_header: None,
            auth_prefix: None,
            request_body_format: RequestBodyFormat::OpenAi,
        };

        let messages = vec![ChatMessage {
            role: "user".into(),
            content: "hello".into(),
        }];

        let thinking = ThinkingConfig {
            enabled: true,
            reasoning_effort: Some("high".into()),
            budget_tokens: None,
        };
        let body = build_request_body(&runtime, &messages, "o3", &thinking).unwrap();
        assert_eq!(body["reasoning_effort"], "high");
    }

    #[test]
    fn build_openai_request_body_thinking_disabled() {
        let runtime = ProviderRuntime {
            id: "openai".into(),
            base_url: "".into(),
            api_key: None,
            chat_endpoint: "".into(),
            models_endpoint: None,
            auth_header: None,
            auth_prefix: None,
            request_body_format: RequestBodyFormat::OpenAi,
        };

        let messages = vec![ChatMessage {
            role: "user".into(),
            content: "hello".into(),
        }];

        let thinking = ThinkingConfig::default();
        let body = build_request_body(&runtime, &messages, "gpt-4", &thinking).unwrap();
        assert!(body.get("reasoning_effort").is_none());
    }

    #[test]
    fn build_anthropic_request_body_with_thinking() {
        let runtime = ProviderRuntime {
            id: "anthropic".into(),
            base_url: "".into(),
            api_key: None,
            chat_endpoint: "".into(),
            models_endpoint: None,
            auth_header: None,
            auth_prefix: None,
            request_body_format: RequestBodyFormat::Anthropic,
        };

        let messages = vec![ChatMessage {
            role: "user".into(),
            content: "hello".into(),
        }];

        let thinking = ThinkingConfig {
            enabled: true,
            budget_tokens: Some(5000),
            reasoning_effort: None,
        };
        let body = build_request_body(&runtime, &messages, "claude-sonnet-4-20250514", &thinking).unwrap();
        assert_eq!(body["thinking"]["type"], "enabled");
        assert_eq!(body["thinking"]["budget_tokens"], 5000);
    }

    #[test]
    fn build_ollama_request_body_with_think() {
        let runtime = ProviderRuntime {
            id: "ollama".into(),
            base_url: "".into(),
            api_key: None,
            chat_endpoint: "".into(),
            models_endpoint: None,
            auth_header: None,
            auth_prefix: None,
            request_body_format: RequestBodyFormat::Ollama,
        };

        let messages = vec![];
        let thinking = ThinkingConfig {
            enabled: true,
            budget_tokens: None,
            reasoning_effort: None,
        };
        let body = build_request_body(&runtime, &messages, "qwen3", &thinking).unwrap();
        assert_eq!(body["think"], true);
    }
}
