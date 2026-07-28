use super::{CommandContext, CommandResult, SlashCommand};
use crate::error::{BimoError, Result};

pub(super) struct ThinkingCommand;

impl SlashCommand for ThinkingCommand {
    fn name(&self) -> &str {
        "thinking"
    }

    fn description(&self) -> &str {
        "toggle or configure model thinking (on|off|budget <tokens>|effort <low|medium|high>)"
    }

    fn execute(&self, ctx: &mut CommandContext, args: &str) -> Result<CommandResult> {
        let args = args.trim();

        if args.is_empty() || args == "status" {
            let status = if ctx.thinking.enabled {
                let mut details = vec!["Thinking: ON".to_string()];
                if let Some(ref effort) = ctx.thinking.reasoning_effort {
                    details.push(format!("Reasoning effort: {}", effort));
                }
                if let Some(budget) = ctx.thinking.budget_tokens {
                    details.push(format!("Budget tokens: {}", budget));
                }
                details.join("\n")
            } else {
                "Thinking: OFF".to_string()
            };
            return Ok(CommandResult {
                command: "thinking".into(),
                output: status,
                data: None,
            });
        }

        let parts: Vec<&str> = args.splitn(2, ' ').collect();
        match parts[0] {
            "on" => {
                ctx.thinking.enabled = true;
                Ok(CommandResult {
                    command: "thinking".into(),
                    output: "Thinking enabled.".into(),
                    data: Some(serde_json::json!({ "thinking_enabled": true })),
                })
            }
            "off" => {
                ctx.thinking.enabled = false;
                Ok(CommandResult {
                    command: "thinking".into(),
                    output: "Thinking disabled.".into(),
                    data: Some(serde_json::json!({ "thinking_enabled": false })),
                })
            }
            "budget" => {
                let tokens = parts
                    .get(1)
                    .and_then(|s| s.parse::<u32>().ok())
                    .ok_or_else(|| {
                        BimoError::Command(
                            "usage: /thinking budget <tokens> (e.g. /thinking budget 10000)".into(),
                        )
                    })?;
                ctx.thinking.enabled = true;
                ctx.thinking.budget_tokens = Some(tokens);
                Ok(CommandResult {
                    command: "thinking".into(),
                    output: format!("Thinking enabled with budget of {} tokens.", tokens),
                    data: Some(
                        serde_json::json!({ "thinking_enabled": true, "budget_tokens": tokens }),
                    ),
                })
            }
            "effort" => {
                let effort = parts.get(1).ok_or_else(|| {
                    BimoError::Command("usage: /thinking effort <low|medium|high>".into())
                })?;
                if !matches!(*effort, "low" | "medium" | "high") {
                    return Err(BimoError::Command(
                        "reasoning effort must be low, medium, or high".into(),
                    ));
                }
                ctx.thinking.enabled = true;
                ctx.thinking.reasoning_effort = Some(effort.to_string());
                Ok(CommandResult {
                    command: "thinking".into(),
                    output: format!("Thinking enabled with reasoning effort: {}.", effort),
                    data: Some(
                        serde_json::json!({ "thinking_enabled": true, "reasoning_effort": effort }),
                    ),
                })
            }
            _ => Err(BimoError::Command(format!(
                "unknown subcommand '{}'. Usage: /thinking [on|off|budget <tokens>|effort <low|medium|high>]",
                parts[0]
            ))),
        }
    }
}
