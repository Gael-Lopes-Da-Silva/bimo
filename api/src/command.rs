use crate::error::{BimoError, Result};
use crate::session::SessionInfo;
use crate::tools::Tool;
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
// Command metadata — for autocompletion
// ---------------------------------------------------------------------------

/// Full metadata about a slash command, suitable for client autocompletion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandInfo {
    pub name: String,
    pub description: String,
    pub subcommands: Vec<SubcommandInfo>,
    #[serde(rename = "async")]
    pub async_command: bool,
}

/// Metadata about a subcommand (e.g. `/session list`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubcommandInfo {
    pub name: String,
    pub description: String,
    pub usage: String,
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

    /// Return full metadata for autocompletion. Override to add subcommands.
    fn command_info(&self) -> CommandInfo {
        CommandInfo {
            name: self.name().to_string(),
            description: self.description().to_string(),
            subcommands: vec![],
            async_command: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Async command trait — for commands that need await
// ---------------------------------------------------------------------------

/// A slash command that requires async execution.
pub trait AsyncSlashCommand: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn execute<'a>(
        &'a self,
        ctx: &'a mut CommandContext,
        args: &'a str,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<CommandResult>> + Send + 'a>,
    >;
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
    pub session_messages: Vec<crate::session::Message>,
    pub provider_ids: Vec<String>,
    pub provider_names: Vec<String>,
    pub needs_configuration: bool,

    // Tools
    pub tools: Vec<Tool>,

    // Session management
    pub saved_sessions: Vec<SessionInfo>,

    // Compaction — set to true by /compact command, agent handles the async work
    pub compact_requested: bool,

    // Provider runtime info (needed for compact to call the provider)
    pub has_runtime: bool,

    // Tree command post-actions
    pub tree_fork_index: Option<usize>,
    pub tree_revert_index: Option<usize>,
}

// ---------------------------------------------------------------------------
// Command registry
// ---------------------------------------------------------------------------

pub struct CommandRegistry {
    commands: HashMap<String, Box<dyn SlashCommand>>,
    async_commands: HashMap<String, Box<dyn AsyncSlashCommand>>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        let mut reg = Self {
            commands: HashMap::new(),
            async_commands: HashMap::new(),
        };
        // Register built-in sync commands
        reg.register(Box::new(HelpCommand));
        reg.register(Box::new(StatusCommand));
        reg.register(Box::new(ClearCommand));
        reg.register(Box::new(ModelCommand));
        reg.register(Box::new(ProviderCommand));
        reg.register(Box::new(ToolsCommand));
        reg.register(Box::new(SessionCommand));
        reg.register(Box::new(TreeCommand));
        // Register async commands
        reg.register_async(Box::new(CompactCommand));
        reg
    }

    pub fn register(&mut self, cmd: Box<dyn SlashCommand>) {
        self.commands.insert(cmd.name().to_string(), cmd);
    }

    pub fn register_async(&mut self, cmd: Box<dyn AsyncSlashCommand>) {
        self.async_commands.insert(cmd.name().to_string(), cmd);
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

        if let Some(cmd) = self.commands.get(cmd_name) {
            return cmd.execute(ctx, args);
        }
        if self.async_commands.contains_key(cmd_name) {
            return Err(BimoError::Command(format!(
                "command '/{cmd_name}' requires async dispatch"
            )));
        }
        Err(BimoError::Command(format!(
            "unknown command '/{cmd_name}'. Type /help to see available commands."
        )))
    }

    /// Dispatch an async slash command.
    pub async fn dispatch_async(
        &self,
        input: &str,
        ctx: &mut CommandContext,
    ) -> Result<CommandResult> {
        let input = input.trim();
        if !input.starts_with('/') {
            return Err(BimoError::Command("not a slash command".into()));
        }
        let rest = &input[1..];
        let parts: Vec<&str> = rest.splitn(2, ' ').collect();
        let cmd_name = parts[0];
        let args = parts.get(1).copied().unwrap_or("");

        if let Some(cmd) = self.commands.get(cmd_name) {
            return cmd.execute(ctx, args);
        }
        if let Some(cmd) = self.async_commands.get(cmd_name) {
            return cmd.execute(ctx, args).await;
        }
        Err(BimoError::Command(format!(
            "unknown command '/{cmd_name}'. Type /help to see available commands."
        )))
    }

    /// List all registered commands as (name, description) pairs.
    pub fn list(&self) -> Vec<(&str, &str)> {
        let mut cmds: Vec<_> = self
            .commands
            .values()
            .map(|c| (c.name(), c.description()))
            .collect();
        for cmd in self.async_commands.values() {
            cmds.push((cmd.name(), cmd.description()));
        }
        cmds.sort_by_key(|(name, _)| *name);
        cmds
    }

    /// List all commands with full metadata for autocompletion.
    pub fn list_detailed(&self) -> Vec<CommandInfo> {
        let mut cmds: Vec<CommandInfo> = self
            .commands
            .values()
            .map(|c| c.command_info())
            .collect();
        for cmd in self.async_commands.values() {
            cmds.push(CommandInfo {
                name: cmd.name().to_string(),
                description: cmd.description().to_string(),
                subcommands: vec![],
                async_command: true,
            });
        }
        cmds.sort_by(|a, b| a.name.cmp(&b.name));
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
        let _ = ctx;
        let output = "\
Available commands:

  /help       — list all available commands
  /status     — show current provider, model, and session info
  /provider   — list providers, select, or configure a provider
  /model      — list models, or select a model
  /clear      — clear the current conversation session
  /tools      — list all available agent tools
  /session    — manage sessions (list, save, resume, delete, info)
  /tree       — view conversation tree, fork or revert to a point
  /compact    — compact the session context into a summary
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

struct ProviderCommand;

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
                    name: "configure".into(),
                    description: "configure a provider's base URL and API key".into(),
                    usage: "/provider configure <provider-id>".into(),
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

// ---------------------------------------------------------------------------
// /tools
// ---------------------------------------------------------------------------

struct ToolsCommand;

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

        let output = format!("Available tools ({}):\n{}", ctx.tools.len(), lines.join("\n\n"));
        let data = serde_json::to_value(&ctx.tools).ok();

        Ok(CommandResult {
            command: "tools".into(),
            output,
            data,
        })
    }
}

