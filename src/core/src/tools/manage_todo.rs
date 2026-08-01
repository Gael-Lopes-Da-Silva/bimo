use std::sync::{Arc, Mutex, OnceLock};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::info;

use aisdk::core::tools::Tool;
use aisdk::macros::tool;

static TODO_LIST: OnceLock<SharedTodoList> = OnceLock::new();

/// A thread-safe shared reference to a [`TodoList`].
pub type SharedTodoList = Arc<Mutex<TodoList>>;

/// The status of a todo item.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TodoStatus {
    /// Not started yet.
    #[serde(rename = "pending")]
    Pending,
    /// Currently being worked on.
    #[serde(rename = "in_progress")]
    InProgress,
    /// Finished.
    #[serde(rename = "completed")]
    Completed,
    /// Discarded without completion.
    #[serde(rename = "cancelled")]
    Cancelled,
}

/// Priority level for a todo item.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TodoPriority {
    /// High priority.
    #[serde(rename = "high")]
    High,
    /// Medium priority.
    #[serde(rename = "medium")]
    Medium,
    /// Low priority.
    #[serde(rename = "low")]
    Low,
}

/// A single todo item with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoItem {
    /// Unique item id.
    pub id: String,
    /// Human-readable description of the task.
    pub description: String,
    /// Current status.
    pub status: TodoStatus,
    /// Priority level.
    pub priority: TodoPriority,
    /// When the item was created.
    pub created_at: DateTime<Utc>,
    /// When the item was last updated.
    pub updated_at: DateTime<Utc>,
}

/// An ordered collection of [`TodoItem`]s.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoList {
    pub items: Vec<TodoItem>,
}

impl TodoList {
    /// Creates an empty todo list.
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// Adds a new item with the given description and priority.
    ///
    /// Returns the generated item id.
    pub fn add(&mut self, description: String, priority: TodoPriority) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now();
        self.items.push(TodoItem {
            id: id.clone(),
            description,
            status: TodoStatus::Pending,
            priority,
            created_at: now,
            updated_at: now,
        });
        id
    }

    /// Updates the status of an item by id.
    ///
    /// Returns `true` if the item was found and updated.
    pub fn update_status(&mut self, id: &str, status: TodoStatus) -> bool {
        if let Some(item) = self.items.iter_mut().find(|i| i.id == id) {
            item.status = status;
            item.updated_at = Utc::now();
            true
        } else {
            false
        }
    }

    /// Removes an item by id.
    ///
    /// Returns `true` if the item was found and removed.
    pub fn remove(&mut self, id: &str) -> bool {
        let len = self.items.len();
        self.items.retain(|i| i.id != id);
        self.items.len() < len
    }

    /// Returns a one-line summary of the todo list status counts.
    pub fn summary(&self) -> String {
        let total = self.items.len();
        let pending = self
            .items
            .iter()
            .filter(|i| i.status == TodoStatus::Pending)
            .count();
        let in_progress = self
            .items
            .iter()
            .filter(|i| i.status == TodoStatus::InProgress)
            .count();
        let completed = self
            .items
            .iter()
            .filter(|i| i.status == TodoStatus::Completed)
            .count();
        let cancelled = self
            .items
            .iter()
            .filter(|i| i.status == TodoStatus::Cancelled)
            .count();

        format!(
            "Todo: {total} total ({pending} pending, {in_progress} in progress, {completed} completed, {cancelled} cancelled)"
        )
    }

    /// Formats the full todo list as a human-readable string.
    pub fn format(&self) -> String {
        if self.items.is_empty() {
            return "No todo items.".to_string();
        }

        let mut output = String::from("## Todo List\n\n");
        for item in &self.items {
            let status_icon = match item.status {
                TodoStatus::Pending => "[ ]",
                TodoStatus::InProgress => "[>]",
                TodoStatus::Completed => "[x]",
                TodoStatus::Cancelled => "[-]",
            };
            let priority_str = match item.priority {
                TodoPriority::High => "high",
                TodoPriority::Medium => "medium",
                TodoPriority::Low => "low",
            };
            output.push_str(&format!(
                "- {} {} {} ({})\n",
                status_icon, priority_str, item.description, item.id
            ));
        }
        output
    }
}

impl Default for TodoList {
    fn default() -> Self {
        Self::new()
    }
}

/// Manages a task list — supports `add`, `update`, `remove`, and `list` actions.
#[tool(
    name = "manage_todo",
    desc = "Manage a task list. Actions: add, update, remove, list. When adding, provide a description and priority (high/medium/low). When updating, provide the item id and new status (pending/in_progress/completed/cancelled)."
)]
pub fn manage_todo(
    action: String,
    description: Option<String>,
    priority: Option<String>,
    id: Option<String>,
    status: Option<String>,
) -> Tool {
    let todo_list = TODO_LIST.get_or_init(new_shared_todo_list);

    let mut list = todo_list.lock().map_err(|e| format!("Lock error: {}", e))?;

    match action.as_str() {
        "add" => {
            let desc = description.ok_or_else(|| "description required for add".to_string())?;
            let prio = match priority.as_deref() {
                Some("high") => TodoPriority::High,
                Some("medium") | None => TodoPriority::Medium,
                Some("low") => TodoPriority::Low,
                Some(other) => return Err(format!("invalid priority: {other}")),
            };
            let item_id = list.add(desc, prio);
            info!("Todo added: {}", item_id);
            Ok(format!("Added todo: {item_id}\n{}", list.format()))
        }
        "update" => {
            let item_id = id.ok_or_else(|| "id required for update".to_string())?;
            let new_status = match status.as_deref() {
                Some("pending") => TodoStatus::Pending,
                Some("in_progress") => TodoStatus::InProgress,
                Some("completed") => TodoStatus::Completed,
                Some("cancelled") => TodoStatus::Cancelled,
                Some(other) => return Err(format!("invalid status: {other}")),
                None => return Err("status required for update".to_string()),
            };
            if list.update_status(&item_id, new_status) {
                info!("Todo updated: {}", item_id);
                Ok(format!("Updated todo {item_id}\n{}", list.format()))
            } else {
                Err(format!("Todo item {item_id} not found"))
            }
        }
        "remove" => {
            let item_id = id.ok_or_else(|| "id required for remove".to_string())?;
            if list.remove(&item_id) {
                info!("Todo removed: {}", item_id);
                Ok(format!("Removed todo {item_id}\n{}", list.format()))
            } else {
                Err(format!("Todo item {item_id} not found"))
            }
        }
        "list" => Ok(list.format()),
        other => Err(format!("unknown action: {other}")),
    }
}

/// Creates a new empty shared todo list.
pub fn new_shared_todo_list() -> SharedTodoList {
    Arc::new(Mutex::new(TodoList::new()))
}

/// Initializes the global todo list singleton.
///
/// Must be called once before `manage_todo` is used.
pub fn init_todo_list(todo: SharedTodoList) {
    let _ = TODO_LIST.set(todo);
}
