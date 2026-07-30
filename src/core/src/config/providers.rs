use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderType {
    Local,
    Cloud,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiFormat {
    OpenAICompatible,
    Anthropic,
    OpenAI,
    Google,
    Other(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub base_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(rename = "type")]
    pub provider_type: ProviderType,
    #[serde(default)]
    pub models: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_format: Option<ApiFormat>,
}

impl Provider {
    pub fn local(id: &str, name: &str, base_url: &str) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            base_url: base_url.to_string(),
            api_key: None,
            provider_type: ProviderType::Local,
            models: Vec::new(),
            api_format: None,
        }
    }

    pub fn cloud(id: &str, name: &str, base_url: &str) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            base_url: base_url.to_string(),
            api_key: None,
            provider_type: ProviderType::Cloud,
            models: Vec::new(),
            api_format: None,
        }
    }

    pub fn is_local(&self) -> bool {
        matches!(self.provider_type, ProviderType::Local)
    }

    pub fn is_cloud(&self) -> bool {
        matches!(self.provider_type, ProviderType::Cloud)
    }

    pub fn effective_api_format(&self) -> ApiFormat {
        self.api_format
            .clone()
            .unwrap_or(ApiFormat::OpenAICompatible)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvidersFile {
    #[serde(default)]
    pub providers: Vec<Provider>,
    #[serde(default)]
    pub default: Option<String>,
}

impl ProvidersFile {
    pub fn path() -> std::path::PathBuf {
        let base = dirs::config_dir().unwrap_or_else(|| std::path::PathBuf::from("~/.config"));
        base.join("bimo").join("providers.json")
    }

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

    pub fn save(&self) -> crate::Result<()> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    pub fn default_provider(&self) -> Option<&Provider> {
        let default_id = self.default.as_deref()?;
        self.providers.iter().find(|p| p.id == default_id)
    }

    pub fn local_providers(&self) -> Vec<&Provider> {
        self.providers.iter().filter(|p| p.is_local()).collect()
    }

    pub fn cloud_providers(&self) -> Vec<&Provider> {
        self.providers.iter().filter(|p| p.is_cloud()).collect()
    }

    pub fn find(&self, id_or_name: &str) -> Option<&Provider> {
        let lower = id_or_name.to_lowercase();
        self.providers
            .iter()
            .find(|p| p.id.to_lowercase() == lower || p.name.to_lowercase() == lower)
    }
}
