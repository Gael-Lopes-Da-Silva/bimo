//! Session and agent settings — types and persistence for `settings.json`.

use serde::{Deserialize, Serialize};

/// Application settings persisted to `~/.config/bimo/settings.json`.
///
/// Controls session lifecycle, agent defaults, and cleanup behaviour.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// Sessions older than this many hours are eligible for cleanup.
    #[serde(default = "default_session_ttl_hours")]
    pub session_ttl_hours: u64,

    /// Maximum number of sessions kept on disk before the oldest are removed.
    #[serde(default = "default_max_sessions")]
    pub max_sessions: usize,

    /// How often (in minutes) the background cleanup task runs.
    #[serde(default = "default_cleanup_interval_minutes")]
    pub cleanup_interval_minutes: u64,

    /// Maximum tool-call steps per agent run.
    #[serde(default = "default_max_steps")]
    pub max_steps: usize,

    /// Default provider id used when none is specified at build time.
    #[serde(default)]
    pub default_provider: Option<String>,

    /// Default model id used when none is specified at build time.
    #[serde(default)]
    pub default_model: Option<String>,
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
        }
    }
}

impl Settings {
    /// Returns the path to `settings.json` inside `~/.config/bimo/`.
    pub fn path() -> std::path::PathBuf {
        let base = dirs::config_dir().unwrap_or_else(|| std::path::PathBuf::from("~/.config"));
        base.join("bimo").join("settings.json")
    }

    /// Loads settings from disk, writing defaults if the file does not exist.
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

    /// Saves settings to `settings.json`, creating parent directories as needed.
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
