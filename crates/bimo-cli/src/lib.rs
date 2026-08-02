//! Bimo CLI library — a Clap-based command-line interface for the Bimo
//! coding agent harness.
//!
//! [`Cli`] defines the full command surface; [`run`] executes a parsed [`Cli`]
//! on its own tokio runtime, and [`run_async`] runs it on an existing one
//! (as `bimo-tui` will).

pub mod cli;
mod handler;
mod handlers;
mod output;

pub use bimo_core;
pub use cli::{Cli, Command};
pub use handler::run_async;

pub use bimo_core::error::Result;

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
