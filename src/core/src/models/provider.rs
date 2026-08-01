//! Concrete model client construction from provider configuration.
//!
//! Bridges [`ApiFormat`] and provider connection details to the concrete
//! aisdk SDK model instances used at runtime.

use aisdk::core::DynamicModel;
use aisdk::providers::{Anthropic, Google, OpenAICompatible};

use crate::config::ApiFormat;
use crate::error::{BimoError, Result};

/// Erased OpenAI-compatible model type used at runtime.
type OpenAIModel = OpenAICompatible<DynamicModel>;
/// Erased Anthropic model type used at runtime.
type AnthropicModel = Anthropic<DynamicModel>;
/// Erased Google model type used at runtime.
type GoogleModel = Google<DynamicModel>;

/// Erased model type — dispatches to the concrete provider SDK at build time.
pub enum ModelProvider {
    OpenAI(Box<OpenAIModel>),
    Anthropic(Box<AnthropicModel>),
    Google(Box<GoogleModel>),
}

impl ModelProvider {
    /// Builds the appropriate model variant from config fields.
    pub async fn build(
        api_format: &ApiFormat,
        base_url: &str,
        model_name: &str,
        api_key: Option<String>,
    ) -> Result<Self> {
        match api_format {
            ApiFormat::OpenAICompatible | ApiFormat::OpenAI => {
                Self::build_openai(base_url, model_name, api_key).await
            }
            ApiFormat::Google => Self::build_google(base_url, model_name, api_key).await,
            ApiFormat::Anthropic => Self::build_anthropic(base_url, model_name, api_key).await,
            ApiFormat::Other(fmt) => Err(BimoError::Provider(format!(
                "unsupported API format: {fmt}"
            ))),
        }
    }

    /// Builds an OpenAI-compatible model client.
    async fn build_openai(
        base_url: &str,
        model_name: &str,
        api_key: Option<String>,
    ) -> Result<Self> {
        let mut builder = OpenAICompatible::<DynamicModel>::builder()
            .base_url(base_url)
            .model_name(model_name);
        if let Some(key) = api_key {
            builder = builder.api_key(key);
        }
        builder
            .build()
            .map(|m| Self::OpenAI(Box::new(m)))
            .map_err(|e| {
                BimoError::Provider(format!("Failed to build OpenAI-compatible model: {e}"))
            })
    }

    /// Builds an Anthropic model client.
    async fn build_anthropic(
        base_url: &str,
        model_name: &str,
        api_key: Option<String>,
    ) -> Result<Self> {
        let mut builder = Anthropic::<DynamicModel>::builder()
            .base_url(base_url)
            .model_name(model_name);
        if let Some(key) = api_key {
            builder = builder.api_key(key);
        }
        builder
            .build()
            .map(|m| Self::Anthropic(Box::new(m)))
            .map_err(|e| BimoError::Provider(format!("Failed to build Anthropic model: {e}")))
    }

    /// Builds a Google model client.
    async fn build_google(
        base_url: &str,
        model_name: &str,
        api_key: Option<String>,
    ) -> Result<Self> {
        let mut builder = Google::<DynamicModel>::builder()
            .base_url(base_url)
            .model_name(model_name);
        if let Some(key) = api_key {
            builder = builder.api_key(key);
        }
        builder
            .build()
            .map(|m| Self::Google(Box::new(m)))
            .map_err(|e| BimoError::Provider(format!("Failed to build Google model: {e}")))
    }
}

/// Applies a method to whichever concrete model variant is stored.
///
/// `$receiver.$method(*model, ...)` is expanded once per variant and awaited,
/// so the handler only needs to be generic over the model type.
macro_rules! dispatch_model {
    ($provider:expr, $receiver:expr, $method:ident $(, $arg:expr)* $(,)?) => {
        match $provider {
            crate::models::ModelProvider::OpenAI(model) => {
                $receiver.$method(*model $(, $arg)*).await
            }
            crate::models::ModelProvider::Anthropic(model) => {
                $receiver.$method(*model $(, $arg)*).await
            }
            crate::models::ModelProvider::Google(model) => {
                $receiver.$method(*model $(, $arg)*).await
            }
        }
    };
}
pub(crate) use dispatch_model;
