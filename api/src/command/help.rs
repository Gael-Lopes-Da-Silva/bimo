use super::{CommandContext, CommandResult, SlashCommand};
use crate::error::Result;

pub(super) struct HelpCommand;

impl SlashCommand for HelpCommand {
    fn name(&self) -> &str {
        "help"
    }

    fn description(&self) -> &str {
        "list all available commands"
    }

    fn execute(&self, ctx: &mut CommandContext, _args: &str) -> Result<CommandResult> {
        let mut output = String::from("Available commands:\n");
        for (name, desc) in &ctx.command_descriptions {
            output.push_str(&format!("  /{:<16} {}\n", name, desc));
        }
        Ok(CommandResult {
            command: "help".into(),
            output: output.trim().into(),
            data: None,
        })
    }
}
