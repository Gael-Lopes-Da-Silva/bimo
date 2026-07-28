use serde::{Deserialize, Serialize};

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_codes_match_variants() {
        let cases: Vec<(BimoError, &str)> = vec![
            (BimoError::Config("x".into()), "CONFIG_ERROR"),
            (BimoError::Provider("x".into()), "PROVIDER_ERROR"),
            (BimoError::Model("x".into()), "MODEL_ERROR"),
            (BimoError::Session("x".into()), "SESSION_ERROR"),
            (BimoError::Network("x".into()), "NETWORK_ERROR"),
            (BimoError::Command("x".into()), "COMMAND_ERROR"),
            (BimoError::Api("x".into()), "API_ERROR"),
            (BimoError::Serialization("x".into()), "SERIALIZATION_ERROR"),
            (BimoError::NotImplemented("x".into()), "NOT_IMPLEMENTED"),
        ];
        for (err, expected_code) in cases {
            let payload = ApiErrorPayload::from(&err);
            assert_eq!(payload.code, expected_code, "mismatch for {err}");
            assert_eq!(payload.message, err.to_string());
        }
    }

    #[test]
    fn error_display_format() {
        let err = BimoError::Provider("timeout".into());
        assert_eq!(err.to_string(), "provider: timeout");
    }

    #[test]
    fn error_is_clone() {
        let err = BimoError::Model("test".into());
        let cloned = err.clone();
        assert_eq!(format!("{err}"), format!("{cloned}"));
    }

    #[test]
    fn error_is_serialize_deserialize() {
        let err = BimoError::Command("bad input".into());
        let json = serde_json::to_string(&err).unwrap();
        let deserialized: BimoError = serde_json::from_str(&json).unwrap();
        assert_eq!(format!("{err}"), format!("{deserialized}"));
    }
}
