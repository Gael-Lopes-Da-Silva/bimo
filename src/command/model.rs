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
                    return Ok(CommandResult {
                        command: "model".into(),
                        output: "No models available. Select a provider first with /provider."
                            .into(),
                        data: None,
                    });
                }
                let lines: Vec<String> = ctx
                    .available_models
                    .iter()
                    .map(|m| {
                        let sel = if Some(&m.id) == ctx.selected_model.as_ref() {
                            " *"
                        } else {
                            ""
                        };
                        format!("  {} — {}{sel}", m.id, m.name)
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
