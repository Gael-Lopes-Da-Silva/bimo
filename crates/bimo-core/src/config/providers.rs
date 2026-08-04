use serde::{Deserialize, Serialize};

/// Whether a provider runs locally or is a cloud service.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderType {
    Local,
    Cloud,
}

/// API format expected by a provider's endpoint.
///
/// Maps from the `npm` field in models.dev entries (e.g. `@ai-sdk/anthropic`)
/// or the user's `api_format` override in `providers.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiFormat {
    OpenAICompatible,
    Anthropic,
    OpenAI,
    Google,
    Other(String),
}

/// A configured provider (local or cloud).
///
/// Serialized as an entry in `providers.json`. For cloud providers the user
/// supplies the `api_key`; the `base_url` may be filled automatically from
/// the models.dev registry if left empty.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provider {
    /// Unique identifier used to reference this provider (e.g. `"ollama"`).
    pub id: String,
    /// Human-readable display name (e.g. `"Ollama"`).
    pub name: String,
    /// Base URL of the provider's API endpoint.
    pub base_url: String,
    /// Optional API key. Omitted from serialization when unset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    /// Whether this is a local or cloud service (serialized as `"type"`).
    #[serde(rename = "type")]
    pub provider_type: ProviderType,
    /// Model ids served by this provider, if known.
    #[serde(default)]
    pub models: Vec<String>,
    /// Wire format the provider's endpoint expects.
    pub api_format: ApiFormat,
}

impl Provider {
    /// Creates a local provider with the given identity and base URL.
    pub fn local(id: &str, name: &str, base_url: &str, api_format: ApiFormat) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            base_url: base_url.to_string(),
            api_key: None,
            provider_type: ProviderType::Local,
            models: Vec::new(),
            api_format,
        }
    }

    /// Creates a cloud provider with the given identity and base URL.
    pub fn cloud(id: &str, name: &str, base_url: &str, api_format: ApiFormat) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            base_url: base_url.to_string(),
            api_key: None,
            provider_type: ProviderType::Cloud,
            models: Vec::new(),
            api_format,
        }
    }

    /// Returns `true` if this provider runs locally.
    pub fn is_local(&self) -> bool {
        matches!(self.provider_type, ProviderType::Local)
    }

    /// Returns `true` if this provider is a cloud service.
    pub fn is_cloud(&self) -> bool {
        matches!(self.provider_type, ProviderType::Cloud)
    }
}

/// The user's `providers.json` file.
///
/// Lists configured providers and optionally names a default.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvidersConfig {
    #[serde(default)]
    pub providers: Vec<Provider>,
    #[serde(default)]
    pub default: Option<String>,
}

impl ProvidersConfig {
    /// Returns the path to `providers.json` inside the config directory.
    pub fn path() -> std::path::PathBuf {
        crate::paths::config_dir().join("providers.json")
    }

    /// Loads providers from disk, returning an empty file if the path does not exist.
    pub fn load() -> crate::Result<Self> {
        let path = Self::path();
        if !path.exists() {
            return Ok(Self {
                providers: Vec::new(),
                default: None,
            });
        }
        let content = std::fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&content)?)
    }

    /// Saves providers to `providers.json`, creating parent directories as needed.
    pub fn save(&self) -> crate::Result<()> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    /// Returns the provider marked as default, if any.
    pub fn default_provider(&self) -> Option<&Provider> {
        let default_id = self.default.as_deref()?;
        self.providers.iter().find(|p| p.id == default_id)
    }

    /// Returns all local providers in the configuration.
    pub fn local_providers(&self) -> Vec<&Provider> {
        self.providers.iter().filter(|p| p.is_local()).collect()
    }

    /// Returns all cloud providers in the configuration.
    pub fn cloud_providers(&self) -> Vec<&Provider> {
        self.providers.iter().filter(|p| p.is_cloud()).collect()
    }

    /// Looks up a provider by id or name (case-insensitive).
    pub fn find(&self, id_or_name: &str) -> Option<&Provider> {
        let lower = id_or_name.to_lowercase();
        self.providers
            .iter()
            .find(|p| p.id.to_lowercase() == lower || p.name.to_lowercase() == lower)
    }
}
