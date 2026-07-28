use super::{CommandContext, CommandInfo, CommandResult, SlashCommand, SubcommandInfo};
use crate::error::{BimoError, Result};

pub(super) struct ProviderCommand;

impl SlashCommand for ProviderCommand {
    fn name(&self) -> &str {
        "provider"
    }

    fn description(&self) -> &str {
        "list, select, or configure providers"
    }

    fn command_info(&self) -> CommandInfo {
        CommandInfo {
            name: "provider".into(),
            description: self.description().into(),
            subcommands: vec![
                SubcommandInfo {
                    name: "list".into(),
                    description: "list all available providers".into(),
                    usage: "/provider list".into(),
                },
                SubcommandInfo {
                    name: "select".into(),
                    description: "select a provider by id".into(),
                    usage: "/provider select <provider-id>".into(),
                },
                SubcommandInfo {
                    name: "configure".into(),
                    description: "configure a provider's base URL and API key".into(),
                    usage: "/provider configure <provider-id>".into(),
                },
            ],
            async_command: false,
        }
    }

    fn execute(&self, ctx: &mut CommandContext, args: &str) -> Result<CommandResult> {
        let parts: Vec<&str> = args.split_whitespace().collect();
        match parts.first().copied() {
            Some("list") | None => {
                let lines: Vec<String> = ctx
                    .provider_ids
                    .iter()
                    .zip(ctx.provider_names.iter())
                    .map(|(id, name)| {
                        let sel = if Some(id) == ctx.selected_provider.as_ref() {
                            " *"
                        } else {
                            ""
                        };
                        format!("  {id} — {name}{sel}")
                    })
                    .collect();
                let output = format!("Available providers:\n{}", lines.join("\n"));
                Ok(CommandResult {
                    command: "provider".into(),
                    output,
                    data: None,
                })
            }
            Some("select") => {
                let provider_id = parts.get(1).ok_or_else(|| {
                    BimoError::Command("usage: /provider select <provider-id>".into())
                })?;
                let exists = ctx.provider_ids.iter().any(|id| id == *provider_id);
                if !exists {
                    return Err(BimoError::Command(format!(
                        "provider '{provider_id}' not found. Use /provider list."
                    )));
                }
                ctx.selected_provider = Some(provider_id.to_string());
                Ok(CommandResult {
                    command: "provider".into(),
                    output: format!(
                        "Selected provider: {provider_id}. Run /model to see available models."
                    ),
                    data: None,
                })
            }
            Some("configure") => {
                let provider_id = parts.get(1).ok_or_else(|| {
                    BimoError::Command("usage: /provider configure <provider-id>".into())
                })?;
                let exists = ctx.provider_ids.iter().any(|id| id == *provider_id);
                if !exists {
                    return Err(BimoError::Command(format!(
                        "provider '{provider_id}' not found."
                    )));
                }
                Ok(CommandResult {
                    command: "provider".into(),
                    output: format!(
                        "To configure '{provider_id}', use the API endpoint \
                         POST /api/provider/configure with your settings."
                    ),
                    data: None,
                })
            }
            Some(other) => Err(BimoError::Command(format!(
                "unknown subcommand '{other}'. Usage: /provider [list|select|configure]"
            ))),
        }
    }
}
