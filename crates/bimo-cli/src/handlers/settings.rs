use bimo_core::config::SettingsConfig;
use bimo_core::error::CustomError;

use crate::cli::SettingsCommand;
use crate::output;

pub async fn run(json: bool, sub: &SettingsCommand) -> crate::Result<()> {
    match sub {
        SettingsCommand::Show => show(json).await,
        SettingsCommand::Set { key, value } => set(key, value).await,
        SettingsCommand::Unset { key } => unset(key).await,
        SettingsCommand::Reset => reset().await,
    }
}

async fn show(json: bool) -> crate::Result<()> {
    let settings = SettingsConfig::load()?;
    if json {
        return output::emit_json(&settings);
    }
    println!("session_ttl_hours: {}", settings.session_ttl_hours);
    println!("max_sessions: {}", settings.max_sessions);
    println!(
        "cleanup_interval_minutes: {}",
        settings.cleanup_interval_minutes
    );
    println!("max_steps: {}", settings.max_steps);
    println!(
        "default_provider: {}",
        settings.default_provider.as_deref().unwrap_or("-")
    );
    println!(
        "default_model: {}",
        settings.default_model.as_deref().unwrap_or("-")
    );
    println!("debug: {}", settings.debug);
    println!("retry_attempts: {}", settings.retry_attempts);
    println!("retry_timeout_secs: {}", settings.retry_timeout_secs);
    println!("snapshots: {}", settings.snapshots);
    println!("path: {}", SettingsConfig::path().display());
    Ok(())
}

async fn set(key: &str, value: &str) -> crate::Result<()> {
    let mut settings = SettingsConfig::load()?;
    match key {
        "session_ttl_hours" => settings.session_ttl_hours = parse(key, value)?,
        "max_sessions" => settings.max_sessions = parse(key, value)?,
        "cleanup_interval_minutes" => settings.cleanup_interval_minutes = parse(key, value)?,
        "max_steps" => settings.max_steps = parse(key, value)?,
        "default_provider" => settings.default_provider = Some(value.to_string()),
        "default_model" => settings.default_model = Some(value.to_string()),
        "debug" => settings.debug = parse(key, value)?,
        "retry_attempts" => settings.retry_attempts = parse(key, value)?,
        "retry_timeout_secs" => settings.retry_timeout_secs = parse(key, value)?,
        "snapshots" => settings.snapshots = parse(key, value)?,
        other => return Err(CustomError::Config(format!("Unknown setting '{other}'"))),
    }
    settings.save()?;
    println!("Set {key} = {value}");
    Ok(())
}

async fn unset(key: &str) -> crate::Result<()> {
    let mut settings = SettingsConfig::load()?;
    match key {
        "default_provider" => settings.default_provider = None,
        "default_model" => settings.default_model = None,
        other => {
            return Err(CustomError::Config(format!(
                "Setting '{other}' is not optional; use `set`"
            )));
        }
    }
    settings.save()?;
    println!("Unset {key}");
    Ok(())
}

async fn reset() -> crate::Result<()> {
    SettingsConfig::default().save()?;
    println!("Settings reset to defaults");
    Ok(())
}

fn parse<T: std::str::FromStr>(key: &str, value: &str) -> crate::Result<T> {
    value
        .parse()
        .map_err(|_| CustomError::Config(format!("Invalid value '{value}' for setting '{key}'")))
}
