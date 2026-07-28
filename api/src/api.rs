use crate::agent::Agent;
use crate::config::CustomProviderConfig;
use crate::error::{ApiErrorPayload, BimoError};
use serde::{Deserialize, Serialize};
use tracing;

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
}

#[derive(Debug, Serialize)]
pub struct ChatData {
    pub content: String,
    pub model: Option<String>,
    pub usage: Option<crate::provider::UsageInfo>,
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

// ---------------------------------------------------------------------------
// BimoApi — the public interface
// ---------------------------------------------------------------------------

/// The top-level API object. Wrap this in any transport (HTTP, gRPC, stdin, etc.).
pub struct BimoApi {
    agent: Agent,
}

impl Default for BimoApi {
    fn default() -> Self {
        Self::new()
    }
}

impl BimoApi {
    pub fn new() -> Self {
        tracing::info!("initializing BimoApi");
        Self {
            agent: Agent::new(),
        }
    }

    /// Convenience constructor from an existing agent (useful for testing).
    pub fn from_agent(agent: Agent) -> Self {
        Self { agent }
    }

    // -----------------------------------------------------------------------
    // Provider endpoints
    // -----------------------------------------------------------------------

    pub fn list_providers(&self) -> ApiResponse {
        tracing::debug!("list_providers called");
        let resp = ApiResponse::ok(self.agent.list_providers());
        tracing::debug!(
            count = self.agent.list_providers().len(),
            "list_providers done"
        );
        resp
    }

    pub async fn select_provider(&mut self, req: SelectProviderRequest) -> ApiResponse {
        tracing::info!(provider_id = %req.provider_id, "select_provider called");
        match self.agent.select_provider(&req.provider_id).await {
            Ok(info) => {
                tracing::info!(provider = %info.name, "select_provider success");
                ApiResponse::ok(info)
            }
            Err(e) => {
                tracing::error!(error = %e, "select_provider failed");
                ApiResponse::err(e)
            }
        }
    }

    pub fn configure_provider(&mut self, req: ConfigureProviderRequest) -> ApiResponse {
        tracing::info!(provider_id = %req.provider_id, "configure_provider called");
        match self
            .agent
            .configure_provider(&req.provider_id, req.base_url, req.api_key)
        {
            Ok(()) => {
                tracing::info!("configure_provider success");
                ApiResponse::ok(serde_json::Value::Null)
            }
            Err(e) => {
                tracing::error!(error = %e, "configure_provider failed");
                ApiResponse::err(e)
            }
        }
    }

    pub fn add_custom_provider(&mut self, req: AddCustomProviderRequest) -> ApiResponse {
        tracing::info!(id = %req.id, name = %req.name, "add_custom_provider called");
        let cp = CustomProviderConfig {
            id: req.id,
            name: req.name,
            category: req.category,
            base_url: req.base_url,
            api_key_required: req.api_key_required,
            chat_endpoint: req.chat_endpoint,
            models_endpoint: req.models_endpoint,
            auth_header: req.auth_header,
            auth_prefix: req.auth_prefix,
        };
        match self.agent.add_custom_provider(cp) {
            Ok(()) => {
                tracing::info!("add_custom_provider success");
                ApiResponse::ok(serde_json::Value::Null)
            }
            Err(e) => {
                tracing::error!(error = %e, "add_custom_provider failed");
                ApiResponse::err(e)
            }
        }
    }

    // -----------------------------------------------------------------------
    // Model endpoints
    // -----------------------------------------------------------------------

    pub async fn list_models(&mut self) -> ApiResponse {
        tracing::debug!("list_models called");
        if self.agent.list_models().is_empty() && self.agent.runtime.is_some() {
            tracing::debug!("model list empty, fetching from provider");
            let _ = self.agent.fetch_models().await;
        }
        let count = self.agent.list_models().len();
        tracing::debug!(count, "list_models done");
        ApiResponse::ok(self.agent.list_models().to_vec())
    }

    pub fn select_model(&mut self, req: SelectModelRequest) -> ApiResponse {
        tracing::info!(model_id = %req.model_id, "select_model called");
        match self.agent.select_model(&req.model_id) {
            Ok(()) => {
                tracing::info!("select_model success");
                ApiResponse::ok(serde_json::Value::Null)
            }
            Err(e) => {
                tracing::error!(error = %e, "select_model failed");
                ApiResponse::err(e)
            }
        }
    }

    // -----------------------------------------------------------------------
    // Chat
    // -----------------------------------------------------------------------

    pub async fn chat(&mut self, req: ChatRequest) -> ApiResponse {
        tracing::info!(message_len = req.message.len(), "chat called");
        match self.agent.chat(&req.message).await {
            Ok(resp) => {
                tracing::info!(
                    model = ?resp.model,
                    content_len = resp.content.len(),
                    "chat success"
                );
                ApiResponse::ok(ChatData {
                    content: resp.content,
                    model: resp.model,
                    usage: resp.usage,
                    session_id: resp.session_id,
                })
            }
            Err(e) => {
                tracing::error!(error = %e, "chat failed");
                ApiResponse::err(e)
            }
        }
    }

    // -----------------------------------------------------------------------
    // Session
    // -----------------------------------------------------------------------

    pub fn get_session(&self) -> ApiResponse {
        tracing::debug!("get_session called");
        let data = SessionData {
            session_id: self.agent.session.id.clone(),
            messages: self.agent.session.messages.clone(),
            message_count: self.agent.session.message_count(),
        };
        tracing::debug!(message_count = data.message_count, "get_session done");
        ApiResponse::ok(data)
    }

