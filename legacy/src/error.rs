use serde::{Deserialize, Serialize};

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

    #[error("api: {0}")]
    Api(String),

    #[error("serialization: {0}")]
    Serialization(String),
}

pub type Result<T> = std::result::Result<T, BimoError>;
