pub mod manager;
pub mod persistence;

use crate::prompts;
use crate::todo::TodoList;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Contextual metadata about the project/environment.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct SessionContext {
    /// The current working directory.
    #[serde(default)]
    pub cwd: String,
    /// The active git branch, if any.
    #[serde(default)]
    pub git_branch: Option<String>,
    /// Filenames of agent instruction files that were found and loaded
    /// (e.g. "AGENTS.md", "CLAUDE.md").
    #[serde(default)]
    pub agent_instructions: Vec<String>,
    /// Loaded agent skills.
    #[serde(default)]
    pub skills: Vec<crate::skill::Skill>,
    /// Available prompt templates.
    #[serde(default)]
    pub prompts: Vec<crate::command::prompt::PromptTemplate>,
}

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
    /// The model that generated this message (assistant messages only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// The provider that generated this message (assistant messages only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    /// Estimated token count for this message's content.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimated_tokens: Option<usize>,
}

/// A conversation session that maintains context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub messages: Vec<Message>,
    pub todos: TodoList,
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
            id: uuid::Uuid::new_v4().to_string(),
            messages: Vec::new(),
            todos: TodoList::new(),
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
            model: None,
            provider: None,
            estimated_tokens: None,
        });
        self.updated_at = Utc::now();
    }

    /// Add a user message with an estimated token count.
    pub fn add_user_message_with_tokens(&mut self, content: &str, estimated_tokens: Option<usize>) {
        self.messages.push(Message {
            role: Role::User,
            content: content.to_string(),
            timestamp: Utc::now(),
            model: None,
            provider: None,
            estimated_tokens,
        });
        self.updated_at = Utc::now();
    }

    /// Add an assistant message to the session.
    pub fn add_assistant_message(&mut self, content: &str) {
        self.messages.push(Message {
            role: Role::Assistant,
            content: content.to_string(),
            timestamp: Utc::now(),
            model: None,
            provider: None,
            estimated_tokens: None,
        });
        self.updated_at = Utc::now();
    }

    /// Add an assistant message with model/provider metadata and token estimate.
    pub fn add_assistant_response(
        &mut self,
        content: &str,
        model: Option<String>,
        provider: Option<String>,
        estimated_tokens: Option<usize>,
    ) {
        self.messages.push(Message {
            role: Role::Assistant,
            content: content.to_string(),
            timestamp: Utc::now(),
            model,
            provider,
            estimated_tokens,
        });
        self.updated_at = Utc::now();
    }

    /// Add a system message to the session.
    pub fn add_system_message(&mut self, content: &str) {
        self.messages.push(Message {
            role: Role::System,
            content: content.to_string(),
            timestamp: Utc::now(),
            model: None,
            provider: None,
            estimated_tokens: None,
        });
        self.updated_at = Utc::now();
    }

    /// Add a tool result message to the session.
    pub fn add_tool_message(&mut self, content: &str) {
        self.messages.push(Message {
            role: Role::Tool,
            content: content.to_string(),
            timestamp: Utc::now(),
            model: None,
            provider: None,
            estimated_tokens: None,
        });
        self.updated_at = Utc::now();
    }

    /// Add a tool result message with estimated tokens.
    pub fn add_tool_message_with_tokens(&mut self, content: &str, estimated_tokens: Option<usize>) {
        self.messages.push(Message {
            role: Role::Tool,
            content: content.to_string(),
            timestamp: Utc::now(),
            model: None,
            provider: None,
            estimated_tokens,
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

    /// Create a new session forked from this one, keeping only messages up to (and
    /// including) the given index. The new session is saved and returned.
    pub fn fork(&self, index: usize) -> crate::error::Result<Self> {
        use crate::error::BimoError;
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
    pub fn revert(&mut self, index: usize) -> crate::error::Result<()> {
        use crate::error::BimoError;
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
            model: None,
            provider: None,
            estimated_tokens: None,
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

    #[test]
    fn delete_saved_removes_session_from_disk() {
        let mut session = Session::new();
        session.add_user_message("to be deleted");
        session.save().unwrap();

        let id = session.id.clone();
        assert!(Session::load(&id).is_ok());

        Session::delete_saved(&id).unwrap();
        assert!(Session::load(&id).is_err());
    }

    #[test]
    fn delete_saved_unknown_id_returns_error() {
        let result = Session::delete_saved("nonexistent-session-id");
        assert!(result.is_err());
    }

    #[test]
    fn delete_saved_only_removes_target_session() {
        let mut s1 = Session::new();
        s1.add_user_message("first");
        s1.save().unwrap();

        let mut s2 = Session::new();
        s2.add_user_message("second");
        s2.save().unwrap();

        Session::delete_saved(&s1.id).unwrap();

        assert!(Session::load(&s1.id).is_err());
        assert!(Session::load(&s2.id).is_ok());

        // cleanup
        let _ = Session::delete_saved(&s2.id);
    }
}
