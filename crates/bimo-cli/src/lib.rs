//! Bimo CLI; a Clap-based command-line interface for the Bimo coding agent harness.
//!
//! # Entry point
//!
//! [`run`] is the crate's single entry point: it initializes tracing, parses
//! `std::env::args`, and executes the parsed [`Cli`] on its own (multi-threaded)
//! tokio runtime, built and torn down per invocation. It is used by the `bimo`
//! binary crate, which carries no clap setup of its own.
//!
//! Execution first applies the global `--config-dir` flag, then dispatches the
//! parsed [`Command`] to the matching handler in the private `handlers` module.
//!
//! # Command surface
//!
//! [`Cli`] carries two global flags — `--config-dir` (overrides the base
//! config directory) and `--json` (machine-readable output for list/show
//! commands) — plus an optional [`Command`]:
//!
//! - `tui` — launch the interactive TUI (the default when no subcommand is
//!   given); optionally load a session and pick a theme.
//! - `provider` — manage providers: list configured providers, search the
//!   models.dev catalogue and built-in local providers, add/remove/show,
//!   set the default provider, list a provider's models (auto-discovering for
//!   local providers), and re-fetch the models.dev cache.
//! - `model` — inspect model metadata from the models.dev registry: list
//!   models (optionally restricted to one provider) and show full metadata.
//! - `session` — create, list, show, delete, fork, clear, export (Markdown or
//!   JSON), rename, auto-title, undo/redo, and restore archived messages.
//! - `settings` — show, set, unset, or reset application settings.
//! - `tools` — list the built-in agent tools.
//! - `skills` — list skills discovered from the default skill directories
//!   (optionally within a project directory).
//! - `cleanup` — run session cleanup (expired/excess sessions) immediately,
//!   with optional TTL/max overrides.
//! - `config-path` — print the resolved config directory.
//!
//! # Modules
//!
//! - [`cli`] — the public Clap command-line definitions: [`Cli`], [`Command`],
//!   and their argument/value types (provider, model, session, tui, settings,
//!   tools, skills, cleanup).
//! - `handler` — the async dispatch logic behind [`run`].
//! - `handlers` — private per-domain implementations (agent, model, provider,
//!   session, settings, misc, tui).
//! - `output` — private helpers for JSON and human-readable output.
//!
//! The crate re-exports `bimo_core` and its [`Result`] type.

pub mod cli;
mod handler;
mod handlers;
mod output;

pub use bimo_core;
pub use bimo_core::error::Result;
pub use cli::{Cli, Command};

/// Initializes tracing, parses `std::env::args`, and executes the command.
///
/// This is the crate's single entry point, used by the `bimo` binary crate.
/// Bare `bimo` (no subcommand) launches the TUI.
pub fn run() -> Result<()> {
    use clap::Parser;
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .init();

    let cli = Cli::parse();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(crate::handler::run_async(&cli))
}
