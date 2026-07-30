use super::{CommandContext, CommandInfo, CommandResult, SlashCommand, SubcommandInfo};
use crate::error::{BimoError, Result};
use crate::todo::TodoStatus;

pub(super) struct TodoCommand;

impl SlashCommand for TodoCommand {
    fn name(&self) -> &str {
        "todo"
    }

    fn description(&self) -> &str {
        "manage session todos"
    }

    fn command_info(&self) -> CommandInfo {
        CommandInfo {
            name: "todo".into(),
            description: self.description().into(),
            subcommands: vec![
                SubcommandInfo {
                    name: "list".into(),
                    description: "list all todos (default)".into(),
                    usage: "/todo".into(),
                },
                SubcommandInfo {
                    name: "clear".into(),
                    description: "clear all todos".into(),
                    usage: "/todo clear".into(),
                },
                SubcommandInfo {
                    name: "done".into(),
                    description: "mark a todo as done".into(),
                    usage: "/todo done <id>".into(),
                },
                SubcommandInfo {
                    name: "progress".into(),
                    description: "mark a todo as in progress".into(),
                    usage: "/todo progress <id>".into(),
                },
                SubcommandInfo {
                    name: "pending".into(),
                    description: "mark a todo as pending".into(),
                    usage: "/todo pending <id>".into(),
                },
            ],
            async_command: false,
        }
    }

    fn execute(&self, ctx: &mut CommandContext, args: &str) -> Result<CommandResult> {
        let parts: Vec<&str> = args.split_whitespace().collect();
        match parts.first().copied() {
            Some("list") | None => {
                let output = if ctx.todos.is_empty() {
                    "No todos.".to_string()
                } else {
                    let summary = ctx.todos.render_summary();
                    format!("{}\n\n{}", summary, ctx.todos.render_full())
                };
                Ok(CommandResult {
                    command: "todo".into(),
                    output,
                    data: Some(serde_json::json!({
                        "todos": ctx.todos.items(),
                    })),
                })
            }
            Some("clear") => {
                let count = ctx.todos.len();
                ctx.todos.clear();
                Ok(CommandResult {
                    command: "todo".into(),
                    output: format!("Cleared {} todo(s).", count),
                    data: None,
                })
            }
            Some("done") => {
                let id = parts
                    .get(1)
                    .ok_or_else(|| BimoError::Command("usage: /todo done <id>".into()))?
                    .parse::<u32>()
                    .map_err(|_| BimoError::Command("id must be a number".into()))?;
                match ctx.todos.update_status(id, TodoStatus::Done) {
                    Some(item) => Ok(CommandResult {
                        command: "todo".into(),
                        output: format!("Marked todo #{} as done: {}", item.id, item.description),
                        data: None,
                    }),
                    None => Err(BimoError::Command(format!("todo #{} not found", id))),
                }
            }
            Some("progress") => {
                let id = parts
                    .get(1)
                    .ok_or_else(|| BimoError::Command("usage: /todo progress <id>".into()))?
                    .parse::<u32>()
                    .map_err(|_| BimoError::Command("id must be a number".into()))?;
                match ctx.todos.update_status(id, TodoStatus::InProgress) {
                    Some(item) => Ok(CommandResult {
                        command: "todo".into(),
                        output: format!(
                            "Marked todo #{} as in progress: {}",
                            item.id, item.description
                        ),
                        data: None,
                    }),
                    None => Err(BimoError::Command(format!("todo #{} not found", id))),
                }
            }
            Some("pending") => {
                let id = parts
                    .get(1)
                    .ok_or_else(|| BimoError::Command("usage: /todo pending <id>".into()))?
                    .parse::<u32>()
                    .map_err(|_| BimoError::Command("id must be a number".into()))?;
                match ctx.todos.update_status(id, TodoStatus::Pending) {
                    Some(item) => Ok(CommandResult {
                        command: "todo".into(),
                        output: format!(
                            "Marked todo #{} as pending: {}",
                            item.id, item.description
                        ),
                        data: None,
                    }),
                    None => Err(BimoError::Command(format!("todo #{} not found", id))),
                }
            }
            Some(other) => Err(BimoError::Command(format!(
                "unknown subcommand '{other}'. Usage: /todo [list|clear|done|progress|pending]"
            ))),
        }
    }
}
