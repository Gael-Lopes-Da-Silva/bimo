use bimo_core::{Session, session::SessionManager};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct SessionItem {
    pub id: String,
    pub name: String,
    pub model: String,
    pub updated: String,
    pub message_count: usize,
    pub is_active: bool,
}

#[derive(Debug, Clone)]
pub struct SessionState {
    pub sessions: Vec<SessionItem>,
    pub current_session: Option<Session>,
    pub selected_index: Option<usize>,
    pub filter: String,
    pub sort_by: SessionSort,
    pub sort_descending: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SessionSort {
    Updated,
    Created,
    Name,
    MessageCount,
}

impl SessionState {
    pub fn new() -> Self {
        Self {
            sessions: Vec::new(),
            current_session: None,
            selected_index: None,
            filter: String::new(),
            sort_by: SessionSort::Updated,
            sort_descending: true,
        }
    }

    pub fn update_from_manager(&mut self, manager: &SessionManager) {
        // This would be called async in practice
        // For now, we'll keep it simple
    }

    pub fn add_session(&mut self, session: Session) {
        let item = SessionItem {
            id: session.id.clone(),
            name: session
                .metadata
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("Unnamed")
                .to_string(),
            model: session
                .metadata
                .get("model")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown")
                .to_string(),
            updated: session.updated_at.format("%Y-%m-%d %H:%M").to_string(),
            message_count: session.messages.len(),
            is_active: false,
        };

        // Update active status
        for s in &mut self.sessions {
            s.is_active = false;
        }
        self.sessions.insert(0, item);
        self.selected_index = Some(0);
    }

    pub fn remove_session(&mut self, id: &str) {
        self.sessions.retain(|s| s.id != id);
        if let Some(idx) = self.selected_index {
            if idx >= self.sessions.len() && !self.sessions.is_empty() {
                self.selected_index = Some(self.sessions.len() - 1);
            }
        }
    }

    pub fn set_active(&mut self, id: &str) {
        for s in &mut self.sessions {
            s.is_active = s.id == id;
        }
        self.selected_index = self.sessions.iter().position(|s| s.id == id);
        if let Some(idx) = self.selected_index {
            // Load full session
        }
    }

    pub fn update_session(&mut self, session: Session) {
        if let Some(idx) = self.sessions.iter().position(|s| s.id == session.id) {
            self.sessions[idx].name = session
                .metadata
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("Unnamed")
                .to_string();
            self.sessions[idx].model = session
                .metadata
                .get("model")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown")
                .to_string();
            self.sessions[idx].updated = session.updated_at.format("%Y-%m-%d %H:%M").to_string();
            self.sessions[idx].message_count = session.messages.len();
        }
        if self.current_session.as_ref().map(|s| &s.id) == Some(&session.id) {
            self.current_session = Some(session);
        }
    }

    pub fn selected_session(&self) -> Option<&SessionItem> {
        self.selected_index.and_then(|i| self.sessions.get(i))
    }

    pub fn filtered_sessions(&self) -> Vec<&SessionItem> {
        let mut sessions: Vec<_> = self
            .sessions
            .iter()
            .filter(|s| s.name.to_lowercase().contains(&self.filter.to_lowercase()))
            .collect();

        sessions.sort_by(|a, b| {
            let ord = match self.sort_by {
                SessionSort::Updated => a.updated.cmp(&b.updated),
                SessionSort::Created => a.id.cmp(&b.id), // Would need created_at
                SessionSort::Name => a.name.cmp(&b.name),
                SessionSort::MessageCount => a.message_count.cmp(&b.message_count),
            };
            if self.sort_descending {
                ord.reverse()
            } else {
                ord
            }
        });

        sessions
    }
}

impl Default for SessionState {
    fn default() -> Self {
        Self::new()
    }
}
