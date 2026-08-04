use std::path::PathBuf;
use std::sync::RwLock;

/// Override set by [`set_config_dir`]. When unset, the platform config
/// directory (`~/.config/bimo`) is used.
static CONFIG_DIR_OVERRIDE: RwLock<Option<PathBuf>> = RwLock::new(None);

/// Sets the base config directory override (e.g. from `--config-dir`).
///
/// Must be called before any core path is computed. A later call replaces an
/// earlier override.
pub fn set_config_dir(dir: PathBuf) {
    *CONFIG_DIR_OVERRIDE
        .write()
        .unwrap_or_else(|e| e.into_inner()) = Some(dir);
}

/// Returns the base config directory: the override when set, otherwise
/// `~/.config/bimo`.
pub fn config_dir() -> PathBuf {
    CONFIG_DIR_OVERRIDE
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
        .unwrap_or_else(|| {
            let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("~/.config"));
            base.join("bimo")
        })
}
