use crate::config::settings::ThinkingConfig;
use crate::error::{BimoError, Result};
use crate::provider::types::{
    ChatCompletionResponse, ChatMessage, ProviderRuntime, RawModel, RequestBodyFormat, UsageInfo,
};
use futures_util::StreamExt;
use reqwest::Client;
use std::collections::HashSet;
use std::sync::LazyLock;

static HTTP_CLIENT: LazyLock<Client> = LazyLock::new(Client::new);

pub async fn fetch_models(runtime: &ProviderRuntime) -> Result<Vec<RawModel>> {
    let endpoint = match &runtime.models_endpoint {
        Some(ep) => ep,
        None => return Ok(Vec::new()),
    };

    let url = format!("{}{}", runtime.base_url.trim_end_matches('/'), endpoint);
    let mut req = HTTP_CLIENT.get(&url);
    req = apply_auth(req, runtime)?;

    let resp = req.send().await.map_err(|e| {
        BimoError::Network(format!("failed to fetch models from {}: {e}", runtime.id))
    })?;

    let status = resp.status();
    if !status.is_success() {
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

    parse_models_response(&raw)
}

fn parse_models_response(raw: &serde_json::Value) -> Result<Vec<RawModel>> {
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
            let tier =
                infer_tier_from_pricing(&v).or_else(|| infer_tier_from_name(&id, name.as_deref()));
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

static KNOWN_FREE_MODELS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    HashSet::from([
        "deepseek-v4-flash-free",
        "mimo-v2.5-free",
        "laguna-s-2.1-free",
        "ling-3.0-flash-free",
        "north-mini-code-free",
        "nemotron-3-ultra-free",
        "big-pickle",
    ])
});

fn infer_tier_from_name(id: &str, name: Option<&str>) -> Option<String> {
    let haystack = name.unwrap_or(id).to_lowercase();
    if haystack.contains("free") || KNOWN_FREE_MODELS.contains(&haystack.as_str()) {
        Some("free".into())
    } else {
        None
    }
}

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
            (Some(p), Some(c)) => Some(if p == 0.0 && c == 0.0 { "free" } else { "paid" }.into()),
            (Some(p), None) => Some(if p == 0.0 { "free" } else { "paid" }.into()),
            (None, Some(c)) => Some(if c == 0.0 { "free" } else { "paid" }.into()),
            (None, None) => Some("free".into()),
        }
    } else {
        None
    }
}

fn extract_context_window(model: &serde_json::Value) -> Option<u32> {
    for key in &[
        "context_length",
        "max_context",
        "context_window",
        "max_context_length",
    ] {
        if let Some(val) = model.get(*key) {
            if let Some(n) = val.as_u64() {
                return Some(n as u32);
            }
            if let Some(s) = val.as_str()
                && let Ok(n) = s.parse::<u32>()
            {
                return Some(n);
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

pub async fn chat_completion_streaming(
    runtime: &ProviderRuntime,
    messages: &[ChatMessage],
    model: &str,
    thinking: &ThinkingConfig,
) -> Result<impl futures_util::Stream<Item = std::result::Result<serde_json::Value, BimoError>>> {
    let url = format!(
        "{}{}",
        runtime.base_url.trim_end_matches('/'),
        runtime.chat_endpoint
    );

    let mut body = build_request_body(runtime, messages, model, thinking)?;
    if let Some(obj) = body.as_object_mut() {
        obj.insert("stream".into(), serde_json::json!(true));
    }

    let mut req = HTTP_CLIENT.post(&url).json(&body);
    req = apply_auth(req, runtime)?;

    let resp = req.send().await.map_err(|e| {
        BimoError::Network(format!(
            "streaming chat request to {} failed: {e}",
            runtime.id
        ))
    })?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(BimoError::Provider(format!(
            "streaming chat failed ({}): {}",
            status, body
        )));
    }

    let _provider_id = runtime.id.clone();

    let stream = async_stream::stream! {
        let mut buffer = String::new();
        let mut bytes_stream = resp.bytes_stream();

        while let Some(chunk_result) = bytes_stream.next().await {
            let chunk = match chunk_result {
                Ok(c) => c,
                Err(e) => {
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
                        return;
                    }
                    match serde_json::from_str::<serde_json::Value>(data) {
                        Ok(val) => yield Ok(val),
                        Err(_) => {}
                    }
                }
            }
        }
    };

    Ok(stream)
}

pub fn extract_stream_delta(
    chunk: &serde_json::Value,
    format: &RequestBodyFormat,
) -> Option<String> {
    match format {
        RequestBodyFormat::OpenAi => chunk
            .get("choices")?
            .as_array()?
            .first()?
            .get("delta")?
            .get("content")?
            .as_str()
            .map(String::from),
        RequestBodyFormat::Anthropic => {
            let type_str = chunk.get("type")?.as_str()?;
            if type_str == "content_block_delta" {
                chunk.get("delta")?.get("text")?.as_str().map(String::from)
            } else {
                None
            }
        }
    }
}

pub fn build_request_body(
    runtime: &ProviderRuntime,
    messages: &[ChatMessage],
    model: &str,
    thinking: &ThinkingConfig,
) -> Result<serde_json::Value> {
    match runtime.request_body_format {
        RequestBodyFormat::OpenAi => {
            let msgs: Vec<serde_json::Value> = messages
                .iter()
                .map(|m| serde_json::json!({ "role": m.role, "content": m.content }))
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
                .map(|m| serde_json::json!({ "role": m.role, "content": m.content }))
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
    }
}

pub fn parse_chat_response(raw: &serde_json::Value) -> Result<ChatCompletionResponse> {
    // Try OpenAI format
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

    Err(BimoError::Serialization(format!(
        "unable to parse chat response: {}",
        serde_json::to_string(raw).unwrap_or_default()
    )))
}
