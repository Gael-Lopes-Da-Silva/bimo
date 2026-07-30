//! Configuration persistence — providers and settings stored in `~/.config/bimo/`.

pub mod providers;
pub mod settings;

pub use providers::{ApiFormat, Provider, ProviderType, ProvidersFile};
pub use settings::Settings;
