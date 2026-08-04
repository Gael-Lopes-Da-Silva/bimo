//! Bimo CORE; core library for the Bimo AI coding agent harness.
//!
//! Bimo is a coding agent: given a prompt, it runs a model against a provider,
//! streams the response, executes tool calls (file read/write/edit, shell
//! commands, todo management), and persists the whole conversation as a
//! session. This crate provides all of the underlying building blocks, with no
//! CLI or UI of its own (see `bimo-cli` for the command-line layer).
//!
//! # Modules
//!
//! - [`agent`] — the agent loop. An [`Agent`] is assembled with the
//!   builder-pattern [`AgentBuilder`] (provider, model, session, user prompt,
//!   sampling/reasoning/retry options), then runs model generations, tool
//!   calls, and optional steering ([`SteerCommand`]) while emitting streaming
//!   [`AgentEvent`]s over a broadcast channel. Also handles session naming and
//!   conversation compaction.
//! - [`config`] — persisted configuration: [`ProvidersConfig`] (the
//!   `providers.json` file listing configured [`Provider`]s, their kind
//!   ([`ProviderType`]), wire format ([`ApiFormat`]), base URL, and API key)
//!   and [`SettingsConfig`] (the `settings.json` file: session lifecycle,
//!   agent defaults, retry policy, and snapshot toggles).
//! - [`error`] — the unified [`CustomError`] and [`Result`] types.
//! - [`models`] — model metadata from the models.dev registry ([`ModelEntry`],
//!   [`ModelRegistry`]) and the runtime-erased model type that dispatches to
//!   the OpenAI-compatible, Anthropic, or Google SDK based on the provider's
//!   [`ApiFormat`].
//! - [`paths`] — the base config directory (overrideable via
//!   [`paths::set_config_dir`]), under which configuration files, session files, the
//!   models.dev cache, and snapshot metadata live.
//! - [`prompt`] — [`PromptEngine`] renders the compile-time-embedded system,
//!   summary, session-name, and compaction prompt templates, substituting
//!   `{{PLACEHOLDER}}` variables.
//! - [`providers`] — provider registries: [`CloudProviderRegistry`] fetches
//!   the models.dev catalogue and caches it to `models_cache.json` for offline
//!   use; [`LocalProviderRegistry`] knows the built-in local providers
//!   (ollama, lmstudio, vllm, llamacpp) and can auto-discover their models.
//! - [`session`] — persisted conversations ([`Session`], [`Message`],
//!   [`UndoBatch`]) plus [`SessionManager`] lifecycle management with a
//!   background cleanup task. Sessions support undo/redo with filesystem
//!   restore, forking, compaction archives, per-tool and per-skill disabling,
//!   and per-model reasoning effort.
//! - [`skill`] — loads `SKILL.md` skills (YAML frontmatter plus body) from
//!   project and user directories for injection into the system prompt.
//! - [`snapshot`] — git-backed filesystem snapshots; see the section below.
//! - [`tools`] — the agent's built-in tools — `read_file`, `edit_file`,
//!   `write_file`, `run_command`, and `manage_todo` (with its [`TodoList`]
//!   support) — plus helpers to enumerate and describe them.
//!
//! # Re-exports
//!
//! The most commonly used types are re-exported at the crate root, including
//! [`Agent`] and friends, the config types, [`CustomError`]/[`Result`],
//! [`PromptEngine`], both provider registries, the session types,
//! [`Skill`], the snapshot types, the [`TodoList`] machinery, and
//! [`ReasoningEffort`].
//!
//! # Filesystem snapshots
//!
//! Git-backed filesystem snapshots for reverting agent file changes.
//!
//! A [`Snapshot`] records the full working-tree state of a git repository
//! (tracked, modified, and untracked non-ignored files) as a commit object in
//! that repository. [`Snapshot::restore`] rewinds the working tree to the
//! captured state: files modified after the snapshot are overwritten, files
//! deleted after it are recreated, and files created after it are removed.
//! Git-ignored paths such as build artifacts are left untouched.
//!
//! Snapshots depend on git and therefore only work inside git repositories.
//! When a snapshot cannot be captured — the project is not a repository, git
//! is unavailable, ... — callers degrade gracefully: the conversation can
//! still be rewound, only the filesystem cannot be restored.

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
