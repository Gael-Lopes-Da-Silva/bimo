use crate::error::Result;
use crate::provider::{self, ProviderRuntime, RawModel};
use serde::{Deserialize, Serialize};

/// A model that has been fetched and is available for selection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub provider_id: String,
    /// Tier label (e.g. "free", "paid") for providers that distinguish model tiers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tier: Option<String>,
    /// Context window size in tokens, if known from the provider or our built-in map.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_window: Option<u32>,
}

/// Fetch available models for a provider and return them as [`ModelInfo`] entries.
pub async fn fetch_models_for_provider(runtime: &ProviderRuntime) -> Result<Vec<ModelInfo>> {
    let raw_models: Vec<RawModel> = provider::fetch_models(runtime).await?;

    let models = raw_models
        .into_iter()
        .map(|rm| {
            let id = rm.id.clone();
            let name = rm.name.unwrap_or(id.clone());
            let context_window = rm
                .context_window
                .or_else(|| lookup_known_context_window(&id));
            ModelInfo {
                id,
                name,
                provider_id: runtime.id.clone(),
                tier: rm.tier,
                context_window,
            }
        })
        .collect();

    Ok(models)
}

/// Static lookup of known context window sizes by model id or family pattern.
/// Used as a fallback when the provider API doesn't return context window info.
pub fn lookup_known_context_window(model_id: &str) -> Option<u32> {
    let lower = model_id.to_lowercase();

    // ── Anthropic Claude ──────────────────────────────────────────────
    if lower.contains("claude") {
        if lower.contains("sonnet-4")
            || lower.contains("opus-4")
            || lower.contains("haiku-3.5")
            || lower.contains("3.5")
            || lower.contains("3-opus")
            || lower.contains("3-haiku")
            || lower.contains("3-sonnet")
        {
            return Some(200_000);
        }
        return Some(100_000);
    }

    // ── OpenAI o-series reasoning ────────────────────────────────────
    if lower.contains("o3") || lower.contains("o4") || lower.contains("o1") {
        return Some(200_000);
    }

    // ── GPT 4.1 series (1M context) ─────────────────────────────────
    if lower.contains("gpt-4.1") || lower.contains("gpt4.1") {
        return Some(1_000_000);
    }

    // ── GPT-4.5 series (128K context) ───────────────────────────────
    if lower.contains("gpt-4.5") || lower.contains("gpt4.5") {
        return Some(128_000);
    }

    // ── GPT-4o / GPT-4 Turbo ────────────────────────────────────────
    if lower.contains("gpt-4o")
        || lower.contains("gpt4o")
        || lower.contains("gpt-4-turbo")
        || lower.contains("gpt4turbo")
    {
        return Some(128_000);
    }

    // ── GPT-4 (base, < 4.1) ─────────────────────────────────────────
    if lower.contains("gpt-4") || lower.contains("gpt4") {
        return Some(8_192);
    }

    // ── GPT-3.5 ─────────────────────────────────────────────────────
    if lower.contains("gpt-3.5") || lower.contains("gpt3.5") {
        return Some(16_385);
    }

    // ── Google Gemini ───────────────────────────────────────────────
    if lower.contains("gemini") {
        if lower.contains("1.5") {
            return if lower.contains("pro") {
                Some(2_000_000)
            } else {
                Some(1_000_000)
            };
        }
        return Some(1_000_000);
    }

    // ── Meta Llama ──────────────────────────────────────────────────
    if lower.contains("llama") {
        if lower.contains("llama2") || lower.contains("llama-2") {
            return Some(4_096);
        }
        if lower.contains("3.1") || lower.contains("3.2") || lower.contains("3.3") {
            return Some(128_000);
        }
        if lower.contains("llama3") || lower.contains("llama-3") {
            return Some(8_192);
        }
        return Some(128_000);
    }

    // ── DeepSeek ────────────────────────────────────────────────────
    if lower.contains("deepseek") {
        return Some(128_000);
    }

    // ── Mistral / Mixtral / Codestral ───────────────────────────────
    if lower.contains("codestral") {
        return Some(256_000);
    }
    if lower.contains("mistral") || lower.contains("mixtral") {
        return Some(128_000);
    }

    // ── Qwen ────────────────────────────────────────────────────────
    if lower.contains("qwen") {
        if lower.contains("qwen2.5")
            || lower.contains("qwen-2.5")
            || lower.contains("qwen3")
            || lower.contains("qwen-3")
        {
            return Some(128_000);
        }
        return Some(32_768);
    }

    // ── Cohere Command R ────────────────────────────────────────────
    if lower.contains("command-r") || lower.contains("command-r-plus") {
        return Some(128_000);
    }

    // ── Unknown model — no static data ──────────────────────────────
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_claude_models() {
        assert_eq!(
            lookup_known_context_window("claude-sonnet-4-20250514"),
            Some(200_000)
        );
        assert_eq!(lookup_known_context_window("claude-3-opus"), Some(200_000));
        assert_eq!(
            lookup_known_context_window("claude-3-haiku-20240307"),
            Some(200_000)
        );
        assert_eq!(lookup_known_context_window("claude-2"), Some(100_000));
        assert_eq!(
            lookup_known_context_window("claude-instant-1"),
            Some(100_000)
        );
    }

    #[test]
    fn lookup_openai_o_series() {
        assert_eq!(lookup_known_context_window("o1"), Some(200_000));
        assert_eq!(lookup_known_context_window("o3-mini"), Some(200_000));
        assert_eq!(lookup_known_context_window("o4-mini"), Some(200_000));
    }

    #[test]
    fn lookup_gpt4_series() {
        assert_eq!(lookup_known_context_window("gpt-4.1-nano"), Some(1_000_000));
        assert_eq!(
            lookup_known_context_window("gpt-4.5-preview"),
            Some(128_000)
        );
        assert_eq!(lookup_known_context_window("gpt-4o"), Some(128_000));
        assert_eq!(lookup_known_context_window("gpt-4-turbo"), Some(128_000));
        assert_eq!(lookup_known_context_window("gpt-4"), Some(8_192));
    }

    #[test]
    fn lookup_gpt35() {
        assert_eq!(lookup_known_context_window("gpt-3.5-turbo"), Some(16_385));
    }

    #[test]
    fn lookup_gemini() {
        assert_eq!(
            lookup_known_context_window("gemini-1.5-pro"),
            Some(2_000_000)
        );
        assert_eq!(
            lookup_known_context_window("gemini-1.5-flash"),
            Some(1_000_000)
        );
        assert_eq!(
            lookup_known_context_window("gemini-2.5-pro"),
            Some(1_000_000)
        );
    }

    #[test]
    fn lookup_llama() {
        assert_eq!(lookup_known_context_window("llama2"), Some(4_096));
        assert_eq!(lookup_known_context_window("llama-2"), Some(4_096));
        assert_eq!(lookup_known_context_window("llama3"), Some(8_192));
        assert_eq!(lookup_known_context_window("llama-3"), Some(8_192));
        assert_eq!(lookup_known_context_window("llama3.1"), Some(128_000));
        assert_eq!(lookup_known_context_window("llama3.2"), Some(128_000));
        assert_eq!(lookup_known_context_window("llama-3.3"), Some(128_000));
        assert_eq!(
            lookup_known_context_window("unknown-llama-variant"),
            Some(128_000)
        );
    }

    #[test]
    fn lookup_deepseek() {
        assert_eq!(lookup_known_context_window("deepseek-chat"), Some(128_000));
        assert_eq!(lookup_known_context_window("deepseek-coder"), Some(128_000));
    }

    #[test]
    fn lookup_mistral_family() {
        assert_eq!(lookup_known_context_window("mistral-large"), Some(128_000));
        assert_eq!(lookup_known_context_window("mixtral-8x7b"), Some(128_000));
        assert_eq!(lookup_known_context_window("codestral"), Some(256_000));
    }

    #[test]
    fn lookup_qwen() {
        assert_eq!(lookup_known_context_window("qwen2.5"), Some(128_000));
        assert_eq!(lookup_known_context_window("qwen-2.5"), Some(128_000));
        assert_eq!(lookup_known_context_window("qwen3"), Some(128_000));
        assert_eq!(lookup_known_context_window("qwen-3"), Some(128_000));
        assert_eq!(lookup_known_context_window("qwen2"), Some(32_768));
    }

    #[test]
    fn lookup_command_r() {
        assert_eq!(lookup_known_context_window("command-r"), Some(128_000));
        assert_eq!(lookup_known_context_window("command-r-plus"), Some(128_000));
    }

    #[test]
    fn lookup_unknown_model() {
        assert_eq!(lookup_known_context_window("totally-unknown-model"), None);
        assert_eq!(lookup_known_context_window(""), None);
    }

    #[test]
    fn lookup_case_insensitive() {
        assert_eq!(lookup_known_context_window("GPT-4o"), Some(128_000));
        assert_eq!(
            lookup_known_context_window("Claude-3-Sonnet"),
            Some(200_000)
        );
        assert_eq!(lookup_known_context_window("LLAMA3.1"), Some(128_000));
    }
}
