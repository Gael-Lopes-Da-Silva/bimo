use aisdk::core::DynamicModel;
use aisdk::providers::{Anthropic, OpenAICompatible};

use crate::error::{BimoError, Result};
use crate::provider::types::{ProviderRuntime, RequestBodyFormat};

pub enum AisdkProvider {
    OpenAiCompatible(OpenAICompatible<DynamicModel>),
    Anthropic(Anthropic<DynamicModel>),
}

impl AisdkProvider {
    pub fn from_runtime(runtime: &ProviderRuntime, model_id: &str) -> Result<Self> {
        match runtime.request_body_format {
            RequestBodyFormat::OpenAi => {
                let mut builder = OpenAICompatible::<DynamicModel>::builder()
                    .model_name(model_id)
                    .base_url(runtime.base_url.clone())
                    .provider_name(runtime.id.clone());
                if let Some(ref key) = runtime.api_key {
                    builder = builder.api_key(key.clone());
                }
                let provider = builder
                    .build()
                    .map_err(|e| BimoError::Provider(format!("failed to build aisdk provider: {e}")))?;
                Ok(Self::OpenAiCompatible(provider))
            }
            RequestBodyFormat::Anthropic => {
                let mut builder = Anthropic::<DynamicModel>::builder()
                    .model_name(model_id)
                    .base_url(runtime.base_url.clone())
                    .provider_name(runtime.id.clone());
                if let Some(ref key) = runtime.api_key {
                    builder = builder.api_key(key.clone());
                }
                let provider = builder
                    .build()
                    .map_err(|e| BimoError::Provider(format!("failed to build aisdk provider: {e}")))?;
                Ok(Self::Anthropic(provider))
            }
        }
    }
}
