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

    /// Enables debug logging and event persistence.
    #[serde(default)]
    pub debug: bool,

    /// Maximum retry attempts for failed agent steps.
    #[serde(default = "default_retry_attempts")]
    pub retry_attempts: usize,

    /// Timeout in seconds before retrying a failed step.
    #[serde(default = "default_retry_timeout_secs")]
    pub retry_timeout_secs: u64,

    /// Captures git-backed filesystem snapshots before agent runs so file
    /// changes can be reverted when undoing a prompt. Only works in git
    /// repositories. When disabled, undoing still rewinds the conversation
    /// but cannot restore modified files.
    #[serde(default = "default_snapshots")]
    pub snapshots: bool,
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

fn default_retry_attempts() -> usize {
    10
}

fn default_retry_timeout_secs() -> u64 {
    5
}

fn default_snapshots() -> bool {
    true
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
            debug: false,
            retry_attempts: default_retry_attempts(),
            retry_timeout_secs: default_retry_timeout_secs(),
            snapshots: default_snapshots(),
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
