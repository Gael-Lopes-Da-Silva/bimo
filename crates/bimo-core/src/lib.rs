//! Core library for the Bimo AI coding agent harness.
//!
//! Provides configuration, provider/model registries, agent execution,
//! tool definitions, prompt rendering, and session management.

pub mod agent;
pub mod config;
pub mod error;
pub mod models;
pub mod paths;
pub mod prompt;
pub mod providers;
pub mod session;
pub mod skill;
pub mod snapshot;
pub mod tools;

pub use agent::{Agent, AgentBuilder, AgentEvent, GenerationOutcome, SteerCommand};
pub use aisdk::core::language_model::ReasoningEffort;
pub use config::{ApiFormat, Provider, ProviderType, ProvidersConfig, SettingsConfig};
pub use error::{CustomError, Result};
pub use models::{ModelEntry, ModelRegistry};
pub use prompt::PromptEngine;
pub use providers::{CloudProviderRegistry, LocalProviderRegistry};
pub use session::{Message, Session, SessionManager, UndoBatch};
pub use skill::Skill;
pub use snapshot::{Snapshot, SnapshotRecord};
pub use tools::{
    TodoItem, TodoList, TodoPriority, TodoStatus, edit_file, is_builtin, manage_todo, read_file,
    run_command, tool_names, write_file,
};
