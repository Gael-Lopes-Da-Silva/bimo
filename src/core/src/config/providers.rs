use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub name: String,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvidersFile {
    pub providers: Vec<ProviderConfig>,
    #[serde(default)]
    pub default: Option<String>,
}

impl ProvidersFile {
    pub fn default_path() -> std::path::PathBuf {
        Self::bimo_dir().join("providers.json")
    }

    pub fn bimo_dir() -> std::path::PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("~/.config"))
            .join("bimo")
    }

    pub fn load() -> crate::Result<Self> {
        let path = Self::default_path();
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
        let path = Self::default_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    pub fn default_provider(&self) -> Option<&ProviderConfig> {
        let default_name = self.default.as_deref()?;
        self.providers.iter().find(|p| p.name == default_name)
    }
}
