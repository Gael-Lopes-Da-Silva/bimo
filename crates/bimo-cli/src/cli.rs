//! Clap command-line interface definitions for Bimo.

use std::path::PathBuf;

use bimo_core::ReasoningEffort;
use bimo_core::config::ApiFormat;
use clap::{Args, Parser, Subcommand, ValueEnum};

/// Bimo coding agent command-line interface.
#[derive(Debug, Parser)]
#[command(
    name = "bimo",
    version,
    about = "Bimo coding agent harness",
    long_about = "Bimo coding agent harness; configure providers and models, manage sessions, and inspect tools and skills."
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
// shared agent options
// ---------------------------------------------------------------------------

/// Options shared by model-backed commands (`session title`).
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
