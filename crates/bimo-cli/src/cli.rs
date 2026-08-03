//! Clap command-line interface definitions for Bimo.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

use bimo_core::ReasoningEffort;
use bimo_core::config::ApiFormat;

/// Bimo coding agent command-line interface.
#[derive(Debug, Parser)]
#[command(
    name = "bimo",
    version,
    about = "Bimo coding agent harness",
    long_about = "Bimo coding agent harness — configure providers and models, manage sessions, and run the agent."
)]
pub struct Cli {
    /// Base config directory (defaults to the platform config dir + `/bimo`).
    #[arg(long, global = true, value_name = "DIR")]
    pub config_dir: Option<PathBuf>,

    /// Emit machine-readable JSON for list/show commands.
    #[arg(long, global = true)]
    pub json: bool,

    /// When omitted, the TUI is launched (see the `tui` subcommand).
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Launch the interactive TUI.
    Tui(TuiArgs),
    /// Configure and inspect providers.
    Provider {
        #[command(subcommand)]
        sub: ProviderCommand,
    },
    /// Inspect model metadata from the models.dev registry.
    Model {
        #[command(subcommand)]
        sub: ModelCommand,
    },
    /// Manage agent sessions.
    Session {
        #[command(subcommand)]
        sub: SessionCommand,
    },
    /// Run the agent on a single prompt.
    Run(RunArgs),
    /// Read and update application settings.
    Settings {
        #[command(subcommand)]
        sub: SettingsCommand,
    },
    /// List built-in agent tools.
    Tools {
        #[command(subcommand)]
        sub: ToolsCommand,
    },
    /// List skills discovered from the default skill directories.
    Skills {
        #[command(subcommand)]
        sub: SkillsCommand,
    },
    /// Run session cleanup now (expired/excess sessions).
    Cleanup(CleanupArgs),
    /// Print the resolved config directory.
    ConfigPath,
}

// ---------------------------------------------------------------------------
// provider
// ---------------------------------------------------------------------------

#[derive(Debug, Subcommand)]
pub enum ProviderCommand {
    /// List configured providers.
    List,
    /// Search the models.dev catalogue and built-in local providers.
    Search {
        /// Optional query matched against provider id/name.
        query: Option<String>,
    },
    /// Add a provider to the configuration.
    Add(ProviderAddArgs),
    /// Remove a provider from the configuration.
    Remove {
        /// Provider id.
        id: String,
    },
    /// Show details of a configured provider.
    Show {
        /// Provider id or name.
        id: String,
    },
    /// Set the default provider.
    SetDefault {
        /// Provider id.
        id: String,
    },
    /// List the models available for a provider.
    Models {
        /// Provider id or name.
        id: String,
        /// Re-fetch the models.dev catalogue before listing.
        #[arg(long)]
        refresh: bool,
    },
    /// Re-fetch the models.dev catalogue into the local cache.
    Refresh,
}

#[derive(Debug, Args)]
pub struct ProviderAddArgs {
    /// Unique provider id (e.g. `ollama`).
    pub id: String,
    /// Human-readable display name (defaults to `id`).
    #[arg(long)]
    pub name: Option<String>,
    /// Provider kind: local or cloud.
    #[arg(long = "type", value_enum, default_value_t = ProviderTypeArg::Local)]
    pub provider_type: ProviderTypeArg,
    /// Base URL of the provider's API endpoint.
    #[arg(long)]
    pub base_url: Option<String>,
    /// Wire format expected by the endpoint.
    #[arg(long, value_enum, default_value_t = ApiFormatArg::OpenAICompatible)]
    pub api_format: ApiFormatArg,
    /// API key (cloud providers).
    #[arg(long)]
    pub api_key: Option<String>,
    /// Auto-discover models from a local provider's endpoint.
    #[arg(long)]
    pub discover: bool,
}

// ---------------------------------------------------------------------------
// model
// ---------------------------------------------------------------------------

