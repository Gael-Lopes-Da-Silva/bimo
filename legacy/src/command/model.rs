use super::{CommandContext, CommandInfo, CommandResult, SlashCommand, SubcommandInfo};
use crate::error::{BimoError, Result};

pub(super) struct ModelCommand;

impl SlashCommand for ModelCommand {
    fn name(&self) -> &str {
        "model"
    }

    fn description(&self) -> &str {
        "list models or select a model"
    }

    fn command_info(&self) -> CommandInfo {
        CommandInfo {
            name: "model".into(),
            description: self.description().into(),
            subcommands: vec![
                SubcommandInfo {
                    name: "list".into(),
                    description: "list available models for the selected provider".into(),
                    usage: "/model list".into(),
                },
                SubcommandInfo {
                    name: "select".into(),
                    description: "select a model by id".into(),
                    usage: "/model select <model-id>".into(),
                },
            ],
            async_command: false,
        }
    }

    fn execute(&self, ctx: &mut CommandContext, args: &str) -> Result<CommandResult> {
        let parts: Vec<&str> = args.split_whitespace().collect();
        match parts.first().copied() {
            Some("list") | None => {
                if ctx.available_models.is_empty() {
                    let output = if let Some(pid) = ctx.selected_provider.as_deref() {
                        let needs_key = ctx
                            .providers
                            .iter()
                            .find(|p| p.id == pid)
                            .map(|p| p.requires_api_key)
                            .unwrap_or(false);
                        if needs_key {
                            format!(
                                "No models available. The '{pid}' provider requires an API key. \
                                 Run /provider configure {pid} to set it."
                            )
                        } else {
                            format!(
                                "No models available from '{pid}'. \
                                 Make sure the provider is running and accessible."
                            )
                        }
                    } else {
                        "No models available. Select a provider first with /provider.".into()
                    };
                    return Ok(CommandResult {
                        command: "model".into(),
                        output,
                        data: None,
                    });
                }
                let lines: Vec<String> = ctx
                    .available_models
                    .iter()
                    .map(|m| {
                        let sel = if Some(&m.id) == ctx.selected_model.as_ref() {
                            " [selected]"
                        } else {
                            ""
                        };
                        let label = if m.name == m.id {
                            m.id.clone()
                        } else {
                            format!("{} — {}", m.id, m.name)
                        };
                        let tier_tag = m
                            .tier
                            .as_ref()
                            .map(|t| format!(" [{t}]"))
                            .unwrap_or_default();
                        format!("  {label}{tier_tag}{sel}")
                    })
                    .collect();
                let output = format!("Available models:\n{}", lines.join("\n"));
                let data = serde_json::to_value(&ctx.available_models).ok();
                Ok(CommandResult {
                    command: "model".into(),
                    output,
                    data,
                })
            }
            Some("select") => {
                let model_id = parts
                    .get(1)
                    .ok_or_else(|| BimoError::Command("usage: /model select <model-id>".into()))?;
                let exists = ctx.available_models.iter().any(|m| m.id == *model_id);
                if !exists {
                    return Err(BimoError::Command(format!(
                        "model '{model_id}' not found. Use /model list to see available models."
                    )));
                }
                ctx.selected_model = Some(model_id.to_string());
                Ok(CommandResult {
                    command: "model".into(),
                    output: format!("Selected model: {model_id}"),
                    data: None,
                })
            }
            Some(other) => Err(BimoError::Command(format!(
                "unknown subcommand '{other}'. Usage: /model [list|select <id>]"
            ))),
        }
    }
}
