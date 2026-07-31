//! Model metadata types from the models.dev API.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Map of model id → [`ModelEntry`] for a given provider.
pub type ModelMap = HashMap<String, ModelEntry>;

/// Metadata about a single model from the models.dev API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelEntry {
    /// Model identifier (e.g. `"gpt-4o"`).
    pub id: String,
    /// Human-readable model name.
    pub name: String,
    /// Optional short description of the model.
    #[serde(default)]
    pub description: Option<String>,
    /// Optional model family/grouping (e.g. `"gpt-4"`).
    #[serde(default)]
    pub family: Option<String>,
    /// Supported input/output modalities.
    #[serde(default)]
    pub modalities: Option<ModelModalities>,
    /// Context and output token limits.
    #[serde(default)]
    pub limit: Option<ModelLimit>,
    /// Per-token pricing.
    #[serde(default)]
    pub cost: Option<ModelCost>,
    /// Whether the model supports tool/function calling.
    #[serde(default)]
    pub tool_call: Option<bool>,
    /// Whether the model supports structured (schema-constrained) output.
    #[serde(default)]
    pub structured_output: Option<bool>,
    /// Whether the model supports reasoning.
    #[serde(default)]
    pub reasoning: Option<bool>,
    /// Reasoning budget options available for the model.
    #[serde(default)]
    pub reasoning_options: Option<Vec<ReasoningOption>>,
    /// Whether the temperature parameter is supported.
    #[serde(default)]
    pub temperature: Option<bool>,
    /// Whether the model accepts file attachments.
    #[serde(default)]
    pub attachment: Option<bool>,
    /// Whether the model's weights are publicly available.
    #[serde(default)]
    pub open_weights: Option<bool>,
    /// Whether streaming/interleaved output is supported (free-form in
    /// models.dev, so kept as raw JSON).
    #[serde(default)]
    pub interleaved: Option<serde_json::Value>,
    /// Knowledge cutoff, if advertised.
    #[serde(default)]
    pub knowledge: Option<String>,
    /// Release date of the model.
    #[serde(default)]
    pub release_date: Option<String>,
    /// When the models.dev entry was last updated.
    #[serde(default)]
    pub last_updated: Option<String>,
}

/// Supported input and output types for a model (e.g. text, image).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelModalities {
    /// Modalities the model accepts as input (e.g. `"text"`, `"image"`).
    #[serde(default)]
    pub input: Vec<String>,
    /// Modalities the model can produce as output.
    #[serde(default)]
    pub output: Vec<String>,
}

/// Context window and output token limits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelLimit {
    /// Maximum context window size in tokens.
    #[serde(default)]
    pub context: Option<u64>,
    /// Maximum output length in tokens.
    #[serde(default)]
    pub output: Option<u64>,
}

/// Pricing per token (input, output, cache tiers).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCost {
    /// Price per million input tokens.
    #[serde(default)]
    pub input: Option<f64>,
    /// Price per million output tokens.
    #[serde(default)]
    pub output: Option<f64>,
    /// Price per million tokens read from cache.
    #[serde(default)]
    pub cache_read: Option<f64>,
    /// Price per million tokens written to cache.
    #[serde(default)]
    pub cache_write: Option<f64>,
}

/// A reasoning budget option available for this model.
///
/// models.dev exposes three option types: `"toggle"` (boolean on/off, no
/// parameters), `"effort"` (string selector via `values`), and
/// `"budget_tokens"` (numeric range via `min`/`max`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningOption {
    /// Option kind: `"toggle"`, `"effort"`, or `"budget_tokens"`.
    #[serde(rename = "type")]
    pub option_type: String,
    /// Allowed string values, for `"effort"` options.
    #[serde(default, deserialize_with = "de_option_values")]
    pub values: Option<Vec<String>>,
    /// Minimum budget tokens, for `"budget_tokens"` options.
    #[serde(default)]
    pub min: Option<i64>,
    /// Maximum budget tokens, for `"budget_tokens"` options.
    #[serde(default)]
    pub max: Option<i64>,
}

/// Tolerantly deserializes `values`, dropping `null` elements.
///
/// models.dev emits `null` inside `values` for a couple of models
/// (e.g. `sarvam/sarvam-30b`), which would otherwise fail strict
/// `Vec<String>` deserialization and break the whole models.dev load.
fn de_option_values<'de, D>(deserializer: D) -> Result<Option<Vec<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let values: Option<Vec<Option<String>>> = Option::deserialize(deserializer)?;
    Ok(values.map(|v| v.into_iter().flatten().collect()))
}
