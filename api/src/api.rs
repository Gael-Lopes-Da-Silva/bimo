use crate::agent::Agent;
use crate::config::CustomProviderConfig;
use crate::error::{ApiErrorPayload, BimoError};
use serde::{Deserialize, Serialize};

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

// ---------------------------------------------------------------------------
// BimoApi — the public interface
// ---------------------------------------------------------------------------

/// The top-level API object. Wrap this in any transport (HTTP, gRPC, stdin, etc.).
pub struct BimoApi {
    agent: Agent,
}

impl BimoApi {
    pub fn new() -> Self {
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
        ApiResponse::ok(self.agent.list_providers())
    }

    pub async fn select_provider(&mut self, req: SelectProviderRequest) -> ApiResponse {
        match self.agent.select_provider(&req.provider_id).await {
            Ok(info) => ApiResponse::ok(info),
            Err(e) => ApiResponse::err(e),
        }
    }

    pub fn configure_provider(&mut self, req: ConfigureProviderRequest) -> ApiResponse {
        match self
            .agent
            .configure_provider(&req.provider_id, req.base_url, req.api_key)
        {
            Ok(()) => ApiResponse::ok(serde_json::Value::Null),
            Err(e) => ApiResponse::err(e),
        }
    }

    pub fn add_custom_provider(&mut self, req: AddCustomProviderRequest) -> ApiResponse {
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
            Ok(()) => ApiResponse::ok(serde_json::Value::Null),
            Err(e) => ApiResponse::err(e),
        }
    }

    // -----------------------------------------------------------------------
    // Model endpoints
    // -----------------------------------------------------------------------

    pub async fn list_models(&mut self) -> ApiResponse {
        if self.agent.list_models().is_empty() && self.agent.runtime.is_some() {
            let _ = self.agent.fetch_models().await;
        }
        ApiResponse::ok(self.agent.list_models().to_vec())
    }

    pub fn select_model(&mut self, req: SelectModelRequest) -> ApiResponse {
        match self.agent.select_model(&req.model_id) {
            Ok(()) => ApiResponse::ok(serde_json::Value::Null),
            Err(e) => ApiResponse::err(e),
        }
    }

    // -----------------------------------------------------------------------
    // Chat
    // -----------------------------------------------------------------------

    pub async fn chat(&mut self, req: ChatRequest) -> ApiResponse {
        match self.agent.chat(&req.message).await {
            Ok(resp) => ApiResponse::ok(ChatData {
                content: resp.content,
                model: resp.model,
                usage: resp.usage,
                session_id: resp.session_id,
            }),
            Err(e) => ApiResponse::err(e),
        }
    }

    // -----------------------------------------------------------------------
    // Session
    // -----------------------------------------------------------------------

    pub fn get_session(&self) -> ApiResponse {
        ApiResponse::ok(SessionData {
            session_id: self.agent.session.id.clone(),
            messages: self.agent.session.messages.clone(),
            message_count: self.agent.session.message_count(),
        })
    }

    pub fn clear_session(&mut self) -> ApiResponse {
        self.agent.clear_session();
        ApiResponse::ok(serde_json::Value::Null)
    }

    // -----------------------------------------------------------------------
    // Commands
    // -----------------------------------------------------------------------

    pub fn execute_command(&mut self, req: CommandRequest) -> ApiResponse {
        let input = if req.command.starts_with('/') {
            req.command
        } else {
            format!("/{}", req.command)
        };

        match self.agent.execute_command(&input) {
            Ok(result) => ApiResponse::ok(result),
            Err(e) => ApiResponse::err(e),
        }
    }

    // -----------------------------------------------------------------------
    // Status & help
    // -----------------------------------------------------------------------

    pub fn status(&self) -> ApiResponse {
        ApiResponse::ok(StatusData {
            provider: self.agent.config.selected_provider.clone(),
            model: self.agent.config.selected_model.clone(),
            session_id: self.agent.session.id.clone(),
            message_count: self.agent.session.message_count(),
            needs_configuration: self.agent.needs_configuration(),
        })
    }

    pub fn help(&self) -> ApiResponse {
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
        ApiResponse::ok(HelpData { commands })
    }
}
