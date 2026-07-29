pub mod clear;
pub mod compact;
pub mod help;
pub mod model;
pub mod provider;
pub mod session;
pub mod status;
pub mod thinking;
pub mod todo;
pub mod tools;
pub mod tree;

use crate::config::ThinkingConfig;
use crate::error::{BimoError, Result};
use crate::model::ModelInfo;
use crate::session::SessionInfo;
use crate::todo::TodoList;
use crate::tool::Tool;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing;

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

pub trait SlashCommand: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn execute(&self, ctx: &mut CommandContext, args: &str) -> Result<CommandResult>;

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

pub trait AsyncSlashCommand: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn execute<'a>(
        &'a self,
        ctx: &'a mut CommandContext,
        args: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<CommandResult>> + Send + 'a>>;
}

// ---------------------------------------------------------------------------
// Command context — provides access to the agent state
// ---------------------------------------------------------------------------

pub struct CommandContext {
    pub selected_provider: Option<String>,
    pub selected_model: Option<String>,
    pub available_models: Vec<ModelInfo>,
    pub session_id: String,
    pub session_message_count: usize,
    pub session_messages: Vec<crate::session::Message>,
    pub provider_ids: Vec<String>,
    pub provider_names: Vec<String>,
    pub needs_configuration: bool,
    pub tools: Vec<Tool>,
    pub command_descriptions: Vec<(String, String)>,
    pub saved_sessions: Vec<SessionInfo>,
    pub compact_requested: bool,
    pub has_runtime: bool,
    pub tree_fork_index: Option<usize>,
    pub tree_revert_index: Option<usize>,
    pub thinking: ThinkingConfig,
    pub todos: TodoList,
    // Multi-session support
    pub active_session_id: String,
    pub all_sessions: Vec<SessionInfo>,
    pub switch_session_id: Option<String>,
}

// ---------------------------------------------------------------------------
// Command registry
// ---------------------------------------------------------------------------

