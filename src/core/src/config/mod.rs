pub mod providers;
pub mod settings;

pub use providers::{CustomProviderConfig, ProviderPersistedConfig};
pub use settings::{Settings, ThinkingConfig};

use crate::error::{BimoError, Result};
use std::fs;
use std::path::PathBuf;

fn config_dir() -> Result<PathBuf> {
    let home = dirs::home_dir()
        .ok_or_else(|| BimoError::Config("cannot determine home directory".into()))?;
    Ok(home.join(".config").join("bimo"))
}

pub fn read_json<T: serde::de::DeserializeOwned>(filename: &str) -> Result<T> {
    let path = config_dir()?.join(filename);
    if !path.exists() {
        return Err(BimoError::Config(format!("file not found: {}", path.display())));
    }
    let data = fs::read_to_string(&path)
        .map_err(|e| BimoError::Config(format!("failed to read {}: {e}", path.display())))?;
    let value: T = serde_json::from_str(&data)
        .map_err(|e| BimoError::Config(format!("failed to parse {}: {e}", path.display())))?;
    Ok(value)
}

pub fn write_json<T: serde::Serialize>(filename: &str, value: &T) -> Result<()> {
    let dir = config_dir()?;
    fs::create_dir_all(&dir)
        .map_err(|e| BimoError::Config(format!("failed to create config dir: {e}")))?;
    let path = dir.join(filename);
    let data = serde_json::to_string_pretty(value)
        .map_err(|e| BimoError::Config(format!("failed to serialize: {e}")))?;
    fs::write(&path, data)
        .map_err(|e| BimoError::Config(format!("failed to write {}: {e}", path.display())))?;
    Ok(())
}

pub fn ensure_config_dir() -> Result<PathBuf> {
    let dir = config_dir()?;
    fs::create_dir_all(&dir)
        .map_err(|e| BimoError::Config(format!("failed to create config dir: {e}")))?;
    Ok(dir)
}
