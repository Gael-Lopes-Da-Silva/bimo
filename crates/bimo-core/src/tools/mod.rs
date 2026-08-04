mod ask_user;
mod create_file;
mod delete_file;
pub mod manage_todo;
mod read_file;
mod run_command;
mod update_file;

pub use ask_user::{ask_user, has_question_handler, set_question_handler};
pub use create_file::create_file;
pub use delete_file::delete_file;
pub use manage_todo::manage_todo;
pub use manage_todo::{
    SharedTodoList, TodoItem, TodoList, TodoPriority, TodoStatus, init_todo_list,
    new_shared_todo_list, shared_todo_list, todo_list_snapshot,
};
pub use read_file::read_file;
pub use run_command::run_command;
pub use update_file::update_file;

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
        ask_user(),
        create_file(),
        read_file(),
        update_file(),
        delete_file(),
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
