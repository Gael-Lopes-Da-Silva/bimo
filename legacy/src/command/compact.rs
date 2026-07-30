use super::{AsyncSlashCommand, CommandContext, CommandResult};
use crate::error::{BimoError, Result};

pub(super) struct CompactCommand;

impl AsyncSlashCommand for CompactCommand {
    fn name(&self) -> &str {
        "compact"
    }

    fn description(&self) -> &str {
        "compact the session context into a summary"
    }

    fn execute<'a>(
        &'a self,
        ctx: &'a mut CommandContext,
        _args: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<CommandResult>> + Send + 'a>>
    {
        Box::pin(async move {
            if ctx.session_message_count == 0 {
                return Ok(CommandResult {
                    command: "compact".into(),
                    output: "Session is already empty, nothing to compact.".into(),
                    data: None,
                });
            }

            if !ctx.has_runtime {
                return Err(BimoError::Command(
                    "no provider selected. Select a provider first with /provider.".into(),
                ));
            }

            ctx.compact_requested = true;

            Ok(CommandResult {
                command: "compact".into(),
                output: "Compacting session context...".into(),
                data: Some(serde_json::json!({ "status": "compacting" })),
            })
        })
    }
}
