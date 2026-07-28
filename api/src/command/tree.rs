use super::{
    CommandContext, CommandInfo, CommandResult, SlashCommand, SubcommandInfo, format_role, truncate,
};
use crate::error::{BimoError, Result};

pub(super) struct TreeCommand;

impl SlashCommand for TreeCommand {
    fn name(&self) -> &str {
        "tree"
    }

    fn description(&self) -> &str {
        "view conversation tree and fork or revert"
    }

    fn command_info(&self) -> CommandInfo {
        CommandInfo {
            name: "tree".into(),
            description: self.description().into(),
            subcommands: vec![
                SubcommandInfo {
                    name: "fork".into(),
                    description: "fork to a new session at a message index".into(),
                    usage: "/tree fork <index>".into(),
                },
                SubcommandInfo {
                    name: "revert".into(),
                    description: "revert to a message index, erasing later context".into(),
                    usage: "/tree revert <index>".into(),
                },
            ],
            async_command: false,
        }
    }

    fn execute(&self, ctx: &mut CommandContext, args: &str) -> Result<CommandResult> {
        let parts: Vec<&str> = args.split_whitespace().collect();
        match parts.first().copied() {
            Some("fork") => {
                let index: usize = parts
                    .get(1)
                    .ok_or_else(|| BimoError::Command("usage: /tree fork <index>".into()))?
                    .parse()
                    .map_err(|_| BimoError::Command("index must be a number".into()))?;

                if index >= ctx.session_messages.len() {
                    return Err(BimoError::Command(format!(
                        "index {} out of range (session has {} messages)",
                        index,
                        ctx.session_messages.len()
                    )));
                }

                ctx.tree_fork_index = Some(index);

                let role = format_role(&ctx.session_messages[index].role);
                let preview = truncate(&ctx.session_messages[index].content, 40);

                Ok(CommandResult {
                    command: "tree".into(),
                    output: format!("Forking at message {index} [{role}]: {preview}..."),
                    data: Some(serde_json::json!({
                        "action": "fork",
                        "index": index,
                    })),
                })
            }
            Some("revert") => {
                let index: usize = parts
                    .get(1)
                    .ok_or_else(|| BimoError::Command("usage: /tree revert <index>".into()))?
                    .parse()
                    .map_err(|_| BimoError::Command("index must be a number".into()))?;

                if index >= ctx.session_messages.len() {
                    return Err(BimoError::Command(format!(
                        "index {} out of range (session has {} messages)",
                        index,
                        ctx.session_messages.len()
                    )));
                }

                let keep = index + 1;
                let discard = ctx.session_messages.len() - keep;

                ctx.tree_revert_index = Some(index);

                let role = format_role(&ctx.session_messages[index].role);
                let preview = truncate(&ctx.session_messages[index].content, 40);

                Ok(CommandResult {
                    command: "tree".into(),
                    output: format!(
                        "Reverting to message {index} [{role}]: {preview}... ({discard} messages will be removed)"
                    ),
                    data: Some(serde_json::json!({
                        "action": "revert",
                        "index": index,
                    })),
                })
            }
            None | Some(_) => {
                if ctx.session_messages.is_empty() {
                    return Ok(CommandResult {
                        command: "tree".into(),
                        output: "Session is empty.".into(),
                        data: Some(serde_json::json!({ "messages": [] })),
                    });
                }

                let mut lines: Vec<String> = Vec::new();
                for (i, msg) in ctx.session_messages.iter().enumerate() {
                    let role = format_role(&msg.role);
                    let preview = truncate(&msg.content, 60);
                    let marker = if i == ctx.session_messages.len() - 1 {
                        " <- latest"
                    } else {
                        ""
                    };
                    lines.push(format!("  {i:>3}  [{role}]  {preview}{marker}"));
                }

                let output = format!(
                    "Conversation tree ({} messages, session {}):\n{}\n\nUse /tree fork <index> or /tree revert <index>.",
                    ctx.session_messages.len(),
                    &ctx.session_id[..8.min(ctx.session_id.len())],
                    lines.join("\n")
                );

                Ok(CommandResult {
                    command: "tree".into(),
                    output,
                    data: Some(serde_json::json!({
                        "messages": ctx.session_messages.iter().enumerate().map(|(i, m)| {
                            serde_json::json!({
                                "index": i,
                                "role": format_role(&m.role),
                                "preview": truncate(&m.content, 60),
                            })
                        }).collect::<Vec<_>>(),
                    })),
                })
            }
        }
    }
}
