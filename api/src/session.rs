use crate::error::{BimoError, Result};
use crate::prompts;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tracing;
use uuid::Uuid;

/// The role of a message participant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
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

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
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

    /// Add a tool result message to the session.
    pub fn add_tool_message(&mut self, content: &str) {
        self.messages.push(Message {
            role: Role::Tool,
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
                    Role::Tool => "user".into(),
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
        tracing::debug!(session_id = %self.id, path = %path.display(), "saving session");
        let data = serde_json::to_string_pretty(self)
            .map_err(|e| BimoError::Session(format!("failed to serialize session: {e}")))?;
        fs::write(&path, data)
            .map_err(|e| BimoError::Session(format!("failed to write session file: {e}")))?;
        tracing::debug!(session_id = %self.id, "session saved");
        Ok(())
    }

    /// Load a session from disk by id.
    pub fn load(id: &str) -> Result<Self> {
        let path = Self::sessions_dir()?.join(format!("{id}.json"));
        tracing::debug!(session_id = id, path = %path.display(), "loading session");
        if !path.exists() {
            tracing::warn!(session_id = id, "session file not found");
            return Err(BimoError::Session(format!("session '{id}' not found")));
        }
        let data = fs::read_to_string(&path)
            .map_err(|e| BimoError::Session(format!("failed to read session file: {e}")))?;
        let session: Session = serde_json::from_str(&data)
            .map_err(|e| BimoError::Session(format!("failed to parse session file: {e}")))?;
        tracing::debug!(
            session_id = id,
            message_count = session.message_count(),
            "session loaded"
        );
        Ok(session)
    }

    /// List all saved sessions (returns info summaries, newest first).
    pub fn list_saved() -> Result<Vec<SessionInfo>> {
        let dir = Self::sessions_dir()?;
        tracing::debug!(dir = %dir.display(), "listing saved sessions");
        let mut sessions = Vec::new();

        let entries = fs::read_dir(&dir)
            .map_err(|e| BimoError::Session(format!("failed to read sessions dir: {e}")))?;

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json")
                && let Ok(data) = fs::read_to_string(&path)
                && let Ok(session) = serde_json::from_str::<Session>(&data)
            {
                sessions.push(session.info());
            }
        }

        sessions.sort_by_key(|b| std::cmp::Reverse(b.updated_at));
        tracing::debug!(count = sessions.len(), "listed saved sessions");
        Ok(sessions)
    }

    /// Delete a saved session from disk.
    pub fn delete_saved(id: &str) -> Result<()> {
        let path = Self::sessions_dir()?.join(format!("{id}.json"));
        tracing::info!(session_id = id, "deleting saved session");
        if !path.exists() {
            tracing::warn!(session_id = id, "session file not found for deletion");
            return Err(BimoError::Session(format!("session '{id}' not found")));
        }
        fs::remove_file(&path)
            .map_err(|e| BimoError::Session(format!("failed to delete session file: {e}")))?;
        tracing::info!(session_id = id, "session deleted");
        Ok(())
    }

    /// Delete all saved sessions from disk.
    pub fn delete_all_saved() -> Result<()> {
        tracing::info!("deleting all saved sessions");
        let dir = Self::sessions_dir()?;
        let entries = fs::read_dir(&dir)
            .map_err(|e| BimoError::Session(format!("failed to read sessions dir: {e}")))?;

        let mut count = 0;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                let _ = fs::remove_file(&path);
                count += 1;
            }
        }
        tracing::info!(count, "all saved sessions purged");
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
            content: prompts::render(
                &prompts::load(prompts::COMPACT_PREFIX),
                &[("SUMMARY", summary)],
            ),
            timestamp: Utc::now(),
        });

        self.updated_at = Utc::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_session_has_unique_id() {
        let s1 = Session::new();
        let s2 = Session::new();
        assert_ne!(s1.id, s2.id);
        assert!(!s1.id.is_empty());
    }

    #[test]
    fn new_session_is_empty() {
        let session = Session::new();
        assert_eq!(session.message_count(), 0);
        assert!(session.messages.is_empty());
    }

    #[test]
    fn add_user_message() {
        let mut session = Session::new();
        session.add_user_message("hello");
        assert_eq!(session.message_count(), 1);
        assert_eq!(session.messages[0].role, Role::User);
        assert_eq!(session.messages[0].content, "hello");
    }

    #[test]
    fn add_assistant_message() {
        let mut session = Session::new();
        session.add_assistant_message("hi there");
        assert_eq!(session.message_count(), 1);
        assert_eq!(session.messages[0].role, Role::Assistant);
        assert_eq!(session.messages[0].content, "hi there");
    }

    #[test]
    fn add_system_message() {
        let mut session = Session::new();
        session.add_system_message("system prompt");
        assert_eq!(session.message_count(), 1);
        assert_eq!(session.messages[0].role, Role::System);
    }

    #[test]
    fn to_chat_messages_conversion() {
        let mut session = Session::new();
        session.add_system_message("sys");
        session.add_user_message("q");
        session.add_assistant_message("a");
        let msgs = session.to_chat_messages();
        assert_eq!(msgs.len(), 3);
        assert_eq!(msgs[0].role, "system");
        assert_eq!(msgs[1].role, "user");
        assert_eq!(msgs[2].role, "assistant");
    }

    #[test]
    fn clear_session() {
        let mut session = Session::new();
        session.add_user_message("msg");
        assert_eq!(session.message_count(), 1);
        session.clear();
        assert_eq!(session.message_count(), 0);
    }

    #[test]
    fn message_count() {
        let mut session = Session::new();
        assert_eq!(session.message_count(), 0);
        session.add_user_message("a");
        session.add_assistant_message("b");
        assert_eq!(session.message_count(), 2);
    }

    #[test]
    fn info_struct() {
        let mut session = Session::new();
        session.add_user_message("test");
        let info = session.info();
        assert_eq!(info.id, session.id);
        assert_eq!(info.message_count, 1);
    }

    #[test]
    fn fork_session() {
        let mut session = Session::new();
        session.add_user_message("a");
        session.add_user_message("b");
        session.add_user_message("c");

        let forked = session.fork(1).unwrap();
        assert_eq!(forked.message_count(), 2);
        assert_eq!(forked.messages[0].content, "a");
        assert_eq!(forked.messages[1].content, "b");
        // Original unchanged
        assert_eq!(session.message_count(), 3);
    }

    #[test]
    fn fork_out_of_range() {
        let mut session = Session::new();
        session.add_user_message("a");
        assert!(session.fork(5).is_err());
    }

    #[test]
    fn revert_session() {
        let mut session = Session::new();
        session.add_user_message("a");
        session.add_user_message("b");
        session.add_user_message("c");

        session.revert(1).unwrap();
        assert_eq!(session.message_count(), 2);
        assert_eq!(session.messages[1].content, "b");
    }

    #[test]
    fn revert_out_of_range() {
        let mut session = Session::new();
        session.add_user_message("a");
        assert!(session.revert(5).is_err());
    }

    #[test]
    fn compact_session() {
        let mut session = Session::new();
        session.add_system_message("sys prompt");
        session.add_user_message("hello");
        session.add_assistant_message("hi");

        session.compact("conversation summary");

        // Should keep original system message + add summary system message
        assert_eq!(session.message_count(), 2);
        assert_eq!(session.messages[0].role, Role::System);
        assert_eq!(session.messages[0].content, "sys prompt");
        assert_eq!(session.messages[1].role, Role::System);
        assert!(session.messages[1].content.contains("conversation summary"));
    }

    #[test]
    fn compact_without_system_message() {
        let mut session = Session::new();
        session.add_user_message("hello");
        session.add_assistant_message("hi");

        session.compact("summary");

        assert_eq!(session.message_count(), 1);
        assert_eq!(session.messages[0].role, Role::System);
        assert!(session.messages[0].content.contains("summary"));
    }

    #[test]
    fn session_is_serializable() {
        let mut session = Session::new();
        session.add_user_message("test message");
        let json = serde_json::to_string(&session).unwrap();
        let deserialized: Session = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, session.id);
        assert_eq!(deserialized.message_count(), 1);
        assert_eq!(deserialized.messages[0].content, "test message");
    }

    #[test]
    fn session_info_is_serializable() {
        let session = Session::new();
        let info = session.info();
        let json = serde_json::to_string(&info).unwrap();
        let deserialized: SessionInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, info.id);
    }
}
