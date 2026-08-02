//! Command dispatch — applies global overrides and routes to handlers.

use crate::cli::{Cli, Command};

/// Applies global flags and executes the parsed command.
pub async fn run_async(cli: &Cli) -> crate::Result<()> {
    if let Some(dir) = &cli.config_dir {
        bimo_core::paths::set_config_dir(dir.clone());
    }
    match &cli.command {
        Command::Provider { sub } => crate::handlers::provider::run(cli.json, sub).await,
        Command::Model { sub } => crate::handlers::model::run(cli.json, sub).await,
        Command::Session { sub } => crate::handlers::session::run(cli.json, sub).await,
        Command::Run(args) => crate::handlers::agent::run(cli.json, args).await,
        Command::Settings { sub } => crate::handlers::settings::run(cli.json, sub).await,
        Command::Tools { sub } => crate::handlers::misc::tools_run(cli.json, sub).await,
        Command::Skills { sub } => crate::handlers::misc::skills_run(cli.json, sub).await,
        Command::Cleanup(args) => crate::handlers::misc::cleanup_run(args).await,
        Command::ConfigPath => crate::handlers::misc::config_path(),
    }
}
