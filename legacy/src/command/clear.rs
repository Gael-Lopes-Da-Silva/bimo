use super::{CommandContext, CommandResult, SlashCommand};
use crate::error::Result;

pub(super) struct ClearCommand;

impl SlashCommand for ClearCommand {
    fn name(&self) -> &str {
        "clear"
    }

    fn description(&self) -> &str {
        "clear the current conversation session"
    }

    fn execute(&self, ctx: &mut CommandContext, _args: &str) -> Result<CommandResult> {
        let _ = ctx;
        Ok(CommandResult {
            command: "clear".into(),
            output: "Session cleared. The next message will start a fresh conversation.".into(),
            data: None,
        })
    }
}
