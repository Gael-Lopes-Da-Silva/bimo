//! Local provider registry — built-in local providers.

use crate::config::{ApiFormat, Provider};

/// Registry of built-in local providers known to the system
/// (ollama, lmstudio, vllm, llamacpp).
#[derive(Debug, Clone, Copy, Default)]
pub struct LocalProviderRegistry;

impl LocalProviderRegistry {
    /// Creates a new registry.
    pub fn new() -> Self {
        Self
    }

    /// Returns the built-in local providers known to the system.
    pub fn builtin(&self) -> Vec<Provider> {
        vec![
            Provider::local(
                "ollama",
                "Ollama",
                "http://localhost:11434/v1",
                ApiFormat::OpenAICompatible,
            ),
            Provider::local(
                "lmstudio",
                "LM Studio",
                "http://localhost:1234/v1",
                ApiFormat::OpenAICompatible,
            ),
            Provider::local(
                "vllm",
                "vLLM",
                "http://localhost:8000/v1",
                ApiFormat::OpenAICompatible,
            ),
            Provider::local(
                "llamacpp",
                "llama.cpp",
                "http://localhost:8080/v1",
                ApiFormat::OpenAICompatible,
            ),
        ]
    }

    /// Looks up a built-in provider by id or name (case-insensitive).
    pub fn find(&self, id_or_name: &str) -> Option<Provider> {
        let lower = id_or_name.to_lowercase();
        self.builtin()
            .into_iter()
            .find(|p| p.id.to_lowercase() == lower || p.name.to_lowercase() == lower)
    }
}