#[derive(Debug, Subcommand)]
pub enum ModelCommand {
    /// List models from the models.dev registry.
    List {
        /// Restrict to a single provider id.
        provider: Option<String>,
        /// Re-fetch the models.dev catalogue before listing.
        #[arg(long)]
        refresh: bool,
    },
    /// Show full metadata for a single model.
    Show {
        /// Model id (e.g. `gpt-4o`).
        model_id: String,
    },
}

// ---------------------------------------------------------------------------
// session
// ---------------------------------------------------------------------------

#[derive(Debug, Subcommand)]
pub enum SessionCommand {
    /// Create a new session.
    New {
        /// Optional name stored in session metadata.
        #[arg(long)]
        name: Option<String>,
    },
    /// List sessions, most recently updated first.
    List,
    /// Show a session's messages and state.
    Show {
        /// Session id.
        id: String,
        /// Print full message contents (truncated otherwise).
        #[arg(long)]
        full: bool,
    },
    /// Delete a session.
    Delete {
        /// Session id.
        id: String,
    },
    /// Fork a session into a new independent copy.
    Fork {
        /// Session id.
        id: String,
    },
    /// Clear a session's messages.
    Clear {
        /// Session id.
        id: String,
    },
    /// Export a session to Markdown or JSON.
    Export {
        /// Session id.
        id: String,
        /// Export format.
        #[arg(long, value_enum)]
        format: ExportFormat,
        /// Output file (defaults to stdout).
        #[arg(long, value_name = "PATH")]
        output: Option<PathBuf>,
    },
    /// Store a display name in a session's metadata.
    Rename {
        /// Session id.
        id: String,
        /// New name.
        name: String,
    },
    /// Generate a session title from the conversation using the model.
    Title {
        /// Session id.
        id: String,
        #[command(flatten)]
        agent: AgentArgs,
    },
    /// Undo the last (or a specific) user prompt in a session.
    Undo {
        /// Session id.
        id: String,
        /// Target a specific user message by id.
        #[arg(long)]
        message_id: Option<String>,
    },
    /// Redo the last undone prompt in a session.
    Redo {
        /// Session id.
        id: String,
    },
    /// Restore archived messages back into a session.
    Restore {
        /// Session id.
        id: String,
        /// Restore only a single archived batch by index.
        #[arg(long)]
        batch: Option<usize>,
    },
    /// Resume a session by sending a new prompt and running the agent.
    Send {
        /// Session id.
        id: String,
        /// The user prompt to send.
        message: String,
        #[command(flatten)]
        agent: AgentArgs,
    },
}

// ---------------------------------------------------------------------------
// tui
// ---------------------------------------------------------------------------

#[derive(Debug, Args, Default)]
pub struct TuiArgs {
    /// Session ID to load (optional).
    #[arg(long)]
    pub session: Option<String>,
    /// Theme name to use.
    #[arg(long)]
    pub theme: Option<String>,
    /// List available themes and exit.
    #[arg(long)]
    pub list_themes: bool,
}

// ---------------------------------------------------------------------------
// run / shared agent options
// ---------------------------------------------------------------------------

#[derive(Debug, Args)]
pub struct RunArgs {
    /// The user prompt to run.
    pub prompt: String,
    /// Resume an existing session instead of creating a new one.
    #[arg(long, value_name = "ID")]
    pub session: Option<String>,
    /// Name stored in the new session's metadata.
    #[arg(long)]
    pub name: Option<String>,
    #[command(flatten)]
    pub agent: AgentArgs,
}

/// Options shared by `run` and `session send`/`title`.
#[derive(Debug, Args, Clone, Default)]
pub struct AgentArgs {
    /// Provider id/name (defaults to settings, then the configured default).
    #[arg(long)]
    pub provider: Option<String>,
    /// Model id (defaults to settings, then the provider's default).
    #[arg(long)]
    pub model: Option<String>,
    /// Project root used for instructions and filesystem snapshots.
    #[arg(long, value_name = "DIR")]
    pub project_dir: Option<PathBuf>,
    /// Sampling temperature (0.0–1.0).
    #[arg(long)]
    pub temperature: Option<f32>,
    /// Maximum output tokens per generation.
    #[arg(long)]
    pub max_tokens: Option<u32>,
    /// Maximum tool-call steps before the run stops.
    #[arg(long)]
    pub max_steps: Option<usize>,
    /// Model reasoning effort.
    #[arg(long, value_enum)]
    pub reasoning_effort: Option<ReasoningEffortArg>,
    /// Retry attempts per failed generation step.
    #[arg(long)]
    pub retry_attempts: Option<usize>,
    /// Delay in seconds between retries.
    #[arg(long)]
    pub retry_timeout: Option<u64>,
    /// Enable or disable filesystem snapshots for this run.
    #[arg(long)]
    pub snapshots: Option<bool>,
}

