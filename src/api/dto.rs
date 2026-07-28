use crate::error::{ApiErrorPayload, BimoError};
use serde::{Deserialize, Serialize};

use crate::provider::UsageInfo;

// ---------------------------------------------------------------------------
// Generic JSON envelope
// ---------------------------------------------------------------------------

/// The standard JSON response envelope returned by every API endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ApiErrorPayload>,
}

impl ApiResponse {
    pub fn ok<T: Serialize>(data: T) -> Self {
        Self {
            success: true,
            data: Some(serde_json::to_value(data).unwrap_or_default()),
            error: None,
        }
    }

    pub fn err(error: BimoError) -> Self {
        ApiResponse {
            success: false,
            data: None,
            error: Some(ApiErrorPayload::from(&error)),
        }
    }
}

// ---------------------------------------------------------------------------
// Request / response data types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    pub message: String,
    /// Optional session id to target. If omitted, uses the active session.
    #[serde(default)]
    pub session_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ChatData {
    pub content: String,
    pub model: Option<String>,
    pub usage: Option<UsageInfo>,
    pub session_id: String,
}

#[derive(Debug, Deserialize)]
pub struct SelectProviderRequest {
    pub provider_id: String,
}

#[derive(Debug, Deserialize)]
pub struct ConfigureProviderRequest {
    pub provider_id: String,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SelectModelRequest {
    pub model_id: String,
}

#[derive(Debug, Deserialize)]
pub struct AddCustomProviderRequest {
    pub id: String,
    pub name: String,
    #[serde(default = "default_local")]
    pub category: String,
    pub base_url: String,
    #[serde(default)]
    pub api_key_required: bool,
    pub chat_endpoint: String,
    #[serde(default)]
    pub models_endpoint: Option<String>,
    #[serde(default)]
    pub auth_header: Option<String>,
    #[serde(default)]
    pub auth_prefix: Option<String>,
}

fn default_local() -> String {
    "local".into()
}

#[derive(Debug, Deserialize)]
pub struct CommandRequest {
    pub command: String,
}

#[derive(Debug, Serialize)]
pub struct StatusData {
    pub provider: Option<String>,
    pub model: Option<String>,
    pub session_id: String,
    pub active_session_id: String,
    pub session_count: usize,
    pub message_count: usize,
    pub needs_configuration: bool,
}

#[derive(Debug, Serialize)]
pub struct SessionData {
    pub session_id: String,
    pub messages: Vec<crate::session::Message>,
    pub message_count: usize,
}

#[derive(Debug, Serialize)]
pub struct HelpData {
    pub commands: Vec<CommandHelpEntry>,
}

#[derive(Debug, Serialize)]
pub struct CommandHelpEntry {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Serialize)]
pub struct CommandsData {
    pub commands: Vec<crate::command::CommandInfo>,
}

#[derive(Debug, Serialize)]
pub struct ContextData {
    pub session_id: String,
    pub messages: Vec<ContextMessage>,
    pub total_characters: usize,
    pub estimated_tokens: usize,
    pub max_context_tokens: usize,
    pub remaining_tokens: usize,
    pub usage_percentage: f64,
}

#[derive(Debug, Serialize)]
pub struct ContextMessage {
    pub role: String,
    pub content: String,
    pub characters: usize,
    pub estimated_tokens: usize,
}

#[derive(Debug, Serialize)]
pub struct ThinkingData {
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub budget_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<String>,
}

// ---------------------------------------------------------------------------
// Session management
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct SwitchSessionRequest {
    pub session_id: String,
}

#[derive(Debug, Serialize)]
pub struct CreateSessionData {
    pub session_id: String,
}

#[derive(Debug, Serialize)]
pub struct SessionListData {
    pub sessions: Vec<crate::session::SessionInfo>,
    pub active_session_id: String,
}

// ---------------------------------------------------------------------------
// Streaming events
// ---------------------------------------------------------------------------

/// Events emitted over SSE during a streaming chat response.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum ChatStreamEvent {
    /// A chunk of text content from the LLM.
    #[serde(rename = "content")]
    Content { delta: String },

    /// The LLM produced tool calls — they are now being executed.
    #[serde(rename = "tool_start")]
    ToolStart {
        tool: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        args: Option<serde_json::Value>,
    },

    /// A tool call finished executing.
    #[serde(rename = "tool_result")]
    ToolResult { tool: String, is_error: bool },

    /// The final metadata emitted once the LLM finishes (no more tool calls).
    #[serde(rename = "done")]
    Done {
        model: Option<String>,
        usage: Option<UsageInfo>,
        session_id: String,
    },

    /// An error occurred.
    #[serde(rename = "error")]
    Error { message: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_response_ok() {
        let resp = ApiResponse::ok(serde_json::json!({"key": "value"}));
        assert!(resp.success);
        assert!(resp.data.is_some());
        assert!(resp.error.is_none());
    }

    #[test]
    fn api_response_err() {
        let err = BimoError::Provider("test error".into());
        let resp = ApiResponse::err(err);
        assert!(!resp.success);
        assert!(resp.data.is_none());
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap().code, "PROVIDER_ERROR");
    }

    #[test]
    fn api_response_serialization() {
        let resp = ApiResponse::ok(serde_json::json!({"test": true}));
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"success\":true"));
        assert!(json.contains("\"test\":true"));
        assert!(!json.contains("error"));
    }

    #[test]
    fn api_response_error_serialization() {
        let resp = ApiResponse::err(BimoError::Model("bad model".into()));
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"success\":false"));
        assert!(json.contains("MODEL_ERROR"));
        assert!(!json.contains("\"data\""));
    }
}
