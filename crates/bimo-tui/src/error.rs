use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Theme error: {0}")]
    Theme(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Agent error: {0}")]
    Agent(#[from] bimo_core::error::CustomError),

    #[error("Cursive error: {0}")]
    Cursive(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl From<crate::theme::ThemeError> for Error {
    fn from(err: crate::theme::ThemeError) -> Self {
        Error::Theme(err.to_string())
    }
}