// ---------------------------------------------------------------------------
// /session
// ---------------------------------------------------------------------------

struct SessionCommand;

impl SlashCommand for SessionCommand {
    fn name(&self) -> &str {
        "session"
    }

    fn description(&self) -> &str {
        "manage sessions"
    }

    fn command_info(&self) -> CommandInfo {
        CommandInfo {
            name: "session".into(),
            description: self.description().into(),
            subcommands: vec![
                SubcommandInfo {
                    name: "list".into(),
                    description: "list all saved sessions".into(),
                    usage: "/session list".into(),
                },
                SubcommandInfo {
                    name: "save".into(),
                    description: "save the current session to disk".into(),
                    usage: "/session save".into(),
                },
                SubcommandInfo {
                    name: "resume".into(),
                    description: "resume a saved session by id (supports prefix)".into(),
                    usage: "/session resume <session-id>".into(),
                },
                SubcommandInfo {
                    name: "delete".into(),
                    description: "delete a saved session".into(),
                    usage: "/session delete <session-id>".into(),
                },
                SubcommandInfo {
                    name: "info".into(),
                    description: "show current session details".into(),
                    usage: "/session info".into(),
                },
                SubcommandInfo {
                    name: "purge".into(),
                    description: "delete all saved sessions".into(),
                    usage: "/session purge".into(),
                },
            ],
            async_command: false,
        }
    }

