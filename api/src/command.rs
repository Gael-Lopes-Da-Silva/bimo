use crate::error::{BimoError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Command result — what a command returns to the caller
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResult {
    pub command: String,
    pub output: String,
    pub data: Option<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// Command trait
// ---------------------------------------------------------------------------

/// A slash command that can be registered in the command registry.
///
/// Commands receive a mutable reference to the [`CommandContext`] which gives
/// access to the agent state, and return a [`CommandResult`].
pub trait SlashCommand: Send + Sync {
    /// The single-word command name (without the leading `/`).
    fn name(&self) -> &str;

    /// Short description shown in `/help`.
    fn description(&self) -> &str;

    /// Execute the command. `args` is everything after the command word.
    fn execute(&self, ctx: &mut CommandContext, args: &str) -> Result<CommandResult>;
}

// ---------------------------------------------------------------------------
// Command context — provides access to the agent state
// ---------------------------------------------------------------------------

/// Mutable state exposed to commands.
pub struct CommandContext {
    pub selected_provider: Option<String>,
    pub selected_model: Option<String>,
    pub available_models: Vec<crate::model::ModelInfo>,
    pub session_id: String,
    pub session_message_count: usize,
    pub provider_ids: Vec<String>,
    pub provider_names: Vec<String>,
    pub needs_configuration: bool,
}

// ---------------------------------------------------------------------------
// Command registry
// ---------------------------------------------------------------------------

pub struct CommandRegistry {
    commands: HashMap<String, Box<dyn SlashCommand>>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        let mut reg = Self {
            commands: HashMap::new(),
        };
        // Register built-in commands
        reg.register(Box::new(HelpCommand));
        reg.register(Box::new(StatusCommand));
        reg.register(Box::new(ClearCommand));
        reg.register(Box::new(ModelCommand));
        reg.register(Box::new(ProviderCommand));
        reg
    }

    pub fn register(&mut self, cmd: Box<dyn SlashCommand>) {
        self.commands.insert(cmd.name().to_string(), cmd);
    }

    /// Dispatch a slash command string (e.g. "/help" or "/model select gpt-4").
    pub fn dispatch(&self, input: &str, ctx: &mut CommandContext) -> Result<CommandResult> {
        let input = input.trim();
        if !input.starts_with('/') {
            return Err(BimoError::Command("not a slash command".into()));
        }
        let rest = &input[1..]; // strip the leading `/`
        let parts: Vec<&str> = rest.splitn(2, ' ').collect();
        let cmd_name = parts[0];
        let args = parts.get(1).copied().unwrap_or("");

        match self.commands.get(cmd_name) {
            Some(cmd) => cmd.execute(ctx, args),
            None => Err(BimoError::Command(format!(
                "unknown command '/{cmd_name}'. Type /help to see available commands."
            ))),
        }
    }

    /// List all registered commands as (name, description) pairs.
    pub fn list(&self) -> Vec<(&str, &str)> {
        let mut cmds: Vec<_> = self
            .commands
            .values()
            .map(|c| (c.name(), c.description()))
            .collect();
        cmds.sort_by_key(|(name, _)| *name);
        cmds
    }
}

// ---------------------------------------------------------------------------
// Built-in commands
// ---------------------------------------------------------------------------

struct HelpCommand;

impl SlashCommand for HelpCommand {
    fn name(&self) -> &str {
        "help"
    }

    fn description(&self) -> &str {
        "list all available commands"
    }

    fn execute(&self, ctx: &mut CommandContext, _args: &str) -> Result<CommandResult> {
        // Help is special: it reads from the registry, but since we don't have
        // the registry here, we build a static list from the known commands.
        // This is OK for the built-in set; custom commands would need the registry.
        let _ = ctx;
        let output = "\
Available commands:

  /help       — list all available commands
  /status     — show current provider, model, and session info
  /provider   — list providers, select, or configure a provider
  /model      — list models, or select a model
  /clear      — clear the current conversation session
";
        Ok(CommandResult {
            command: "help".into(),
            output: output.trim().into(),
            data: None,
        })
    }
}

struct StatusCommand;

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

struct ClearCommand;

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

struct ModelCommand;

impl SlashCommand for ModelCommand {
    fn name(&self) -> &str {
        "model"
    }

    fn description(&self) -> &str {
        "list models or select a model (/model select <id> or /model list)"
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

struct ProviderCommand;

impl SlashCommand for ProviderCommand {
    fn name(&self) -> &str {
        "provider"
    }

    fn description(&self) -> &str {
        "list, select, or configure providers (/provider list|select|configure)"
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
                // This is a simplified version — the full configure flow is
                // handled at the API layer where we can do async work.
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
