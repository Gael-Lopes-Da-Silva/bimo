use super::{CommandContext, CommandInfo, CommandResult, SlashCommand, SubcommandInfo};
use crate::error::{BimoError, Result};

pub(super) struct SessionCommand;

impl SlashCommand for SessionCommand {
    fn name(&self) -> &str {
        "session"
    }

    fn description(&self) -> &str {
        "manage sessions"
    }

    fn command_info(&self) -> CommandInfo {
        CommandInfo {
            name: "session".into(),
            description: self.description().into(),
            subcommands: vec![
                SubcommandInfo {
                    name: "list".into(),
                    description: "list all saved sessions".into(),
                    usage: "/session list".into(),
                },
                SubcommandInfo {
                    name: "save".into(),
                    description: "save the current session to disk".into(),
                    usage: "/session save".into(),
                },
                SubcommandInfo {
                    name: "resume".into(),
                    description: "resume a saved session by id (supports prefix)".into(),
                    usage: "/session resume <session-id>".into(),
                },
                SubcommandInfo {
                    name: "delete".into(),
                    description: "delete a saved session".into(),
                    usage: "/session delete <session-id>".into(),
                },
                SubcommandInfo {
                    name: "info".into(),
                    description: "show current session details".into(),
                    usage: "/session info".into(),
                },
                SubcommandInfo {
                    name: "purge".into(),
                    description: "delete all saved sessions".into(),
                    usage: "/session purge".into(),
                },
            ],
            async_command: false,
        }
    }

    fn execute(&self, ctx: &mut CommandContext, args: &str) -> Result<CommandResult> {
        let parts: Vec<&str> = args.split_whitespace().collect();
        match parts.first().copied() {
            Some("list") | None => {
                if ctx.saved_sessions.is_empty() {
                    return Ok(CommandResult {
                        command: "session".into(),
                        output: "No saved sessions.".into(),
                        data: Some(serde_json::json!([])),
                    });
                }
                let lines: Vec<String> = ctx
                    .saved_sessions
                    .iter()
                    .map(|s| {
                        let active = if s.id == ctx.session_id {
                            " (active)"
                        } else {
                            ""
                        };
                        format!(
                            "  {} — {} messages, updated {}{active}",
                            &s.id[..8.min(s.id.len())],
                            s.message_count,
                            s.updated_at.format("%Y-%m-%d %H:%M UTC"),
                        )
                    })
                    .collect();
                let output = format!(
                    "Saved sessions ({}):\n{}",
                    ctx.saved_sessions.len(),
                    lines.join("\n")
                );
                let data = serde_json::to_value(&ctx.saved_sessions).ok();
                Ok(CommandResult {
                    command: "session".into(),
                    output,
                    data,
                })
            }
            Some("save") => Ok(CommandResult {
                command: "session".into(),
                output: "Session saved.".into(),
                data: None,
            }),
            Some("resume") => {
                let id = parts.get(1).ok_or_else(|| {
                    BimoError::Command("usage: /session resume <session-id>".into())
                })?;
                let found = ctx
                    .saved_sessions
                    .iter()
                    .find(|s| s.id == *id || s.id.starts_with(id));
                match found {
                    Some(info) => Ok(CommandResult {
                        command: "session".into(),
                        output: format!(
                            "Resumed session {} ({} messages).",
                            &info.id[..8.min(info.id.len())],
                            info.message_count,
                        ),
                        data: Some(serde_json::json!({ "session_id": info.id })),
                    }),
                    None => Err(BimoError::Command(format!(
                        "session '{id}' not found. Use /session list."
                    ))),
                }
            }
            Some("delete") => {
                let id = parts.get(1).ok_or_else(|| {
                    BimoError::Command("usage: /session delete <session-id>".into())
                })?;
                let found = ctx
                    .saved_sessions
                    .iter()
                    .find(|s| s.id == *id || s.id.starts_with(id));
                match found {
                    Some(info) => Ok(CommandResult {
                        command: "session".into(),
                        output: format!("Deleted session {}.", &info.id[..8.min(info.id.len())]),
                        data: Some(serde_json::json!({ "session_id": info.id })),
                    }),
                    None => Err(BimoError::Command(format!(
                        "session '{id}' not found. Use /session list."
                    ))),
                }
            }
            Some("info") => {
                let output = format!(
                    "Session:     {}\n\
                     Messages:    {}\n\
                     Created:     {}\n\
                     Last active: {}",
                    ctx.session_id, ctx.session_message_count, "—", "—",
                );
                let data = serde_json::json!({
                    "session_id": ctx.session_id,
                    "message_count": ctx.session_message_count,
                });
                Ok(CommandResult {
                    command: "session".into(),
                    output,
                    data: Some(data),
                })
            }
            Some("purge") => Ok(CommandResult {
                command: "session".into(),
                output: "All saved sessions purged.".into(),
                data: None,
            }),
            Some(other) => Err(BimoError::Command(format!(
                "unknown subcommand '{other}'. Usage: /session [list|save|resume|delete|info|purge]"
            ))),
        }
    }
}
