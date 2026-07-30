use super::{CommandContext, CommandResult, SlashCommand};
use crate::error::Result;

pub(super) struct ToolsCommand;

impl SlashCommand for ToolsCommand {
    fn name(&self) -> &str {
        "tools"
    }

    fn description(&self) -> &str {
        "list all available agent tools"
    }

    fn execute(&self, ctx: &mut CommandContext, _args: &str) -> Result<CommandResult> {
        if ctx.tools.is_empty() {
            return Ok(CommandResult {
                command: "tools".into(),
                output: "No tools registered.".into(),
                data: Some(serde_json::json!([])),
            });
        }

        let lines: Vec<String> = ctx
            .tools
            .iter()
            .map(|t| {
                let params: Vec<String> = t
                    .parameters
                    .iter()
                    .map(|p| {
                        let req = if p.required { " *" } else { "" };
                        format!("    {} ({}){req}", p.name, p.parameter_type)
                    })
                    .collect();
                let param_str = if params.is_empty() {
                    String::new()
                } else {
                    format!("\n  Parameters:\n{}", params.join("\n"))
                };
                format!("  {} — {}{param_str}", t.name, t.description)
            })
            .collect();

        let output = format!(
            "Available tools ({}):\n{}",
            ctx.tools.len(),
            lines.join("\n\n")
        );
        let data = serde_json::to_value(&ctx.tools).ok();

        Ok(CommandResult {
            command: "tools".into(),
            output,
            data,
        })
    }
}
