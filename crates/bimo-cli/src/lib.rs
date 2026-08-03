//! Bimo CLI library — a Clap-based command-line interface for the Bimo
//! coding agent harness.
//!
//! [`Cli`] defines the full command surface; [`run`] executes a parsed [`Cli`]
//! on its own tokio runtime, and [`run_async`] runs it on an existing one
//! (as `bimo-tui` will). [`run_env`] parses `std::env::args` and runs — the
//! standalone entry point used by the `bimo` binary.

pub mod cli;
mod handler;
mod handlers;
mod output;

pub use bimo_core;
pub use bimo_core::error::Result;
pub use cli::{Cli, Command};
pub use handler::run_async;

/// Executes a parsed [`Cli`] on its own tokio runtime.
///
/// Useful for standalone invocation. When embedding in an existing runtime
/// (e.g. in `bimo-tui`), call [`run_async`] instead.
pub fn run(cli: Cli) -> Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(run_async(&cli))
}

/// Initializes tracing, parses `std::env::args`, and executes the command.
///
/// Bare `bimo` (no subcommand) launches the TUI. Used by the `bimo` binary
/// crate, which carries no clap setup of its own.
pub fn run_env() -> Result<()> {
    use clap::Parser;
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .init();

    run(Cli::parse())
}
