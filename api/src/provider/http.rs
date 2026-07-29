use crate::config::ThinkingConfig;
use crate::error::{BimoError, Result};
use futures_util::StreamExt;
use reqwest::Client;
use std::sync::LazyLock;
use tracing;

use super::types::{
    ChatCompletionResponse, ChatMessage, ProviderRuntime, RawModel, RequestBodyFormat, UsageInfo,
};

static HTTP_CLIENT: LazyLock<Client> = LazyLock::new(Client::new);

/// Fetch available models from the given provider.
pub async fn fetch_models(runtime: &ProviderRuntime) -> Result<Vec<RawModel>> {
    let endpoint = match &runtime.models_endpoint {
        Some(ep) => ep,
        None => {
            tracing::debug!(provider = %runtime.id, "no models endpoint, skipping fetch");
            return Ok(Vec::new());
        }
    };

    let client = &*HTTP_CLIENT;
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
                    let tier = infer_tier_from_pricing(&v);
                    let context_window = extract_context_window(&v);
                    Some(RawModel {
                        id,
                        name,
                        tier,
                        context_window,
                    })
                })
                .collect();
            Ok(models)
        }
        RequestBodyFormat::Ollama => {
            let arr = raw
                .get("models")
                .and_then(|d| d.as_array())
                .cloned()
                .unwrap_or_default();
            let models = arr
                .into_iter()
                .filter_map(|v| {
                    let id = v.get("name")?.as_str()?.to_string();
                    let context_window = extract_context_window(&v);
                    Some(RawModel {
                        id,
                        name: None,
                        tier: None,
                        context_window,
                    })
                })
                .collect();
            Ok(models)
        }
    }
}

/// Try to infer a model tier from pricing metadata in the API response.
/// Returns "free" if prompt and completion costs are both zero or absent,
/// "paid" if any pricing is present and non-zero, or `None` if indeterminate.
fn infer_tier_from_pricing(model: &serde_json::Value) -> Option<String> {
    if let Some(pricing) = model.get("pricing") {
        let prompt_cost = pricing.get("prompt").and_then(|v| {
            v.as_f64()
                .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
        });
        let completion_cost = pricing.get("completion").and_then(|v| {
            v.as_f64()
                .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
        });

        match (prompt_cost, completion_cost) {
            (Some(p), Some(c)) => {
                if p == 0.0 && c == 0.0 {
                    Some("free".into())
                } else {
                    Some("paid".into())
                }
            }
            (Some(p), None) => {
                if p == 0.0 {
                    Some("free".into())
                } else {
                    Some("paid".into())
                }
            }
            (None, Some(c)) => {
                if c == 0.0 {
                    Some("free".into())
                } else {
                    Some("paid".into())
                }
            }
            (None, None) => Some("free".into()),
        }
    } else {
        None
    }
}

