//! Model metadata types from the models.dev API.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Map of model id → [`ModelEntry`] for a given provider.
pub type ModelMap = HashMap<String, ModelEntry>;

/// Metadata about a single model from the models.dev API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelEntry {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub family: Option<String>,
    #[serde(default)]
    pub modalities: Option<ModelModalities>,
    #[serde(default)]
    pub limit: Option<ModelLimit>,
    #[serde(default)]
    pub cost: Option<ModelCost>,
    #[serde(default)]
    pub tool_call: Option<bool>,
    #[serde(default)]
    pub structured_output: Option<bool>,
    #[serde(default)]
    pub reasoning: Option<bool>,
    #[serde(default)]
    pub reasoning_options: Option<Vec<ReasoningOption>>,
    #[serde(default)]
    pub temperature: Option<bool>,
    #[serde(default)]
    pub attachment: Option<bool>,
    #[serde(default)]
    pub open_weights: Option<bool>,
    #[serde(default)]
    pub interleaved: Option<serde_json::Value>,
    #[serde(default)]
    pub knowledge: Option<String>,
    #[serde(default)]
    pub release_date: Option<String>,
    #[serde(default)]
    pub last_updated: Option<String>,
}

/// Supported input and output types for a model (e.g. text, image).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelModalities {
    #[serde(default)]
    pub input: Vec<String>,
    #[serde(default)]
    pub output: Vec<String>,
}

/// Context window and output token limits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelLimit {
    #[serde(default)]
    pub context: Option<u64>,
    #[serde(default)]
    pub output: Option<u64>,
}

/// Pricing per token (input, output, cache tiers).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCost {
    #[serde(default)]
    pub input: Option<f64>,
    #[serde(default)]
    pub output: Option<f64>,
    #[serde(default)]
    pub cache_read: Option<f64>,
    #[serde(default)]
    pub cache_write: Option<f64>,
}

/// A reasoning budget option available for this model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningOption {
    #[serde(rename = "type")]
    pub option_type: String,
    #[serde(default)]
    pub values: Option<Vec<String>>,
}
