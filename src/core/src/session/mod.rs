mod manager;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::tools::TodoList;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
    pub timestamp: DateTime<Utc>,
}

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

    pub fn add_message(&mut self, role: String, content: String) {
        self.messages.push(Message {
            role,
            content,
            timestamp: Utc::now(),
        });
        self.updated_at = Utc::now();
    }

    pub fn is_expired(&self, ttl_hours: u64) -> bool {
        let elapsed = Utc::now() - self.updated_at;
        elapsed.num_hours() > ttl_hours as i64
    }

    pub fn sessions_dir() -> std::path::PathBuf {
        let base = dirs::config_dir().unwrap_or_else(|| std::path::PathBuf::from("~/.config"));
        base.join("bimo").join("sessions")
    }

    pub fn path(&self) -> std::path::PathBuf {
        Self::sessions_dir().join(format!("{}.json", self.id))
    }

    pub fn save(&self) -> crate::Result<()> {
        let path = self.path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    pub fn load(id: &str) -> crate::Result<Self> {
        let path = Self::sessions_dir().join(format!("{id}.json"));
        let content = std::fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&content)?)
    }

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
