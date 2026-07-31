//! Built-in tool definitions for the agent.

mod edit_file;
pub mod manage_todo;
mod read_file;
mod run_command;
mod write_file;

pub use edit_file::edit_file;
pub use manage_todo::manage_todo;
pub use manage_todo::{
    SharedTodoList, TodoItem, TodoList, TodoPriority, TodoStatus, init_todo_list,
    new_shared_todolist,
};
pub use read_file::read_file;
pub use run_command::run_command;
pub use write_file::write_file;

use std::collections::BTreeSet;

use aisdk::core::tools::Tool;

/// Returns the names of all built-in tools.
pub fn tool_names() -> Vec<String> {
    let empty = BTreeSet::new();
    all_tools(&empty).into_iter().map(|t| t.name).collect()
}

/// Returns `true` if `name` matches a built-in tool.
pub fn is_builtin(name: &str) -> bool {
    tool_names().iter().any(|n| n == name)
}

/// Returns the built-in tools that are not disabled for the session.
pub fn all_tools(disabled: &BTreeSet<String>) -> Vec<Tool> {
    vec![
        read_file(),
        edit_file(),
        write_file(),
        run_command(),
        manage_todo(),
    ]
    .into_iter()
    .filter(|t| !disabled.contains(&t.name))
    .collect()
}

/// Returns a human-readable description of the non-disabled tools for the system prompt.
pub fn describe_tools(disabled: &BTreeSet<String>) -> String {
    let mut out = String::new();
    for tool in all_tools(disabled) {
        out.push_str(&format!("- **{}**: {}\n", tool.name, tool.description));
    }
    out
}
