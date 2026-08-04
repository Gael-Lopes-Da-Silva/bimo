use crate::cli::{Cli, Command, TuiArgs};

/// Applies global flags and executes the parsed command.
pub async fn run_async(cli: &Cli) -> crate::Result<()> {
    if let Some(dir) = &cli.config_dir {
        bimo_core::paths::set_config_dir(dir.clone());
    }
    match &cli.command {
        Some(Command::Tui(args)) => crate::handlers::tui::run(args).await,
        Some(Command::Provider { sub }) => crate::handlers::provider::run(cli.json, sub).await,
        Some(Command::Model { sub }) => crate::handlers::model::run(cli.json, sub).await,
        Some(Command::Session { sub }) => crate::handlers::session::run(cli.json, sub).await,
        Some(Command::Run(args)) => crate::handlers::agent::run(cli.json, args).await,
        Some(Command::Settings { sub }) => crate::handlers::settings::run(cli.json, sub).await,
        Some(Command::Tools { sub }) => crate::handlers::misc::tools_run(cli.json, sub).await,
        Some(Command::Skills { sub }) => crate::handlers::misc::skills_run(cli.json, sub).await,
        Some(Command::Cleanup(args)) => crate::handlers::misc::cleanup_run(args).await,
        Some(Command::ConfigPath) => crate::handlers::misc::config_path(),
        None => crate::handlers::tui::run(&TuiArgs::default()).await,
    }
}
