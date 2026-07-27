use crate::config::{AppConfig, CustomProviderConfig};
use crate::error::{BimoError, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

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

        let api_key = api_key.or_else(|| {
            let env = match info.id.as_str() {
                "openai" => std::env::var("OPENAI_API_KEY").ok(),
                "anthropic" => std::env::var("ANTHROPIC_API_KEY").ok(),
                _ => None,
            };
            env
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
        None => return Ok(Vec::new()), // provider doesn't expose a models list
    };

    let client = Client::new();
    let url = format!("{}{}", runtime.base_url.trim_end_matches('/'), endpoint);
    let mut req = client.get(&url);

    req = apply_auth(req, runtime)?;

    let resp = req.send().await.map_err(|e| {
        BimoError::Network(format!("failed to fetch models from {}: {e}", runtime.id))
    })?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(BimoError::Network(format!(
            "model fetch failed ({}): {}",
            status, body
        )));
    }

    let raw: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| BimoError::Serialization(format!("failed to parse model list: {e}")))?;

    parse_models_response(runtime, &raw)
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
    } else if let Some(header) = &runtime.auth_header {
        if let Some(key) = &runtime.api_key {
            req = req.header(header.as_str(), key.as_str());
        }
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
) -> Result<ChatCompletionResponse> {
    let client = Client::new();
    let url = format!(
        "{}{}",
        runtime.base_url.trim_end_matches('/'),
        runtime.chat_endpoint
    );

    let body = build_request_body(runtime, messages, model)?;

    let mut req = client.post(&url).json(&body);
    req = apply_auth(req, runtime)?;

    let resp = req
        .send()
        .await
        .map_err(|e| BimoError::Network(format!("chat request to {} failed: {e}", runtime.id)))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(BimoError::Provider(format!(
            "chat completion failed ({}): {}",
            status, body
        )));
    }

    let raw: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| BimoError::Serialization(format!("failed to parse chat response: {e}")))?;

    parse_chat_response(runtime, &raw)
}

fn build_request_body(
    runtime: &ProviderRuntime,
    messages: &[ChatMessage],
    model: &str,
) -> Result<serde_json::Value> {
    match runtime.request_body_format {
        RequestBodyFormat::OpenAi | RequestBodyFormat::Anthropic => {
            let msgs: Vec<serde_json::Value> = messages
                .iter()
                .map(|m| {
                    serde_json::json!({
                        "role": m.role,
                        "content": m.content,
                    })
                })
                .collect();
            Ok(serde_json::json!({
                "model": model,
                "messages": msgs,
            }))
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
            Ok(serde_json::json!({
                "model": model,
                "messages": msgs,
                "stream": false,
            }))
        }
    }
}

fn parse_chat_response(
    _runtime: &ProviderRuntime,
    raw: &serde_json::Value,
) -> Result<ChatCompletionResponse> {
    // OpenAI-compatible response: { "choices": [{ "message": { "content": "..." } }], ... }
    // Anthropic response: { "content": [{ "text": "..." }], ... }
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
            model,
            usage,
        });
    }

    // Try Anthropic format
    if let Some(text) = raw
        .get("content")
        .and_then(|c| c.as_array())
        .and_then(|a| a.first())
        .and_then(|c| c.get("text"))
        .and_then(|t| t.as_str())
    {
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
            content: text.to_string(),
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
            model,
            usage: None,
        });
    }

    Err(BimoError::Serialization(format!(
        "unable to parse chat response: {}",
        serde_json::to_string(raw).unwrap_or_default()
    )))
}
