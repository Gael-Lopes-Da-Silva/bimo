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

use aisdk::core::tools::Tool;

/// Returns all available tools as a [`Vec<Tool>`] for the agent.
pub fn all_tools() -> Vec<Tool> {
    vec![
        read_file(),
        edit_file(),
        write_file(),
        run_command(),
        manage_todo(),
    ]
}

/// Returns a human-readable description of all available tools for the system prompt.
pub fn describe_tools() -> String {
    let desc = vec![
        (
            "read_file",
            "Read contents of a file (with optional line range).",
        ),
        (
            "edit_file",
            "Make precise string replacements in an existing file. Provide old_string and new_string.",
        ),
        ("write_file", "Create or overwrite a file with new content."),
        (
            "run_command",
            "Execute shell commands in the workspace. Supports output capture and timeout.",
        ),
        (
            "manage_todo",
            "Track tasks during the session. Actions: add, update, remove, list.",
        ),
    ];

    let mut out = String::new();
    for (name, desc) in desc {
        out.push_str(&format!("- **{name}**: {desc}\n"));
    }
    out
}
