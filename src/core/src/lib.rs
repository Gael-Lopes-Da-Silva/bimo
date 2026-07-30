//! Core library for the Bimo AI coding agent harness.
//!
//! Provides configuration, provider/model registries, agent execution,
//! tool definitions, prompt rendering, and session management.

pub mod agent;
pub mod config;
pub mod error;
pub mod models;
pub mod prompt;
pub mod providers;
pub mod session;
pub mod skill;
pub mod tools;

pub use agent::{Agent, AgentBuilder, AgentEvent};
pub use config::{ApiFormat, Provider, ProviderType, ProvidersFile, Settings};
pub use error::{BimoError, Result};
pub use models::{ModelEntry, ModelRegistry};
pub use prompt::PromptEngine;
pub use providers::{
    ProviderRegistry, auto_discover_models, builtin_local_providers, discover_models,
    resolve_from_registry, resolve_provider,
};
pub use session::{Message, Session, SessionManager};
pub use skill::Skill;
pub use tools::{TodoItem, TodoList, TodoPriority, TodoStatus};
pub use tools::{edit_file, manage_todo, read_file, run_command, write_file};
