//! Session model — messages, persistence, and lifecycle management.

mod manager;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::tools::TodoList;

/// A single message in a session conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
    pub timestamp: DateTime<Utc>,
}

/// An agent session — persists conversation history and todo state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub messages: Vec<Message>,
    pub todo_list: TodoList,
    pub metadata: serde_json::Value,
}

impl Session {
    /// Creates a new session with a random id.
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            messages: Vec::new(),
            todo_list: TodoList::new(),
            metadata: serde_json::json!({}),
        }
    }

    /// Adds a message and updates the timestamp.
    pub fn add_message(&mut self, role: String, content: String) {
        self.messages.push(Message {
            role,
            content,
            timestamp: Utc::now(),
        });
        self.updated_at = Utc::now();
    }

    /// Returns `true` if the session has not been updated within `ttl_hours`.
    pub fn is_expired(&self, ttl_hours: u64) -> bool {
        let elapsed = Utc::now() - self.updated_at;
        elapsed.num_hours() > ttl_hours as i64
    }

    /// Returns the directory where session files are stored.
    pub fn sessions_dir() -> std::path::PathBuf {
        let base = dirs::config_dir().unwrap_or_else(|| std::path::PathBuf::from("~/.config"));
        base.join("bimo").join("sessions")
    }

    /// Returns the filesystem path for this session.
    pub fn path(&self) -> std::path::PathBuf {
        Self::sessions_dir().join(format!("{}.json", self.id))
    }

    /// Persists this session to disk.
    pub fn save(&self) -> crate::Result<()> {
        let path = self.path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    /// Loads a session by id from disk.
    pub fn load(id: &str) -> crate::Result<Self> {
        let path = Self::sessions_dir().join(format!("{id}.json"));
        let content = std::fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&content)?)
    }

    /// Deletes the session file from disk.
    pub fn delete(&self) -> crate::Result<()> {
        let path = self.path();
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        Ok(())
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

pub use manager::SessionManager;
