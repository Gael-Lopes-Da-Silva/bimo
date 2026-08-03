//! Bimo TUI - Terminal User Interface for the Bimo coding agent

pub mod app;
pub mod config;
pub mod error;
pub mod events;
pub mod input;
pub mod palette;
pub mod theme;

pub use app::{App, run_tui};
pub use config::{
    ThemeConfigFile, ensure_default_theme, get_themes_dir, list_available_themes, load_theme,
    save_theme,
};
pub use error::{Error, Result};
pub use events::{EventBridge, create_event_bridge, handle_agent_event};
pub use input::{Autocomplete, AutocompleteSource, FileCompleter, KeyBinding, KeyBindings};
pub use palette::{Command, CommandRegistry, create_command_palette_layer};
pub use theme::{BimoTheme, ThemeColors, ThemeError, ThemeVariant};