    fn execute(&self, ctx: &mut CommandContext, args: &str) -> Result<CommandResult> {
        let parts: Vec<&str> = args.split_whitespace().collect();
        match parts.first().copied() {
            Some("list") | None => {
                if ctx.saved_sessions.is_empty() {
                    return Ok(CommandResult {
                        command: "session".into(),
                        output: "No saved sessions.".into(),
                        data: Some(serde_json::json!([])),
                    });
                }
                let lines: Vec<String> = ctx
                    .saved_sessions
                    .iter()
                    .map(|s| {
                        let active = if s.id == ctx.session_id {
                            " (active)"
                        } else {
                            ""
                        };
                        format!(
                            "  {} — {} messages, updated {}{active}",
                            &s.id[..8.min(s.id.len())],
                            s.message_count,
                            s.updated_at.format("%Y-%m-%d %H:%M UTC"),
                        )
                    })
                    .collect();
                let output = format!(
                    "Saved sessions ({}):\n{}",
                    ctx.saved_sessions.len(),
                    lines.join("\n")
                );
                let data = serde_json::to_value(&ctx.saved_sessions).ok();
                Ok(CommandResult {
                    command: "session".into(),
                    output,
                    data,
                })
            }
            Some("save") => {
                Ok(CommandResult {
                    command: "session".into(),
                    output: "Session saved.".into(),
                    data: None,
                })
            }
            Some("resume") => {
                let id = parts.get(1).ok_or_else(|| {
                    BimoError::Command("usage: /session resume <session-id>".into())
                })?;
                // Try full id match or prefix match
                let found = ctx
                    .saved_sessions
                    .iter()
                    .find(|s| s.id == *id || s.id.starts_with(id));
                match found {
                    Some(info) => Ok(CommandResult {
                        command: "session".into(),
                        output: format!(
                            "Resumed session {} ({} messages).",
                            &info.id[..8.min(info.id.len())],
                            info.message_count,
                        ),
                        data: Some(serde_json::json!({ "session_id": info.id })),
                    }),
                    None => Err(BimoError::Command(format!(
                        "session '{id}' not found. Use /session list."
                    ))),
                }
            }
            Some("delete") => {
                let id = parts.get(1).ok_or_else(|| {
                    BimoError::Command("usage: /session delete <session-id>".into())
                })?;
                let found = ctx
                    .saved_sessions
                    .iter()
                    .find(|s| s.id == *id || s.id.starts_with(id));
                match found {
                    Some(info) => Ok(CommandResult {
                        command: "session".into(),
                        output: format!("Deleted session {}.", &info.id[..8.min(info.id.len())]),
                        data: Some(serde_json::json!({ "session_id": info.id })),
                    }),
                    None => Err(BimoError::Command(format!(
                        "session '{id}' not found. Use /session list."
                    ))),
                }
            }
            Some("info") => {
                let output = format!(
                    "Session:     {}\n\
                     Messages:    {}\n\
                     Created:     {}\n\
                     Last active: {}",
                    ctx.session_id,
                    ctx.session_message_count,
                    "—", // timestamps handled at agent level
                    "—",
                );
                let data = serde_json::json!({
                    "session_id": ctx.session_id,
                    "message_count": ctx.session_message_count,
                });
                Ok(CommandResult {
                    command: "session".into(),
                    output,
                    data: Some(data),
                })
            }
            Some("purge") => {
                Ok(CommandResult {
                    command: "session".into(),
                    output: "All saved sessions purged.".into(),
                    data: None,
                })
            }
            Some(other) => Err(BimoError::Command(format!(
                "unknown subcommand '{other}'. Usage: /session [list|save|resume|delete|info|purge]"
            ))),
        }
    }
}

// ---------------------------------------------------------------------------
// /tree
// ---------------------------------------------------------------------------

struct TreeCommand;

impl SlashCommand for TreeCommand {
    fn name(&self) -> &str {
        "tree"
    }

    fn description(&self) -> &str {
        "view conversation tree and fork or revert"
    }

    fn command_info(&self) -> CommandInfo {
        CommandInfo {
            name: "tree".into(),
            description: self.description().into(),
            subcommands: vec![
                SubcommandInfo {
                    name: "fork".into(),
                    description: "fork to a new session at a message index".into(),
                    usage: "/tree fork <index>".into(),
                },
                SubcommandInfo {
                    name: "revert".into(),
                    description: "revert to a message index, erasing later context".into(),
                    usage: "/tree revert <index>".into(),
                },
            ],
            async_command: false,
        }
    }