/// Extract the context window size from a model entry in a provider response.
/// Checks common field names used by various providers.
fn extract_context_window(model: &serde_json::Value) -> Option<u32> {
    for key in &[
        "context_length",
        "max_context",
        "context_window",
        "max_context_length",
    ] {
        if let Some(val) = model.get(*key) {
            // Try direct number
            if let Some(n) = val.as_u64() {
                return Some(n as u32);
            }
            // Try string that can be parsed as number
            if let Some(s) = val.as_str() {
                if let Ok(n) = s.parse::<u32>() {
                    return Some(n);
                }
            }
        }
    }
    None
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

/// Send a chat completion request to the provider.
pub async fn chat_completion(
    runtime: &ProviderRuntime,
    messages: &[ChatMessage],
    model: &str,
    thinking: &ThinkingConfig,
) -> Result<ChatCompletionResponse> {
    let client = &*HTTP_CLIENT;
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

/// Send a streaming chat completion request. Returns a stream of raw JSON
/// chunks (one per SSE `data:` line). The caller is responsible for parsing
/// content deltas out of each chunk according to the provider format.
pub async fn chat_completion_streaming(
    runtime: &ProviderRuntime,
    messages: &[ChatMessage],
    model: &str,
    thinking: &ThinkingConfig,
) -> Result<
    impl futures_util::Stream<Item = std::result::Result<serde_json::Value, BimoError>> + use<>,
> {
    let client = &*HTTP_CLIENT;
    let url = format!(
        "{}{}",
        runtime.base_url.trim_end_matches('/'),
        runtime.chat_endpoint
    );

    tracing::debug!(provider = %runtime.id, model, url = %url, "sending streaming chat request");
    let mut body = build_request_body(runtime, messages, model, thinking)?;

    // Force streaming on
    if let Some(obj) = body.as_object_mut() {
        obj.insert("stream".into(), serde_json::json!(true));
    }

    let mut req = client.post(&url).json(&body);
    req = apply_auth(req, runtime)?;

    let resp = req.send().await.map_err(|e| {
        tracing::error!(provider = %runtime.id, error = %e, "streaming chat request failed");
        BimoError::Network(format!(
            "streaming chat request to {} failed: {e}",
            runtime.id
        ))
    })?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        tracing::error!(provider = %runtime.id, status = %status, body = %body, "streaming chat failed");
        return Err(BimoError::Provider(format!(
            "streaming chat failed ({}): {}",
            status, body
        )));
    }

    let provider_id = runtime.id.clone();
    let stream = async_stream::stream! {
        let mut buffer = String::new();
        let mut bytes_stream = resp.bytes_stream();

        while let Some(chunk_result) = bytes_stream.next().await {
            let chunk = match chunk_result {
                Ok(c) => c,
                Err(e) => {
                    tracing::error!(provider = %provider_id, error = %e, "stream chunk read error");
                    yield Err(BimoError::Network(format!("stream read error: {e}")));
                    return;
                }
            };

            buffer.push_str(&String::from_utf8_lossy(&chunk));

            while let Some(newline_pos) = buffer.find('\n') {
                let line = buffer[..newline_pos].trim().to_string();
                buffer = buffer[newline_pos + 1..].to_string();

                if let Some(data) = line.strip_prefix("data: ") {
                    let data = data.trim();
                    if data == "[DONE]" {
                        tracing::debug!(provider = %provider_id, "stream [DONE] received");
                        return;
                    }
                    match serde_json::from_str::<serde_json::Value>(data) {
                        Ok(val) => yield Ok(val),
                        Err(e) => {
                            tracing::warn!(provider = %provider_id, error = %e, data = %data, "failed to parse stream chunk");
                        }
                    }
                }
            }
        }

        tracing::debug!(provider = %provider_id, "stream ended");
    };

    Ok(stream)
}

/// Extract text content deltas from a streaming chunk, handling OpenAI,
/// Anthropic, and Ollama formats.
pub fn extract_stream_delta(
    chunk: &serde_json::Value,
    format: &RequestBodyFormat,
) -> Option<String> {
    match format {
        RequestBodyFormat::OpenAi => {
            // OpenAI: { choices: [{ delta: { content: "..." } }] }
            chunk
                .get("choices")?
                .as_array()?
                .first()?
                .get("delta")?
                .get("content")?
                .as_str()
                .map(String::from)
        }
        RequestBodyFormat::Anthropic => {
            // Anthropic: { type: "content_block_delta", delta: { text: "..." } }
            let type_str = chunk.get("type")?.as_str()?;
            if type_str == "content_block_delta" {
                chunk.get("delta")?.get("text")?.as_str().map(String::from)
            } else {
                None
            }
        }
        RequestBodyFormat::Ollama => {
            // Ollama: { message: { content: "..." } }
            chunk
                .get("message")?
                .get("content")?
                .as_str()
                .map(String::from)
        }
    }
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
            let system_text: Vec<String> = messages
                .iter()
                .filter(|m| m.role == "system")
                .map(|m| m.content.clone())
                .collect();
            let non_system: Vec<serde_json::Value> = messages
                .iter()
                .filter(|m| m.role != "system")
                .map(|m| {
                    serde_json::json!({
                        "role": m.role,
                        "content": m.content,
                    })
                })
                .collect();
            let mut body = serde_json::json!({
                "model": model,
                "messages": non_system,
                "max_tokens": 8192,
            });
            if !system_text.is_empty() {
                body.as_object_mut()
                    .unwrap()
                    .insert("system".into(), serde_json::json!(system_text.join("\n")));
            }
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

    // Try Anthropic format
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

    fn make_runtime(format: RequestBodyFormat) -> ProviderRuntime {
        ProviderRuntime {
            id: "test".into(),
            base_url: "".into(),
            api_key: None,
            chat_endpoint: "".into(),
            models_endpoint: None,
            auth_header: None,
            auth_prefix: None,
            request_body_format: format,
        }
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
        let runtime = make_runtime(RequestBodyFormat::OpenAi);

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
        let runtime = make_runtime(RequestBodyFormat::Anthropic);

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
        let runtime = make_runtime(RequestBodyFormat::Ollama);

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
        let runtime = make_runtime(RequestBodyFormat::OpenAi);
        let raw = serde_json::json!({ "unknown": "format" });
        let result = parse_chat_response(&runtime, &raw);
        assert!(result.is_err());
    }

    #[test]
    fn build_openai_request_body() {
        let runtime = make_runtime(RequestBodyFormat::OpenAi);
        let messages = vec![ChatMessage {
            role: "user".into(),
            content: "hello".into(),
        }];

        let body =
            build_request_body(&runtime, &messages, "gpt-4", &ThinkingConfig::default()).unwrap();
        assert_eq!(body["model"], "gpt-4");
        assert!(body["messages"].is_array());
        assert_eq!(body["messages"][0]["role"], "user");
    }

    #[test]
    fn build_ollama_request_body_has_stream_false() {
        let runtime = make_runtime(RequestBodyFormat::Ollama);
        let messages = vec![];
        let body =
            build_request_body(&runtime, &messages, "llama3", &ThinkingConfig::default()).unwrap();
        assert_eq!(body["stream"], false);
    }

    #[test]
    fn parse_anthropic_response_with_thinking() {
        let runtime = make_runtime(RequestBodyFormat::Anthropic);

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
        let runtime = make_runtime(RequestBodyFormat::Anthropic);

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
        let runtime = make_runtime(RequestBodyFormat::OpenAi);
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
        let runtime = make_runtime(RequestBodyFormat::OpenAi);
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
        let runtime = make_runtime(RequestBodyFormat::Anthropic);
        let messages = vec![ChatMessage {
            role: "user".into(),
            content: "hello".into(),
        }];

        let thinking = ThinkingConfig {
            enabled: true,
            budget_tokens: Some(5000),
            reasoning_effort: None,
        };
        let body =
            build_request_body(&runtime, &messages, "claude-sonnet-4-20250514", &thinking).unwrap();
        assert_eq!(body["thinking"]["type"], "enabled");
        assert_eq!(body["thinking"]["budget_tokens"], 5000);
    }

    #[test]
    fn build_ollama_request_body_with_think() {
        let runtime = make_runtime(RequestBodyFormat::Ollama);
        let messages = vec![];
        let thinking = ThinkingConfig {
            enabled: true,
            budget_tokens: None,
            reasoning_effort: None,
        };
        let body = build_request_body(&runtime, &messages, "qwen3", &thinking).unwrap();
        assert_eq!(body["think"], true);
    }

    #[test]
    fn infer_tier_free_model() {
        let model = serde_json::json!({
            "id": "free-model",
            "pricing": { "prompt": "0", "completion": "0" }
        });
        assert_eq!(infer_tier_from_pricing(&model).as_deref(), Some("free"));
    }

    #[test]
    fn infer_tier_paid_model() {
        let model = serde_json::json!({
            "id": "gpt-4",
            "pricing": { "prompt": "0.00003", "completion": "0.00006" }
        });
        assert_eq!(infer_tier_from_pricing(&model).as_deref(), Some("paid"));
    }

    #[test]
    fn infer_tier_no_pricing() {
        let model = serde_json::json!({ "id": "gpt-4" });
        assert!(infer_tier_from_pricing(&model).is_none());
    }

    #[test]
    fn parse_models_response_with_pricing() {
        let runtime = make_runtime(RequestBodyFormat::OpenAi);
        let raw = serde_json::json!({
            "data": [
                { "id": "free-model", "pricing": { "prompt": "0", "completion": "0" } },
                { "id": "paid-model", "pricing": { "prompt": "0.01", "completion": "0.02" } },
                { "id": "unknown-model" }
            ]
        });
        let models = parse_models_response(&runtime, &raw).unwrap();
        assert_eq!(models.len(), 3);
        assert_eq!(models[0].tier.as_deref(), Some("free"));
        assert_eq!(models[1].tier.as_deref(), Some("paid"));
        assert!(models[2].tier.is_none());
    }
}
