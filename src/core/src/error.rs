//! Error types for the Bimo core library.

use thiserror::Error;

/// Unified error type covering all Bimo core operations.
#[derive(Error, Debug)]
pub enum CustomError {
    #[error("{0}")]
    Msg(String),
    #[error("configuration error: {0}")]
    Config(String),
    #[error("session error: {0}")]
    Session(String),
    #[error("agent error: {0}")]
    Agent(String),
    #[error("tool error: {0}")]
    Tool(String),
    #[error("prompt error: {0}")]
    Prompt(String),
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("yaml error: {0}")]
    SerdeYaml(#[from] serde_yaml::Error),
    #[error("provider error: {0}")]
    Provider(String),
    #[error("error: {0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, CustomError>;
