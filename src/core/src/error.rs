use thiserror::Error;

#[derive(Error, Debug)]
pub enum BimoError {
    #[error("{0}")]
    Msg(String),
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Session error: {0}")]
    Session(String),

    #[error("Agent error: {0}")]
    Agent(String),

    #[error("Tool error: {0}")]
    Tool(String),

    #[error("Prompt error: {0}")]
    Prompt(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("YAML deserialization error: {0}")]
    SerdeYaml(#[from] serde_yaml::Error),

    #[error("AI provider error: {0}")]
    Provider(String),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, BimoError>;