// ---------------------------------------------------------------------------
// settings
// ---------------------------------------------------------------------------

#[derive(Debug, Subcommand)]
pub enum SettingsCommand {
    /// Show the current settings.
    Show,
    /// Set a settings key to a value.
    Set {
        /// Setting key (e.g. `max_steps`, `debug`, `default_model`).
        key: String,
        /// Value for the key.
        value: String,
    },
    /// Clear an optional setting (default_provider, default_model).
    Unset {
        /// Setting key.
        key: String,
    },
    /// Reset settings to defaults.
    Reset,
}

// ---------------------------------------------------------------------------
// tools / skills / cleanup
// ---------------------------------------------------------------------------

#[derive(Debug, Subcommand)]
pub enum ToolsCommand {
    /// List built-in tools.
    List,
}

#[derive(Debug, Subcommand)]
pub enum SkillsCommand {
    /// List discovered skills.
    List {
        /// Project directory whose skills should be scanned.
        #[arg(long, value_name = "DIR")]
        project_dir: Option<PathBuf>,
    },
}

#[derive(Debug, Args)]
pub struct CleanupArgs {
    /// Override the session TTL (hours).
    #[arg(long)]
    pub ttl: Option<u64>,
    /// Override the maximum number of kept sessions.
    #[arg(long)]
    pub max: Option<usize>,
}

// ---------------------------------------------------------------------------
// value types
// ---------------------------------------------------------------------------

/// Provider kind used on the CLI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ProviderTypeArg {
    Local,
    Cloud,
}

/// API wire format used on the CLI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ApiFormatArg {
    #[value(name = "openai_compatible")]
    OpenAICompatible,
    #[value(name = "anthropic")]
    Anthropic,
    #[value(name = "openai")]
    OpenAI,
    #[value(name = "google")]
    Google,
}

impl From<ApiFormatArg> for ApiFormat {
    fn from(arg: ApiFormatArg) -> Self {
        match arg {
            ApiFormatArg::OpenAICompatible => ApiFormat::OpenAICompatible,
            ApiFormatArg::Anthropic => ApiFormat::Anthropic,
            ApiFormatArg::OpenAI => ApiFormat::OpenAI,
            ApiFormatArg::Google => ApiFormat::Google,
        }
    }
}

/// Reasoning effort used on the CLI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ReasoningEffortArg {
    Low,
    Medium,
    High,
}

impl From<ReasoningEffortArg> for ReasoningEffort {
    fn from(arg: ReasoningEffortArg) -> Self {
        match arg {
            ReasoningEffortArg::Low => ReasoningEffort::Low,
            ReasoningEffortArg::Medium => ReasoningEffort::Medium,
            ReasoningEffortArg::High => ReasoningEffort::High,
        }
    }
}

