use super::{CommandContext, CommandInfo, CommandResult, SlashCommand, SubcommandInfo};
use crate::config::CustomProviderConfig;
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
                    name: "add".into(),
                    description: "add a custom provider with a base URL".into(),
                    usage: "/provider add <id> <base-url> [api-key]".into(),
                },
                SubcommandInfo {
                    name: "configure".into(),
                    description: "set a provider's API key".into(),
                    usage: "/provider configure <provider-id> [api-key]".into(),
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
                            " [selected]"
                        } else {
                            ""
                        };
                        let cfgd = if ctx.configured_providers.contains(id) {
                            " [configured]"
                        } else {
                            ""
                        };
                        format!("  {id:<16} {name:<16} {cfgd}{sel}")
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
                let info = ctx.providers.iter().find(|p| p.id == *provider_id);
                let requires_key = info.map(|p| p.requires_api_key).unwrap_or(false);
                let is_configured = ctx.configured_providers.contains(&provider_id.to_string());
                let output = if requires_key && !is_configured {
                    format!(
                        "Selected provider: {provider_id}. \
                         This provider requires an API key — run /provider configure {provider_id} \
                         to set it, or it won't work."
                    )
                } else {
                    format!("Selected provider: {provider_id}. Run /model to see available models.")
                };
                Ok(CommandResult {
                    command: "provider".into(),
                    output,
                    data: None,
                })
            }
            Some("add") => {
                let id = parts.get(1).ok_or_else(|| {
                    BimoError::Command("usage: /provider add <id> <base-url> [api-key]".into())
                })?;
                let base_url = parts.get(2).ok_or_else(|| {
                    BimoError::Command("usage: /provider add <id> <base-url> [api-key]".into())
                })?;
                let api_key = parts.get(3).map(|s| s.to_string());
                let name = id.to_string();
                let category = if api_key.is_some() { "cloud" } else { "local" };
                let cp = CustomProviderConfig {
                    id: id.to_string(),
                    name,
                    category: category.to_string(),
                    base_url: base_url.to_string(),
                    api_key_required: api_key.is_some(),
                    chat_endpoint: "/v1/chat/completions".into(),
                    models_endpoint: Some("/v1/models".into()),
                    auth_header: api_key.is_some().then(|| "Authorization".into()),
                    auth_prefix: api_key.is_some().then(|| "Bearer ".into()),
                };
                ctx.provider_add_request = Some(cp);
                Ok(CommandResult {
                    command: "provider".into(),
                    output: format!("Added custom provider '{id}'."),
                    data: None,
                })
            }
            Some("configure") => {
                let provider_id = parts.get(1).ok_or_else(|| {
                    BimoError::Command("usage: /provider configure <provider-id> <api-key>".into())
                })?;
                let exists = ctx.provider_ids.iter().any(|id| id == *provider_id);
                if !exists {
                    return Err(BimoError::Command(format!(
                        "provider '{provider_id}' not found."
                    )));
                }
                let api_key = parts.get(2).ok_or_else(|| {
                    BimoError::Command(format!(
                        "usage: /provider configure {provider_id} <api-key>"
                    ))
                })?;
                ctx.provider_configure_request =
                    Some((provider_id.to_string(), Some(api_key.to_string()), None));
                Ok(CommandResult {
                    command: "provider".into(),
                    output: format!("Configured '{provider_id}' with API key."),
                    data: None,
                })
            }
            Some(other) => Err(BimoError::Command(format!(
                "unknown subcommand '{other}'. Usage: /provider [list|select|add|configure]"
            ))),
        }
    }
}