    pub fn clear_session(&mut self) -> ApiResponse {
        tracing::info!("clear_session called");
        self.agent.clear_session();
        tracing::info!("clear_session done");
        ApiResponse::ok(serde_json::Value::Null)
    }

    // -----------------------------------------------------------------------
    // Commands
    // -----------------------------------------------------------------------

    pub async fn execute_command(&mut self, req: CommandRequest) -> ApiResponse {
        let input = if req.command.starts_with('/') {
            req.command
        } else {
            format!("/{}", req.command)
        };

        tracing::info!(command = %input, "execute_command called");
        match self.agent.execute_command(&input).await {
            Ok(result) => {
                tracing::info!(command = %result.command, output_len = result.output.len(), "execute_command success");
                ApiResponse::ok(result)
            }
            Err(e) => {
                tracing::error!(error = %e, "execute_command failed");
                ApiResponse::err(e)
            }
        }
    }

    // -----------------------------------------------------------------------
    // Status & help
    // -----------------------------------------------------------------------

    pub fn status(&self) -> ApiResponse {
        tracing::debug!("status called");
        let data = StatusData {
            provider: self.agent.config.selected_provider.clone(),
            model: self.agent.config.selected_model.clone(),
            session_id: self.agent.session.id.clone(),
            message_count: self.agent.session.message_count(),
            needs_configuration: self.agent.needs_configuration(),
        };
        tracing::debug!(provider = ?data.provider, model = ?data.model, "status done");
        ApiResponse::ok(data)
    }

    pub fn help(&self) -> ApiResponse {
        tracing::debug!("help called");
        let commands: Vec<CommandHelpEntry> = self
            .agent
            .command_registry
            .list()
            .into_iter()
            .map(|(name, desc)| CommandHelpEntry {
                name: name.to_string(),
                description: desc.to_string(),
            })
            .collect();
        tracing::debug!(count = commands.len(), "help done");
        ApiResponse::ok(HelpData { commands })
    }

    /// Return full command metadata for client autocompletion.
    pub fn list_commands(&self) -> ApiResponse {
        tracing::debug!("list_commands called");
        let commands = self.agent.command_registry.list_detailed();
        tracing::debug!(count = commands.len(), "list_commands done");
        ApiResponse::ok(CommandsData { commands })
    }
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
        // error field should be skipped
        assert!(!json.contains("error"));
    }

    #[test]
    fn api_response_error_serialization() {
        let resp = ApiResponse::err(BimoError::Model("bad model".into()));
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"success\":false"));
        assert!(json.contains("MODEL_ERROR"));
        // data field should be skipped
        assert!(!json.contains("\"data\""));
    }

    #[test]
    fn list_providers_returns_success() {
        let api = BimoApi::new();
        let resp = api.list_providers();
        assert!(resp.success);
        assert!(resp.data.is_some());
        let data = resp.data.unwrap();
        let providers = data.as_array().unwrap();
        assert_eq!(providers.len(), 3);
    }

    #[test]
    fn status_returns_current_state() {
        let api = BimoApi::new();
        let resp = api.status();
        assert!(resp.success);
        let data = resp.data.unwrap();
        assert!(data.get("session_id").is_some());
        assert!(data.get("message_count").is_some());
        assert_eq!(data["message_count"], 0);
    }

    #[test]
    fn help_returns_commands() {
        let api = BimoApi::new();
        let resp = api.help();
        assert!(resp.success);
        let data = resp.data.unwrap();
        let commands = data.get("commands").unwrap().as_array().unwrap();
        assert!(!commands.is_empty());
    }

    #[test]
    fn list_commands_returns_metadata() {
        let api = BimoApi::new();
        let resp = api.list_commands();
        assert!(resp.success);
        let data = resp.data.unwrap();
        let commands = data.get("commands").unwrap().as_array().unwrap();
        assert!(!commands.is_empty());
        // Each command should have name and description
        for cmd in commands {
            assert!(cmd.get("name").is_some());
            assert!(cmd.get("description").is_some());
        }
    }

    #[test]
    fn clear_session_resets_messages() {
        let mut api = BimoApi::new();
        // Add a message via agent directly
        api.agent.session.add_user_message("test");
        assert_eq!(api.agent.session.message_count(), 1);

        let resp = api.clear_session();
        assert!(resp.success);
        assert_eq!(api.agent.session.message_count(), 0);
    }

    #[test]
    fn get_session_returns_data() {
        let mut api = BimoApi::new();
        api.agent.session.add_user_message("hello");

        let resp = api.get_session();
        assert!(resp.success);
        let data = resp.data.unwrap();
        assert_eq!(data["message_count"], 1);
        let messages = data.get("messages").unwrap().as_array().unwrap();
        assert_eq!(messages.len(), 1);
    }

    #[test]
    fn execute_command_prepends_slash() {
        let mut api = BimoApi::new();
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let resp = api
                .execute_command(CommandRequest {
                    command: "help".into(),
                })
                .await;
            assert!(resp.success);
        });
    }

    #[test]
    fn execute_command_with_slash() {
        let mut api = BimoApi::new();
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let resp = api
                .execute_command(CommandRequest {
                    command: "/status".into(),
                })
                .await;
            assert!(resp.success);
        });
    }

    #[test]
    fn select_model_without_provider_succeeds_when_models_empty() {
        let mut api = BimoApi::new();
        let resp = api.select_model(SelectModelRequest {
            model_id: "test-model".into(),
        });
        // Should succeed because available_models is empty (allows unknown)
        assert!(resp.success);
        assert_eq!(
            api.agent.config.selected_model.as_deref(),
            Some("test-model")
        );
    }
}
