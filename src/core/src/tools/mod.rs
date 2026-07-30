mod read_file;
mod edit_file;
mod write_file;
mod run_command;
pub mod manage_todo;

pub use read_file::read_file;
pub use edit_file::edit_file;
pub use write_file::write_file;
pub use run_command::run_command;
pub use manage_todo::manage_todo;
pub use manage_todo::{
    init_todo_list, new_shared_todolist, SharedTodoList, TodoItem, TodoList, TodoPriority,
    TodoStatus,
};

use aisdk::core::tools::Tool;

pub fn all_tools() -> Vec<Tool> {
    vec![
        read_file(),
        edit_file(),
        write_file(),
        run_command(),
        manage_todo(),
    ]
}
