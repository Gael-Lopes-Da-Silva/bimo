use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalProvider {
    pub name: String,
    pub base_url: String,
    #[serde(default)]
    pub models: Vec<String>,
}

impl LocalProvider {
    pub fn ollama() -> Self {
        Self {
            name: "ollama".to_string(),
            base_url: "http://localhost:11434/v1".to_string(),
            models: vec![],
        }
    }

    pub fn lmstudio() -> Self {
        Self {
            name: "lmstudio".to_string(),
            base_url: "http://localhost:1234/v1".to_string(),
            models: vec![],
        }
    }
}

fn default_local_providers() -> Vec<LocalProvider> {
    vec![LocalProvider::ollama(), LocalProvider::lmstudio()]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default = "default_session_ttl_hours")]
    pub session_ttl_hours: u64,

    #[serde(default = "default_max_sessions")]
    pub max_sessions: usize,

    #[serde(default = "default_cleanup_interval_minutes")]
    pub cleanup_interval_minutes: u64,

    #[serde(default = "default_max_steps")]
    pub max_steps: usize,

    #[serde(default)]
    pub default_provider: Option<String>,

    #[serde(default)]
    pub default_model: Option<String>,

    #[serde(default = "default_local_providers")]
    pub local_providers: Vec<LocalProvider>,
}

fn default_session_ttl_hours() -> u64 {
    24
}

fn default_max_sessions() -> usize {
    50
}

fn default_cleanup_interval_minutes() -> u64 {
    30
}

fn default_max_steps() -> usize {
    25
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            session_ttl_hours: default_session_ttl_hours(),
            max_sessions: default_max_sessions(),
            cleanup_interval_minutes: default_cleanup_interval_minutes(),
            max_steps: default_max_steps(),
            default_provider: None,
            default_model: None,
            local_providers: default_local_providers(),
        }
    }
}

impl Settings {
    pub fn path() -> std::path::PathBuf {
        let base = dirs::config_dir().unwrap_or_else(|| std::path::PathBuf::from("~/.config"));
        base.join("bimo").join("settings.json")
    }

    pub fn load() -> crate::Result<Self> {
        let path = Self::path();
        if !path.exists() {
            let settings = Settings::default();
            settings.save()?;
            return Ok(settings);
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
}
