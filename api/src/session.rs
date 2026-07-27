use crate::error::{BimoError, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

/// The role of a message participant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
}

/// A single message in a conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
    pub timestamp: DateTime<Utc>,
}

/// A conversation session that maintains context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub messages: Vec<Message>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Summary info about a saved session (without loading all messages).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: String,
    pub message_count: usize,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Session {
    /// Create a brand-new session.
    pub fn new() -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            messages: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Add a user message to the session.
    pub fn add_user_message(&mut self, content: &str) {
        self.messages.push(Message {
            role: Role::User,
            content: content.to_string(),
            timestamp: Utc::now(),
        });
        self.updated_at = Utc::now();
    }

    /// Add an assistant message to the session.
    pub fn add_assistant_message(&mut self, content: &str) {
        self.messages.push(Message {
            role: Role::Assistant,
            content: content.to_string(),
            timestamp: Utc::now(),
        });
        self.updated_at = Utc::now();
    }

    /// Add a system message to the session.
    pub fn add_system_message(&mut self, content: &str) {
        self.messages.push(Message {
            role: Role::System,
            content: content.to_string(),
            timestamp: Utc::now(),
        });
        self.updated_at = Utc::now();
    }

    /// Return messages formatted for the provider API.
    pub fn to_chat_messages(&self) -> Vec<crate::provider::ChatMessage> {
        self.messages
            .iter()
            .map(|m| crate::provider::ChatMessage {
                role: match m.role {
                    Role::System => "system".into(),
                    Role::User => "user".into(),
                    Role::Assistant => "assistant".into(),
                },
                content: m.content.clone(),
            })
            .collect()
    }

    /// Clear all messages, resetting the session.
    pub fn clear(&mut self) {
        self.messages.clear();
        self.updated_at = Utc::now();
    }

    /// Number of messages in the session.
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    /// Return a summary info struct.
    pub fn info(&self) -> SessionInfo {
        SessionInfo {
            id: self.id.clone(),
            message_count: self.messages.len(),
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }

    // -------------------------------------------------------------------
    // Persistence
    // -------------------------------------------------------------------

    /// Returns the sessions directory (`~/.bimo/sessions/`).
    fn sessions_dir() -> Result<PathBuf> {
        let home = dirs::home_dir()
            .ok_or_else(|| BimoError::Session("cannot determine home directory".into()))?;
        let dir = home.join(".bimo").join("sessions");
        fs::create_dir_all(&dir)
            .map_err(|e| BimoError::Session(format!("failed to create sessions dir: {e}")))?;
        Ok(dir)
    }

    /// Path to this session's JSON file.
    fn session_path(&self) -> Result<PathBuf> {
        Ok(Self::sessions_dir()?.join(format!("{}.json", self.id)))
    }

    /// Save the session to disk.
    pub fn save(&self) -> Result<()> {
        let path = self.session_path()?;
        let data = serde_json::to_string_pretty(self)
            .map_err(|e| BimoError::Session(format!("failed to serialize session: {e}")))?;
        fs::write(&path, data)
            .map_err(|e| BimoError::Session(format!("failed to write session file: {e}")))?;
        Ok(())
    }

    /// Load a session from disk by id.
    pub fn load(id: &str) -> Result<Self> {
        let path = Self::sessions_dir()?.join(format!("{id}.json"));
        if !path.exists() {
            return Err(BimoError::Session(format!("session '{id}' not found")));
        }
        let data = fs::read_to_string(&path)
            .map_err(|e| BimoError::Session(format!("failed to read session file: {e}")))?;
        let session: Session = serde_json::from_str(&data)
            .map_err(|e| BimoError::Session(format!("failed to parse session file: {e}")))?;
        Ok(session)
    }

    /// List all saved sessions (returns info summaries, newest first).
    pub fn list_saved() -> Result<Vec<SessionInfo>> {
        let dir = Self::sessions_dir()?;
        let mut sessions = Vec::new();

        let entries = fs::read_dir(&dir)
            .map_err(|e| BimoError::Session(format!("failed to read sessions dir: {e}")))?;

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                if let Ok(data) = fs::read_to_string(&path) {
                    if let Ok(session) = serde_json::from_str::<Session>(&data) {
                        sessions.push(session.info());
                    }
                }
            }
        }

        sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(sessions)
    }

    /// Delete a saved session from disk.
    pub fn delete_saved(id: &str) -> Result<()> {
        let path = Self::sessions_dir()?.join(format!("{id}.json"));
        if !path.exists() {
            return Err(BimoError::Session(format!("session '{id}' not found")));
        }
        fs::remove_file(&path)
            .map_err(|e| BimoError::Session(format!("failed to delete session file: {e}")))?;
        Ok(())
    }

    /// Delete all saved sessions from disk.
    pub fn delete_all_saved() -> Result<()> {
        let dir = Self::sessions_dir()?;
        let entries = fs::read_dir(&dir)
            .map_err(|e| BimoError::Session(format!("failed to read sessions dir: {e}")))?;

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                let _ = fs::remove_file(&path);
            }
        }
        Ok(())
    }

    /// Create a new session forked from this one, keeping only messages up to (and
    /// including) the given index. The new session is saved and returned.
    pub fn fork(&self, index: usize) -> Result<Self> {
        if index >= self.messages.len() {
            return Err(BimoError::Session(format!(
                "index {} out of range (session has {} messages)",
                index,
                self.messages.len()
            )));
        }
        let mut forked = Self::new();
        forked.messages = self.messages[..=index].to_vec();
        forked.updated_at = Utc::now();
        forked.save()?;
        Ok(forked)
    }

    /// Revert the session by discarding all messages after the given index.
    pub fn revert(&mut self, index: usize) -> Result<()> {
        if index >= self.messages.len() {
            return Err(BimoError::Session(format!(
                "index {} out of range (session has {} messages)",
                index,
                self.messages.len()
            )));
        }
        self.messages.truncate(index + 1);
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Compact the session by replacing the conversation with a summary.
    /// This keeps the first system message (if any) and replaces everything
    /// else with a single system message containing the summary.
    pub fn compact(&mut self, summary: &str) {
        let system_messages: Vec<Message> = self
            .messages
            .iter()
            .filter(|m| m.role == Role::System)
            .cloned()
            .collect();

        self.messages = system_messages;

        self.messages.push(Message {
            role: Role::System,
            content: format!(
                "Previous conversation summary:\n{summary}"
            ),
            timestamp: Utc::now(),
        });

        self.updated_at = Utc::now();
    }
}
