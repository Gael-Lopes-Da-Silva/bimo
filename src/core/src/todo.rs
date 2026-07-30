use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TodoStatus {
    Pending,
    InProgress,
    Done,
}

impl std::fmt::Display for TodoStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TodoStatus::Pending => write!(f, "pending"),
            TodoStatus::InProgress => write!(f, "in_progress"),
            TodoStatus::Done => write!(f, "done"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoItem {
    pub id: u32,
    pub description: String,
    pub status: TodoStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TodoList {
    items: Vec<TodoItem>,
    next_id: u32,
}

impl TodoList {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, description: &str) -> &TodoItem {
        self.next_id += 1;
        let now = Utc::now();
        self.items.push(TodoItem {
            id: self.next_id,
            description: description.to_string(),
            status: TodoStatus::Pending,
            created_at: now,
            updated_at: now,
        });
        self.items.last().unwrap()
    }

    pub fn update_status(&mut self, id: u32, status: TodoStatus) -> Option<&TodoItem> {
        let item = self.items.iter_mut().find(|i| i.id == id)?;
        item.status = status;
        item.updated_at = Utc::now();
        Some(item)
    }

    pub fn update_description(&mut self, id: u32, description: &str) -> Option<&TodoItem> {
        let item = self.items.iter_mut().find(|i| i.id == id)?;
        item.description = description.to_string();
        item.updated_at = Utc::now();
        Some(item)
    }

    pub fn remove(&mut self, id: u32) -> Option<TodoItem> {
        let pos = self.items.iter().position(|i| i.id == id)?;
        Some(self.items.remove(pos))
    }

    pub fn get(&self, id: u32) -> Option<&TodoItem> {
        self.items.iter().find(|i| i.id == id)
    }

    pub fn items(&self) -> &[TodoItem] {
        &self.items
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn pending(&self) -> Vec<&TodoItem> {
        self.items
            .iter()
            .filter(|i| i.status == TodoStatus::Pending)
            .collect()
    }

    pub fn in_progress(&self) -> Vec<&TodoItem> {
        self.items
            .iter()
            .filter(|i| i.status == TodoStatus::InProgress)
            .collect()
    }

    pub fn done(&self) -> Vec<&TodoItem> {
        self.items
            .iter()
            .filter(|i| i.status == TodoStatus::Done)
            .collect()
    }

    pub fn clear(&mut self) {
        self.items.clear();
        self.next_id = 0;
    }

    pub fn render_context(&self) -> String {
        if self.items.is_empty() {
            return "No todos.".to_string();
        }
        let mut out = String::new();
        let in_progress = self.in_progress();
        let pending = self.pending();
        let done = self.done();
        if !in_progress.is_empty() {
            out.push_str("In Progress:\n");
            for item in &in_progress {
                out.push_str(&format!("  [{}] {}\n", item.id, item.description));
            }
        }
        if !pending.is_empty() {
            out.push_str("Pending:\n");
            for item in &pending {
                out.push_str(&format!("  [{}] {}\n", item.id, item.description));
            }
        }
        if !done.is_empty() {
            out.push_str("Done:\n");
            for item in &done {
                out.push_str(&format!("  [{}] {}\n", item.id, item.description));
            }
        }
        out
    }

    pub fn render_summary(&self) -> String {
        let pending = self.pending().len();
        let in_progress = self.in_progress().len();
        let done = self.done().len();
        let total = self.items.len();
        format!(
            "{} todos ({} pending, {} in progress, {} done)",
            total, pending, in_progress, done
        )
    }

    pub fn render_full(&self) -> String {
        if self.items.is_empty() {
            return "No todos.".to_string();
        }
        let mut out = String::new();
        let in_progress = self.in_progress();
        let pending = self.pending();
        let done = self.done();
        if !in_progress.is_empty() {
            out.push_str("In Progress:\n");
            for item in &in_progress {
                out.push_str(&format!(
                    "  [{}] {} (updated {})\n",
                    item.id,
                    item.description,
                    item.updated_at.format("%H:%M UTC")
                ));
            }
        }
        if !pending.is_empty() {
            out.push_str("Pending:\n");
            for item in &pending {
                out.push_str(&format!(
                    "  [{}] {} (created {})\n",
                    item.id,
                    item.description,
                    item.created_at.format("%H:%M UTC")
                ));
            }
        }
        if !done.is_empty() {
            out.push_str("Done:\n");
            for item in &done {
                out.push_str(&format!(
                    "  [{}] {} (completed {})\n",
                    item.id,
                    item.description,
                    item.updated_at.format("%H:%M UTC")
                ));
            }
        }
        out
    }
}