/// Export format for `session export`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ExportFormat {
    Md,
    Json,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Cli {
        Cli::try_parse_from(std::iter::once(&"bimo").chain(args)).unwrap()
    }

    #[test]
    fn parses_global_flags() {
        let cli = parse(&["--config-dir", "/tmp/cfg", "--json", "provider", "list"]);
        assert_eq!(
            cli.config_dir.as_deref(),
            Some(std::path::Path::new("/tmp/cfg"))
        );
        assert!(cli.json);
        assert!(matches!(
            cli.command,
            Some(Command::Provider {
                sub: ProviderCommand::List
            })
        ));
    }

    #[test]
    fn parses_bare_invocation() {
        let cli = parse(&[]);
        assert!(cli.command.is_none());
    }

    #[test]
    fn parses_tui_subcommand() {
        let cli = parse(&["tui", "--session", "abc", "--theme", "dark"]);
        let Some(Command::Tui(args)) = cli.command else {
            panic!("expected tui subcommand")
        };
        assert_eq!(args.session.as_deref(), Some("abc"));
        assert_eq!(args.theme.as_deref(), Some("dark"));
        assert!(!args.list_themes);
    }

    #[test]
    fn parses_tui_list_themes() {
        let cli = parse(&["tui", "--list-themes"]);
        let Some(Command::Tui(args)) = cli.command else {
            panic!("expected tui subcommand")
        };
        assert!(args.list_themes);
    }

    #[test]
    fn parses_provider_add() {
        let cli = parse(&[
            "provider",
            "add",
            "ollama",
            "--type",
            "local",
            "--api-format",
            "openai_compatible",
            "--discover",
        ]);
        let Some(Command::Provider {
            sub: ProviderCommand::Add(args),
        }) = cli.command
        else {
            panic!("expected provider add")
        };
        assert_eq!(args.id, "ollama");
        assert_eq!(args.provider_type, ProviderTypeArg::Local);
        assert_eq!(args.api_format, ApiFormatArg::OpenAICompatible);
        assert!(args.discover);
        assert!(matches!(
            ApiFormat::from(args.api_format),
            ApiFormat::OpenAICompatible
        ));
    }

    #[test]
    fn parses_provider_add_defaults() {
        let cli = parse(&["provider", "add", "anthropic"]);
        let Some(Command::Provider {
            sub: ProviderCommand::Add(args),
        }) = cli.command
        else {
            panic!("expected provider add")
        };
        assert_eq!(args.provider_type, ProviderTypeArg::Local);
        assert_eq!(args.api_format, ApiFormatArg::OpenAICompatible);
    }

    #[test]
    fn parses_settings_set() {
        let cli = parse(&["settings", "set", "max_steps", "40"]);
        let Some(Command::Settings {
            sub: SettingsCommand::Set { key, value },
        }) = cli.command
        else {
            panic!("expected settings set")
        };
        assert_eq!(key, "max_steps");
        assert_eq!(value, "40");
    }

    #[test]
    fn parses_run_with_agent_options() {
        let cli = parse(&[
            "run",
            "hello",
            "--provider",
            "ollama",
            "--model",
            "llama3",
            "--reasoning-effort",
            "high",
            "--max-steps",
            "10",
            "--name",
            "test",
        ]);
        let Some(Command::Run(args)) = cli.command else {
            panic!("expected run")
        };
        assert_eq!(args.prompt, "hello");
        assert_eq!(args.agent.provider.as_deref(), Some("ollama"));
        assert_eq!(args.agent.model.as_deref(), Some("llama3"));
        assert_eq!(args.agent.reasoning_effort, Some(ReasoningEffortArg::High));
        assert_eq!(args.agent.max_steps, Some(10));
        assert_eq!(args.name.as_deref(), Some("test"));
        assert!(matches!(
            ReasoningEffort::from(ReasoningEffortArg::Medium),
            ReasoningEffort::Medium
        ));
    }

    #[test]
    fn parses_session_send() {
        let cli = parse(&["session", "send", "abc", "continue", "--provider", "x"]);
        let Some(Command::Session {
            sub: SessionCommand::Send { id, message, .. },
        }) = cli.command
        else {
            panic!("expected session send")
        };
        assert_eq!(id, "abc");
        assert_eq!(message, "continue");
    }

    #[test]
    fn parses_session_export_format() {
        let cli = parse(&["session", "export", "abc", "--format", "json"]);
        let Some(Command::Session {
            sub: SessionCommand::Export { format, .. },
        }) = cli.command
        else {
            panic!("expected session export")
        };
        assert_eq!(format, ExportFormat::Json);
    }

    #[test]
    fn rejects_unknown_reasoning_effort() {
        assert!(
            Cli::try_parse_from(["bimo", "run", "hi", "--reasoning-effort", "extreme"]).is_err()
        );
    }
}
