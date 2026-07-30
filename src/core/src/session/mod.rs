pub mod persistence;

use crate::todo::TodoList;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimated_tokens: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub messages: Vec<Message>,
    pub todos: TodoList,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default, skip)]
    pub dirty: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: String,
    pub message_count: usize,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Session {
    pub fn new() -> Self {
        let now = Utc::now();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            messages: Vec::new(),
            todos: TodoList::new(),
            created_at: now,
            updated_at: now,
            dirty: false,
        }
    }

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
        self.dirty = true;
    }

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
        self.dirty = true;
    }

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
        self.dirty = true;
    }

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
        self.dirty = true;
    }

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
        self.dirty = true;
    }

    pub fn to_chat_messages(&self) -> Vec<crate::provider::ChatMessage> {
        self.messages.iter().map(|m| {
            let role = match m.role {
                Role::System => "system",
                Role::User => "user",
                Role::Assistant => "assistant",
                Role::Tool => "user",
            };
            crate::provider::ChatMessage {
                role: role.into(),
                content: m.content.clone(),
            }
        }).collect()
    }

    pub fn clear(&mut self) {
        self.messages.clear();
        self.updated_at = Utc::now();
        self.dirty = true;
    }

    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    pub fn info(&self) -> SessionInfo {
        SessionInfo {
            id: self.id.clone(),
            message_count: self.messages.len(),
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }

    pub fn fork(&self, index: usize) -> crate::error::Result<Self> {
        if index >= self.messages.len() {
            return Err(crate::error::BimoError::Session(format!(
                "index {} out of range (session has {} messages)", index, self.messages.len()
            )));
        }
        let mut forked = Self::new();
        forked.messages = self.messages[..=index].to_vec();
        forked.updated_at = Utc::now();
        forked.save()?;
        Ok(forked)
    }

    pub fn revert(&mut self, index: usize) -> crate::error::Result<()> {
        if index >= self.messages.len() {
            return Err(crate::error::BimoError::Session(format!(
                "index {} out of range (session has {} messages)", index, self.messages.len()
            )));
        }
        self.messages.truncate(index + 1);
        self.updated_at = Utc::now();
        self.dirty = true;
        Ok(())
    }

    pub fn compact(&mut self, summary: &str) {
        let system_messages: Vec<Message> = self.messages.iter()
            .filter(|m| m.role == Role::System)
            .cloned()
            .collect();
        self.messages = system_messages;
        self.messages.push(Message {
            role: Role::System,
            content: format!("Previous conversation summary:\n{}", summary),
            timestamp: Utc::now(),
            model: None,
            provider: None,
            estimated_tokens: None,
        });
        self.updated_at = Utc::now();
        self.dirty = true;
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}