pub struct CommandRegistry {
    commands: HashMap<String, Box<dyn SlashCommand>>,
    async_commands: HashMap<String, Box<dyn AsyncSlashCommand>>,
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandRegistry {
    pub fn new() -> Self {
        let mut reg = Self {
            commands: HashMap::new(),
            async_commands: HashMap::new(),
        };
        reg.register(Box::new(help::HelpCommand));
        reg.register(Box::new(status::StatusCommand));
        reg.register(Box::new(clear::ClearCommand));
        reg.register(Box::new(model::ModelCommand));
        reg.register(Box::new(provider::ProviderCommand));
        reg.register(Box::new(tools::ToolsCommand));
        reg.register(Box::new(session::SessionCommand));
        reg.register(Box::new(tree::TreeCommand));
        reg.register(Box::new(thinking::ThinkingCommand));
        reg.register(Box::new(todo::TodoCommand));
        reg.register_async(Box::new(compact::CompactCommand));
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
        let rest = &input[1..];
        let parts: Vec<&str> = rest.splitn(2, ' ').collect();
        let cmd_name = parts[0];
        let args = parts.get(1).copied().unwrap_or("");

        tracing::debug!(command = cmd_name, args, "dispatching sync command");

        if let Some(cmd) = self.commands.get(cmd_name) {
            let result = cmd.execute(ctx, args);
            match &result {
                Ok(r) => tracing::debug!(
                    command = cmd_name,
                    output_len = r.output.len(),
                    "sync command executed"
                ),
                Err(e) => tracing::warn!(command = cmd_name, error = %e, "sync command failed"),
            }
            return result;
        }
        if self.async_commands.contains_key(cmd_name) {
            return Err(BimoError::Command(format!(
                "command '/{cmd_name}' requires async dispatch"
            )));
        }
        tracing::warn!(command = cmd_name, "unknown command");
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

        tracing::debug!(command = cmd_name, args, "dispatching async command");

        if let Some(cmd) = self.commands.get(cmd_name) {
            let result = cmd.execute(ctx, args);
            match &result {
                Ok(r) => tracing::debug!(
                    command = cmd_name,
                    output_len = r.output.len(),
                    "command executed"
                ),
                Err(e) => tracing::warn!(command = cmd_name, error = %e, "command failed"),
            }
            return result;
        }
        if let Some(cmd) = self.async_commands.get(cmd_name) {
            let result = cmd.execute(ctx, args).await;
            match &result {
                Ok(r) => tracing::debug!(
                    command = cmd_name,
                    output_len = r.output.len(),
                    "async command executed"
                ),
                Err(e) => tracing::warn!(command = cmd_name, error = %e, "async command failed"),
            }
            return result;
        }
        tracing::warn!(command = cmd_name, "unknown command");
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
        let mut cmds: Vec<CommandInfo> = self.commands.values().map(|c| c.command_info()).collect();
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
// Helpers
// ---------------------------------------------------------------------------

pub(crate) fn format_role(role: &crate::session::Role) -> &'static str {
    match role {
        crate::session::Role::User => "user",
        crate::session::Role::Assistant => "asst",
        crate::session::Role::System => "sys",
        crate::session::Role::Tool => "tool",
    }
}

pub(crate) fn truncate(s: &str, max_len: usize) -> String {
    let single_line: String = s.chars().filter(|c| *c != '\n').collect();
    let char_count = single_line.chars().count();
    if char_count <= max_len {
        single_line
    } else {
        let truncated: String = single_line.chars().take(max_len).collect();
        format!("{}...", truncated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ModelInfo;
    use crate::todo::TodoList;
    use crate::tool::{Tool, ToolParameter};

    fn make_context() -> CommandContext {
        CommandContext {
            selected_provider: Some("openai".into()),
            selected_model: Some("gpt-4".into()),
            available_models: vec![ModelInfo {
                id: "gpt-4".into(),
                name: "GPT-4".into(),
                provider_id: "openai".into(),
                tier: None,
                context_window: Some(8_192),
            }],
            session_id: "test-session-id".into(),
            session_message_count: 5,
            session_messages: vec![],
            provider_ids: vec!["openai".into(), "anthropic".into(), "ollama".into()],
            provider_names: vec!["OpenAI".into(), "Anthropic".into(), "Ollama".into()],
            needs_configuration: false,
            tools: vec![Tool {
                name: "read_file".into(),
                description: "Read a file".into(),
                parameters: vec![ToolParameter {
                    name: "path".into(),
                    description: "file path".into(),
                    required: true,
                    parameter_type: "string".into(),
                }],
            }],
            command_descriptions: vec![
                ("help".into(), "list all available commands".into()),
                ("status".into(), "show current provider and model".into()),
            ],
            saved_sessions: vec![],
            compact_requested: false,
            has_runtime: true,
            tree_fork_index: None,
            tree_revert_index: None,
            thinking: ThinkingConfig::default(),
            todos: TodoList::new(),
            active_session_id: "test-session-id".into(),
            all_sessions: vec![],
            switch_session_id: None,
        }
    }

    #[test]
    fn registry_has_all_builtin_commands() {
        let reg = CommandRegistry::new();
        let names: Vec<&str> = reg.list().iter().map(|(n, _)| *n).collect();
        assert!(names.contains(&"help"));
        assert!(names.contains(&"status"));
        assert!(names.contains(&"clear"));
        assert!(names.contains(&"model"));
        assert!(names.contains(&"provider"));
        assert!(names.contains(&"tools"));
        assert!(names.contains(&"session"));
        assert!(names.contains(&"tree"));
        assert!(names.contains(&"compact"));
        assert!(names.contains(&"todo"));
    }

    #[test]
    fn dispatch_help() {
        let reg = CommandRegistry::new();
        let mut ctx = make_context();
        let result = reg.dispatch("/help", &mut ctx).unwrap();
        assert_eq!(result.command, "help");
        assert!(result.output.contains("/help"));
        assert!(result.output.contains("/status"));
    }

    #[test]
    fn dispatch_status() {
        let reg = CommandRegistry::new();
        let mut ctx = make_context();
        let result = reg.dispatch("/status", &mut ctx).unwrap();
        assert_eq!(result.command, "status");
        assert!(result.output.contains("openai"));
        assert!(result.output.contains("gpt-4"));
        assert!(result.data.is_some());
    }

    #[test]
    fn dispatch_clear() {
        let reg = CommandRegistry::new();
        let mut ctx = make_context();
        let result = reg.dispatch("/clear", &mut ctx).unwrap();
        assert_eq!(result.command, "clear");
        assert!(result.output.contains("cleared"));
    }

    #[test]
    fn dispatch_model_list() {
        let reg = CommandRegistry::new();
        let mut ctx = make_context();
        let result = reg.dispatch("/model", &mut ctx).unwrap();
        assert_eq!(result.command, "model");
        assert!(result.output.contains("gpt-4"));
    }

    #[test]
    fn dispatch_model_select() {
        let reg = CommandRegistry::new();
        let mut ctx = make_context();
        let _result = reg.dispatch("/model select gpt-4", &mut ctx).unwrap();
        assert_eq!(ctx.selected_model.as_deref(), Some("gpt-4"));
    }

    #[test]
    fn dispatch_model_select_unknown() {
        let reg = CommandRegistry::new();
        let mut ctx = make_context();
        let result = reg.dispatch("/model select unknown", &mut ctx);
        assert!(result.is_err());
    }

    #[test]
    fn dispatch_provider_list() {
        let reg = CommandRegistry::new();
        let mut ctx = make_context();
        let result = reg.dispatch("/provider", &mut ctx).unwrap();
        assert_eq!(result.command, "provider");
        assert!(result.output.contains("openai"));
        assert!(result.output.contains("anthropic"));
    }

    #[test]
    fn dispatch_provider_select() {
        let reg = CommandRegistry::new();
        let mut ctx = make_context();
        let _result = reg
            .dispatch("/provider select anthropic", &mut ctx)
            .unwrap();
        assert_eq!(ctx.selected_provider.as_deref(), Some("anthropic"));
    }

    #[test]
    fn dispatch_provider_select_unknown() {
        let reg = CommandRegistry::new();
        let mut ctx = make_context();
        let result = reg.dispatch("/provider select nonexistent", &mut ctx);
        assert!(result.is_err());
    }

    #[test]
    fn dispatch_tools() {
        let reg = CommandRegistry::new();
        let mut ctx = make_context();
        let result = reg.dispatch("/tools", &mut ctx).unwrap();
        assert_eq!(result.command, "tools");
        assert!(result.output.contains("read_file"));
        assert!(result.data.is_some());
    }

    #[test]
    fn dispatch_unknown_command() {
        let reg = CommandRegistry::new();
        let mut ctx = make_context();
        let result = reg.dispatch("/nonexistent", &mut ctx);
        assert!(result.is_err());
    }

    #[test]
    fn dispatch_not_slash_command() {
        let reg = CommandRegistry::new();
        let mut ctx = make_context();
        let result = reg.dispatch("help", &mut ctx);
        assert!(result.is_err());
    }

    #[test]
    fn dispatch_async_requires_async() {
        let reg = CommandRegistry::new();
        let mut ctx = make_context();
        let result = reg.dispatch("/compact", &mut ctx);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("requires async dispatch")
        );
    }

    #[test]
    fn list_detailed_includes_metadata() {
        let reg = CommandRegistry::new();
        let detailed = reg.list_detailed();
        assert!(!detailed.is_empty());

        let help = detailed.iter().find(|c| c.name == "help").unwrap();
        assert!(!help.async_command);
        assert!(help.subcommands.is_empty());

        let compact = detailed.iter().find(|c| c.name == "compact").unwrap();
        assert!(compact.async_command);

        let model = detailed.iter().find(|c| c.name == "model").unwrap();
        assert!(!model.subcommands.is_empty());
    }

    #[test]
    fn session_command_list() {
        let reg = CommandRegistry::new();
        let mut ctx = make_context();
        ctx.all_sessions = vec![];
        ctx.saved_sessions = vec![];
        let result = reg.dispatch("/session list", &mut ctx).unwrap();
        assert_eq!(result.command, "session");
        assert!(result.output.contains("No active sessions"));
    }

    #[test]
    fn session_command_info() {
        let reg = CommandRegistry::new();
        let mut ctx = make_context();
        let result = reg.dispatch("/session info", &mut ctx).unwrap();
        assert!(result.output.contains("test-session-id"));
        assert!(result.output.contains("5"));
    }

    #[test]
    fn tree_command_empty_session() {
        let reg = CommandRegistry::new();
        let mut ctx = make_context();
        ctx.session_messages = vec![];
        let result = reg.dispatch("/tree", &mut ctx).unwrap();
        assert!(result.output.contains("empty"));
    }

    #[test]
    fn tree_command_with_messages() {
        use crate::session::{Message, Role};
        use chrono::Utc;

        let reg = CommandRegistry::new();
        let mut ctx = make_context();
        ctx.session_messages = vec![
            Message {
                role: Role::User,
                content: "hello".into(),
                timestamp: Utc::now(),
            },
            Message {
                role: Role::Assistant,
                content: "hi there".into(),
                timestamp: Utc::now(),
            },
        ];
        let result = reg.dispatch("/tree", &mut ctx).unwrap();
        assert!(result.output.contains("2 messages"));
        assert!(result.output.contains("user"));
        assert!(result.output.contains("asst"));
    }

    #[test]
    fn tree_fork_sets_index() {
        use crate::session::{Message, Role};
        use chrono::Utc;

        let reg = CommandRegistry::new();
        let mut ctx = make_context();
        ctx.session_messages = vec![
            Message {
                role: Role::User,
                content: "a".into(),
                timestamp: Utc::now(),
            },
            Message {
                role: Role::Assistant,
                content: "b".into(),
                timestamp: Utc::now(),
            },
        ];
        let result = reg.dispatch("/tree fork 0", &mut ctx).unwrap();
        assert_eq!(ctx.tree_fork_index, Some(0));
        let data = result.data.unwrap();
        assert_eq!(data["action"].as_str().unwrap(), "fork");
        assert_eq!(data["index"].as_u64().unwrap(), 0);
    }

    #[test]
    fn tree_revert_sets_index() {
        use crate::session::{Message, Role};
        use chrono::Utc;

        let reg = CommandRegistry::new();
        let mut ctx = make_context();
        ctx.session_messages = vec![
            Message {
                role: Role::User,
                content: "a".into(),
                timestamp: Utc::now(),
            },
            Message {
                role: Role::Assistant,
                content: "b".into(),
                timestamp: Utc::now(),
            },
        ];
        let result = reg.dispatch("/tree revert 0", &mut ctx).unwrap();
        assert_eq!(ctx.tree_revert_index, Some(0));
        let data = result.data.unwrap();
        assert_eq!(data["action"].as_str().unwrap(), "revert");
        assert_eq!(data["index"].as_u64().unwrap(), 0);
    }

    #[test]
    fn tree_fork_out_of_range() {
        use crate::session::{Message, Role};
        use chrono::Utc;

        let reg = CommandRegistry::new();
        let mut ctx = make_context();
        ctx.session_messages = vec![Message {
            role: Role::User,
            content: "a".into(),
            timestamp: Utc::now(),
        }];
        let result = reg.dispatch("/tree fork 5", &mut ctx);
        assert!(result.is_err());
    }

    #[test]
    fn thinking_command_status_off_by_default() {
        let reg = CommandRegistry::new();
        let mut ctx = make_context();
        let result = reg.dispatch("/thinking", &mut ctx).unwrap();
        assert!(result.output.contains("OFF"));
    }

    #[test]
    fn thinking_command_turn_on() {
        let reg = CommandRegistry::new();
        let mut ctx = make_context();
        let result = reg.dispatch("/thinking on", &mut ctx).unwrap();
        assert!(result.output.contains("enabled"));
        assert!(ctx.thinking.enabled);
    }

    #[test]
    fn thinking_command_turn_off() {
        let reg = CommandRegistry::new();
        let mut ctx = make_context();
        ctx.thinking.enabled = true;
        let result = reg.dispatch("/thinking off", &mut ctx).unwrap();
        assert!(result.output.contains("disabled"));
        assert!(!ctx.thinking.enabled);
    }

    #[test]
    fn thinking_command_budget() {
        let reg = CommandRegistry::new();
        let mut ctx = make_context();
        let result = reg.dispatch("/thinking budget 15000", &mut ctx).unwrap();
        assert!(result.output.contains("15000"));
        assert!(ctx.thinking.enabled);
        assert_eq!(ctx.thinking.budget_tokens, Some(15000));
    }

    #[test]
    fn thinking_command_effort() {
        let reg = CommandRegistry::new();
        let mut ctx = make_context();
        let result = reg.dispatch("/thinking effort high", &mut ctx).unwrap();
        assert!(result.output.contains("high"));
        assert!(ctx.thinking.enabled);
        assert_eq!(ctx.thinking.reasoning_effort.as_deref(), Some("high"));
    }

    #[test]
    fn thinking_command_effort_invalid() {
        let reg = CommandRegistry::new();
        let mut ctx = make_context();
        let result = reg.dispatch("/thinking effort extreme", &mut ctx);
        assert!(result.is_err());
    }

    #[test]
    fn thinking_command_budget_invalid() {
        let reg = CommandRegistry::new();
        let mut ctx = make_context();
        let result = reg.dispatch("/thinking budget abc", &mut ctx);
        assert!(result.is_err());
    }

    #[test]
    fn thinking_command_unknown_subcommand() {
        let reg = CommandRegistry::new();
        let mut ctx = make_context();
        let result = reg.dispatch("/thinking foobar", &mut ctx);
        assert!(result.is_err());
    }

    #[test]
    fn todo_command_list_empty() {
        let reg = CommandRegistry::new();
        let mut ctx = make_context();
        let result = reg.dispatch("/todo", &mut ctx).unwrap();
        assert_eq!(result.command, "todo");
        assert!(result.output.contains("No todos"));
    }

    #[test]
    fn todo_command_clear() {
        let reg = CommandRegistry::new();
        let mut ctx = make_context();
        ctx.todos.add("Task 1");
        ctx.todos.add("Task 2");
        let result = reg.dispatch("/todo clear", &mut ctx).unwrap();
        assert!(result.output.contains("Cleared 2"));
        assert!(ctx.todos.is_empty());
    }

    #[test]
    fn todo_command_done() {
        let reg = CommandRegistry::new();
        let mut ctx = make_context();
        ctx.todos.add("Task 1");
        let result = reg.dispatch("/todo done 1", &mut ctx).unwrap();
        assert!(result.output.contains("done"));
        assert_eq!(
            ctx.todos.get(1).unwrap().status,
            crate::todo::TodoStatus::Done
        );
    }

    #[test]
    fn todo_command_done_not_found() {
        let reg = CommandRegistry::new();
        let mut ctx = make_context();
        let result = reg.dispatch("/todo done 99", &mut ctx);
        assert!(result.is_err());
    }

    #[test]
    fn todo_command_progress() {
        let reg = CommandRegistry::new();
        let mut ctx = make_context();
        ctx.todos.add("Task 1");
        let result = reg.dispatch("/todo progress 1", &mut ctx).unwrap();
        assert!(result.output.contains("in progress"));
        assert_eq!(
            ctx.todos.get(1).unwrap().status,
            crate::todo::TodoStatus::InProgress
        );
    }

    #[test]
    fn todo_command_pending() {
        let reg = CommandRegistry::new();
        let mut ctx = make_context();
        ctx.todos.add("Task 1");
        ctx.todos.update_status(1, crate::todo::TodoStatus::Done);
        let result = reg.dispatch("/todo pending 1", &mut ctx).unwrap();
        assert!(result.output.contains("pending"));
        assert_eq!(
            ctx.todos.get(1).unwrap().status,
            crate::todo::TodoStatus::Pending
        );
    }

    #[test]
    fn todo_command_list_with_items() {
        let reg = CommandRegistry::new();
        let mut ctx = make_context();
        ctx.todos.add("Task 1");
        ctx.todos.add("Task 2");
        ctx.todos.update_status(1, crate::todo::TodoStatus::Done);
        let result = reg.dispatch("/todo", &mut ctx).unwrap();
        assert!(result.output.contains("Task 1"));
        assert!(result.output.contains("Task 2"));
        assert!(result.output.contains("2 todos"));
    }

    #[test]
    fn todo_command_unknown_subcommand() {
        let reg = CommandRegistry::new();
        let mut ctx = make_context();
        let result = reg.dispatch("/todo foobar", &mut ctx);
        assert!(result.is_err());
    }

    #[test]
    fn todo_command_done_invalid_id() {
        let reg = CommandRegistry::new();
        let mut ctx = make_context();
        let result = reg.dispatch("/todo done abc", &mut ctx);
        assert!(result.is_err());
    }
}