    fn execute(&self, ctx: &mut CommandContext, args: &str) -> Result<CommandResult> {
        let parts: Vec<&str> = args.split_whitespace().collect();
        match parts.first().copied() {
            Some("fork") => {
                let index: usize = parts
                    .get(1)
                    .ok_or_else(|| BimoError::Command("usage: /tree fork <index>".into()))?
                    .parse()
                    .map_err(|_| BimoError::Command("index must be a number".into()))?;

                if index >= ctx.session_messages.len() {
                    return Err(BimoError::Command(format!(
                        "index {} out of range (session has {} messages)",
                        index,
                        ctx.session_messages.len()
                    )));
                }

                ctx.tree_fork_index = Some(index);

                let role = format_role(&ctx.session_messages[index].role);
                let preview = truncate(&ctx.session_messages[index].content, 40);

                Ok(CommandResult {
                    command: "tree".into(),
                    output: format!("Forking at message {index} [{role}]: {preview}..."),
                    data: Some(serde_json::json!({
                        "action": "fork",
                        "index": index,
                    })),
                })
            }
            Some("revert") => {
                let index: usize = parts
                    .get(1)
                    .ok_or_else(|| BimoError::Command("usage: /tree revert <index>".into()))?
                    .parse()
                    .map_err(|_| BimoError::Command("index must be a number".into()))?;

                if index >= ctx.session_messages.len() {
                    return Err(BimoError::Command(format!(
                        "index {} out of range (session has {} messages)",
                        index,
                        ctx.session_messages.len()
                    )));
                }

                let keep = index + 1;
                let discard = ctx.session_messages.len() - keep;

                ctx.tree_revert_index = Some(index);

                let role = format_role(&ctx.session_messages[index].role);
                let preview = truncate(&ctx.session_messages[index].content, 40);

                Ok(CommandResult {
                    command: "tree".into(),
                    output: format!(
                        "Reverting to message {index} [{role}]: {preview}... ({discard} messages will be removed)"
                    ),
                    data: Some(serde_json::json!({
                        "action": "revert",
                        "index": index,
                    })),
                })
            }
            // Default: show the tree
            None | Some(_) => {
                if ctx.session_messages.is_empty() {
                    return Ok(CommandResult {
                        command: "tree".into(),
                        output: "Session is empty.".into(),
                        data: Some(serde_json::json!({ "messages": [] })),
                    });
                }

                let mut lines: Vec<String> = Vec::new();
                for (i, msg) in ctx.session_messages.iter().enumerate() {
                    let role = format_role(&msg.role);
                    let preview = truncate(&msg.content, 60);
                    let marker = if i == ctx.session_messages.len() - 1 {
                        " <- latest"
                    } else {
                        ""
                    };
                    lines.push(format!("  {i:>3}  [{role}]  {preview}{marker}"));
                }

                let output = format!(
                    "Conversation tree ({} messages, session {}):\n{}\n\nUse /tree fork <index> or /tree revert <index>.",
                    ctx.session_messages.len(),
                    &ctx.session_id[..8.min(ctx.session_id.len())],
                    lines.join("\n")
                );

                Ok(CommandResult {
                    command: "tree".into(),
                    output,
                    data: Some(serde_json::json!({
                        "messages": ctx.session_messages.iter().enumerate().map(|(i, m)| {
                            serde_json::json!({
                                "index": i,
                                "role": format_role(&m.role),
                                "preview": truncate(&m.content, 60),
                            })
                        }).collect::<Vec<_>>(),
                    })),
                })
            }
        }
    }
}

fn format_role(role: &crate::session::Role) -> &'static str {
    match role {
        crate::session::Role::User => "user",
        crate::session::Role::Assistant => "asst",
        crate::session::Role::System => "sys",
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    let single_line: String = s.chars().filter(|c| *c != '\n').collect();
    if single_line.len() <= max_len {
        single_line
    } else {
        format!("{}...", &single_line[..max_len])
    }
}

// ---------------------------------------------------------------------------
// /compact (async)
// ---------------------------------------------------------------------------

struct CompactCommand;

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

            // Signal to the agent that compaction is needed
            ctx.compact_requested = true;

            Ok(CommandResult {
                command: "compact".into(),
                output: "Compacting session context...".into(),
                data: Some(serde_json::json!({ "status": "compacting" })),
            })
        })
    }
}
