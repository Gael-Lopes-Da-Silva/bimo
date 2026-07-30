use super::{CommandContext, CommandResult, SlashCommand};
use crate::error::Result;

pub(super) struct StatusCommand;

impl SlashCommand for StatusCommand {
    fn name(&self) -> &str {
        "status"
    }

    fn description(&self) -> &str {
        "show current provider, model, and session info"
    }

    fn execute(&self, ctx: &mut CommandContext, _args: &str) -> Result<CommandResult> {
        let provider = ctx
            .selected_provider
            .as_deref()
            .unwrap_or("(none — use /provider to configure)");
        let model = ctx
            .selected_model
            .as_deref()
            .unwrap_or("(none — use /model to select)");

        let output = format!(
            "Provider:   {provider}\n\
             Model:      {model}\n\
             Session:    {} ({} messages)\n\
             Configured: {}",
            ctx.session_id,
            ctx.session_message_count,
            if ctx.needs_configuration {
                "NO — run /provider to set up"
            } else {
                "yes"
            }
        );

        let data = serde_json::json!({
            "provider": ctx.selected_provider,
            "model": ctx.selected_model,
            "session_id": ctx.session_id,
            "message_count": ctx.session_message_count,
            "needs_configuration": ctx.needs_configuration,
        });

        Ok(CommandResult {
            command: "status".into(),
            output,
            data: Some(data),
        })
    }
}
