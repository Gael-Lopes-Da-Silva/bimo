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

    /// Render the todo list for inclusion in the system prompt / context.
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

    /// Render a compact single-line summary for display.
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

    /// Render the full todo list for the /todo command output.
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
                    item.updated_at.format("%H:%M UTC"),
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
                    item.created_at.format("%H:%M UTC"),
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
                    item.updated_at.format("%H:%M UTC"),
                ));
            }
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_todo() {
        let mut list = TodoList::new();
        let item = list.add("Implement feature X");
        assert_eq!(item.id, 1);
        assert_eq!(item.description, "Implement feature X");
        assert_eq!(item.status, TodoStatus::Pending);
    }

    #[test]
    fn add_multiple_todos() {
        let mut list = TodoList::new();
        list.add("Task 1");
        list.add("Task 2");
        list.add("Task 3");
        assert_eq!(list.len(), 3);
        assert_eq!(list.items()[0].id, 1);
        assert_eq!(list.items()[1].id, 2);
        assert_eq!(list.items()[2].id, 3);
    }

    #[test]
    fn update_status() {
        let mut list = TodoList::new();
        list.add("Task");
        let item = list.update_status(1, TodoStatus::InProgress).unwrap();
        assert_eq!(item.status, TodoStatus::InProgress);
    }

    #[test]
    fn update_status_done() {
        let mut list = TodoList::new();
        list.add("Task");
        list.update_status(1, TodoStatus::InProgress);
        let item = list.update_status(1, TodoStatus::Done).unwrap();
        assert_eq!(item.status, TodoStatus::Done);
    }

    #[test]
    fn update_status_nonexistent() {
        let mut list = TodoList::new();
        assert!(list.update_status(99, TodoStatus::Done).is_none());
    }

    #[test]
    fn update_description() {
        let mut list = TodoList::new();
        list.add("Old description");
        let item = list.update_description(1, "New description").unwrap();
        assert_eq!(item.description, "New description");
    }

    #[test]
    fn remove_todo() {
        let mut list = TodoList::new();
        list.add("Task");
        let removed = list.remove(1).unwrap();
        assert_eq!(removed.id, 1);
        assert!(list.is_empty());
    }

    #[test]
    fn remove_nonexistent() {
        let mut list = TodoList::new();
        assert!(list.remove(99).is_none());
    }

    #[test]
    fn get_todo() {
        let mut list = TodoList::new();
        list.add("Task");
        let item = list.get(1).unwrap();
        assert_eq!(item.description, "Task");
        assert!(list.get(99).is_none());
    }

    #[test]
    fn filter_by_status() {
        let mut list = TodoList::new();
        list.add("Task 1");
        list.add("Task 2");
        list.add("Task 3");
        list.update_status(1, TodoStatus::InProgress);
        list.update_status(2, TodoStatus::Done);

        assert_eq!(list.pending().len(), 1);
        assert_eq!(list.in_progress().len(), 1);
        assert_eq!(list.done().len(), 1);
        assert_eq!(list.pending()[0].id, 3);
        assert_eq!(list.in_progress()[0].id, 1);
        assert_eq!(list.done()[0].id, 2);
    }

    #[test]
    fn clear_todos() {
        let mut list = TodoList::new();
        list.add("Task 1");
        list.add("Task 2");
        list.clear();
        assert!(list.is_empty());
        assert_eq!(list.len(), 0);
    }

    #[test]
    fn render_context_empty() {
        let list = TodoList::new();
        assert_eq!(list.render_context(), "No todos.");
    }

    #[test]
    fn render_context_with_todos() {
        let mut list = TodoList::new();
        list.add("Task 1");
        list.add("Task 2");
        list.update_status(1, TodoStatus::InProgress);
        list.update_status(2, TodoStatus::Done);

        let ctx = list.render_context();
        assert!(ctx.contains("In Progress:"));
        assert!(ctx.contains("[1] Task 1"));
        assert!(ctx.contains("Done:"));
        assert!(ctx.contains("[2] Task 2"));
    }

    #[test]
    fn render_summary() {
        let mut list = TodoList::new();
        list.add("Task 1");
        list.add("Task 2");
        list.update_status(1, TodoStatus::InProgress);

        let summary = list.render_summary();
        assert!(summary.contains("2 todos"));
        assert!(summary.contains("1 pending"));
        assert!(summary.contains("1 in progress"));
        assert!(summary.contains("0 done"));
    }

    #[test]
    fn render_full_empty() {
        let list = TodoList::new();
        assert_eq!(list.render_full(), "No todos.");
    }

    #[test]
    fn render_full_with_todos() {
        let mut list = TodoList::new();
        list.add("Task 1");
        list.add("Task 2");
        list.update_status(1, TodoStatus::Done);

        let full = list.render_full();
        assert!(full.contains("Done:"));
        assert!(full.contains("[1] Task 1"));
        assert!(full.contains("Pending:"));
        assert!(full.contains("[2] Task 2"));
    }

    #[test]
    fn todo_is_serializable() {
        let mut list = TodoList::new();
        list.add("Task");
        list.update_status(1, TodoStatus::InProgress);

        let json = serde_json::to_string(&list).unwrap();
        let deserialized: TodoList = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.len(), 1);
        assert_eq!(deserialized.items()[0].status, TodoStatus::InProgress);
        assert_eq!(deserialized.items()[0].description, "Task");
    }

    #[test]
    fn todo_item_is_serializable() {
        let mut list = TodoList::new();
        list.add("Test item");
        let item = list.get(1).unwrap();
        let json = serde_json::to_string(item).unwrap();
        let deserialized: TodoItem = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, 1);
        assert_eq!(deserialized.description, "Test item");
    }
}
