use serde::{Deserialize, Serialize};
use std::fmt;

/// The unified error type for all Bimo operations.
///
/// Every variant maps to a machine-readable error code and human message,
/// making it straightforward for any JSON client to handle failures.
#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
pub enum BimoError {
    #[error("config: {0}")]
    Config(String),

    #[error("provider: {0}")]
    Provider(String),

    #[error("model: {0}")]
    Model(String),

    #[error("session: {0}")]
    Session(String),

    #[error("network: {0}")]
    Network(String),

    #[error("command: {0}")]
    Command(String),

    #[error("api: {0}")]
    Api(String),

    #[error("serialization: {0}")]
    Serialization(String),

    #[error("not_implemented: {0}")]
    NotImplemented(String),
}

/// A structured error payload returned inside every JSON response on failure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiErrorPayload {
    pub code: String,
    pub message: String,
}

impl From<&BimoError> for ApiErrorPayload {
    fn from(err: &BimoError) -> Self {
        let code = match err {
            BimoError::Config(_) => "CONFIG_ERROR",
            BimoError::Provider(_) => "PROVIDER_ERROR",
            BimoError::Model(_) => "MODEL_ERROR",
            BimoError::Session(_) => "SESSION_ERROR",
            BimoError::Network(_) => "NETWORK_ERROR",
            BimoError::Command(_) => "COMMAND_ERROR",
            BimoError::Api(_) => "API_ERROR",
            BimoError::Serialization(_) => "SERIALIZATION_ERROR",
            BimoError::NotImplemented(_) => "NOT_IMPLEMENTED",
        };
        ApiErrorPayload {
            code: code.to_string(),
            message: err.to_string(),
        }
    }
}

/// Convenience alias used throughout the crate.
pub type Result<T> = std::result::Result<T, BimoError>;
