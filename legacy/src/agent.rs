use aisdk::core::LanguageModelStreamChunkType;
use futures::StreamExt;
use serde::Serialize;
use tokio::sync::mpsc;

use crate::config::providers::ProvidersConfig;
use crate::config::settings::Settings;
use crate::context::{ProjectContext, build_project_context};
use crate::error::{BimoError, Result};
use crate::model::ModelInfo;
use crate::prompts;
use crate::provider::aisdk::AisdkProvider;
use crate::provider::registry::ProviderRegistry;
use crate::provider::types::{ProviderRuntime, UsageInfo};
use crate::session::{Role, Session};
use crate::tool::execute::execute_tool;
use crate::tool::{ToolRegistry, default_tools};

const DEFAULT_MAX_TOOL_ITERATIONS: usize = 20;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum ChatStreamEvent {
    #[serde(rename = "content")]
    Content { delta: String },
    #[serde(rename = "tool_start")]
    ToolStart {
        tool: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        args: Option<serde_json::Value>,
    },
    #[serde(rename = "tool_result")]
    ToolResult { tool: String, is_error: bool },
    #[serde(rename = "done")]
    Done {
        model: Option<String>,
        usage: Option<UsageInfo>,
        session_id: String,
    },
    #[serde(rename = "error")]
    Error { message: String },
}

pub struct CoreAgent {
    pub settings: Settings,
    pub providers_config: ProvidersConfig,
    pub session: Session,
    pub provider_registry: ProviderRegistry,
    pub available_models: Vec<ModelInfo>,
    pub runtime: Option<ProviderRuntime>,
    pub tool_registry: ToolRegistry,
    #[allow(dead_code)]
    project_context: ProjectContext,
}

impl CoreAgent {
    pub async fn new() -> Self {
        let settings = Settings::load();
        let providers_config = ProvidersConfig::load();
        let provider_registry = ProviderRegistry::new().await;
        let tool_registry = default_tools();
        let mut session = Session::new();

        let cwd = std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "<unknown>".into());
        let project_context = build_project_context(&cwd);
        let tool_json =
            serde_json::to_string_pretty(&tool_registry.render_json_schemas()).unwrap_or_default();
        let now = chrono::Local::now().format("%Y-%m-%d").to_string();

        let system_prompt = prompts::render(
            &prompts::load(prompts::SYSTEM),
            &[
                ("TOOLS", &tool_json),
                ("DATE", &now),
                ("CWD", &cwd),
                ("PROJECT_CONTEXT", &project_context.rendered),
            ],
        );
        session.add_system_message(&system_prompt);

        let runtime = settings.selected_provider.as_deref().and_then(|pid| {
            provider_registry
                .resolve_runtime(pid, &providers_config)
                .ok()
        });

        Self {
            settings,
            providers_config,
            session,
            provider_registry,
            available_models: Vec::new(),
            runtime,
            tool_registry,
            project_context,
        }
    }

    pub fn needs_configuration(&self) -> bool {
        self.settings.selected_provider.is_none()
    }

    pub fn list_providers(&self) -> Vec<crate::provider::types::ProviderInfo> {
        self.provider_registry.list_all(&self.providers_config)
    }

    pub async fn select_provider(
        &mut self,
        provider_id: &str,
    ) -> Result<crate::provider::types::ProviderInfo> {
        let info = self
            .provider_registry
            .list_all(&self.providers_config)
            .into_iter()
            .find(|p| p.id == provider_id)
            .ok_or_else(|| BimoError::Provider(format!("unknown provider '{provider_id}'")))?;

        let runtime = self
            .provider_registry
            .resolve_runtime(provider_id, &self.providers_config)?;

        self.runtime = Some(runtime);
        self.settings.selected_provider = Some(provider_id.to_string());
        self.available_models.clear();
        self.settings.selected_model = None;
        self.settings.save()?;

        self.fetch_models().await?;

        Ok(info)
    }

    pub async fn fetch_models(&mut self) -> Result<Vec<ModelInfo>> {
        let runtime = self
            .runtime
            .as_ref()
            .ok_or_else(|| BimoError::Provider("no provider selected".into()))?;

        let models = crate::model::fetch_models_for_provider(
            runtime,
            self.provider_registry.models_dev.as_ref(),
        )
        .await?;
        self.available_models = models.clone();
        Ok(models)
    }

    pub fn list_models(&self) -> &[ModelInfo] {
        &self.available_models
    }

    pub fn select_model(&mut self, model_id: &str) -> Result<()> {
        let exists = self.available_models.iter().any(|m| m.id == model_id);
        if !exists && !self.available_models.is_empty() {
            return Err(BimoError::Model(format!(
                "model '{model_id}' not found. Available models: {}",
                self.available_models
                    .iter()
                    .map(|m| m.id.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        }
        self.settings.selected_model = Some(model_id.to_string());
        self.settings.save()?;
        Ok(())
    }

    pub async fn chat_stream(
        &mut self,
        user_message: &str,
        tx: mpsc::Sender<ChatStreamEvent>,
    ) -> Result<()> {
        let runtime = self
            .runtime
            .clone()
            .ok_or_else(|| BimoError::Provider("no provider selected".into()))?;
        let model = self
            .settings
            .selected_model
            .clone()
            .ok_or_else(|| BimoError::Model("no model selected".into()))?;
        let model_clone = model.clone();

        self.session.add_user_message(user_message);

        let max_iterations = self
            .settings
            .max_tool_iterations
            .unwrap_or(DEFAULT_MAX_TOOL_ITERATIONS);

        macro_rules! send_or_cancel {
            ($tx:expr, $event:expr) => {
                if $tx.send($event).await.is_err() {
                    return Ok(());
                }
            };
        }

        for _iteration in 0..=max_iterations {
            let messages = self.session.to_chat_messages();
            let mut stream = Box::pin(
                chat_completion_streaming(&runtime, &messages, &model, &self.settings.thinking)
                    .await?,
            );

            let mut content = String::new();

            while let Some(chunk_result) = stream.next().await {
                match chunk_result {
                    Ok(chunk) => {
                        if let Some(delta) =
                            extract_stream_delta(&chunk, &runtime.request_body_format)
                        {
                            content.push_str(&delta);
                            send_or_cancel!(
                                tx,
                                ChatStreamEvent::Content {
                                    delta: delta.clone()
                                }
                            );
                        }
                    }
                    Err(e) => {
                        let _ = tx
                            .send(ChatStreamEvent::Error {
                                message: e.to_string(),
                            })
                            .await;
                        return Ok(());
                    }
                }
            }

            if content.trim().is_empty() {
                send_or_cancel!(
                    tx,
                    ChatStreamEvent::Error {
                        message: "Empty response from model".into(),
                    }
                );
                return Ok(());
            }

            let tool_calls = self.tool_registry.parse_tool_calls(&content)?;

            if tool_calls.is_empty() {
                self.session.add_assistant_response(
                    &content,
                    Some(model_clone.clone()),
                    Some(runtime.id.clone()),
                    None,
                );

                send_or_cancel!(
                    tx,
                    ChatStreamEvent::Done {
                        model: Some(model_clone.clone()),
                        usage: None,
                        session_id: self.session.id.clone(),
                    }
                );
                return Ok(());
            }

            self.session.add_assistant_response(
                &content,
                Some(model_clone.clone()),
                Some(runtime.id.clone()),
                None,
            );

            for call in &tool_calls {
                let args_json = serde_json::to_value(&call.arguments).ok();
                send_or_cancel!(
                    tx,
                    ChatStreamEvent::ToolStart {
                        tool: call.name.clone(),
                        args: args_json,
                    }
                );

                let result = execute_tool(call, &mut self.session.todos).await;
                let is_error = result.is_err();

                send_or_cancel!(
                    tx,
                    ChatStreamEvent::ToolResult {
                        tool: call.name.clone(),
                        is_error,
                    }
                );

                let result_msg = match result {
                    Ok(output) => output,
                    Err(e) => format!("Error: {e}"),
                };
                self.session.add_tool_message(&result_msg);
            }
        }

        Err(BimoError::Api(
            "tool call loop exceeded maximum iterations".into(),
        ))
    }
}

impl Default for CoreAgent {
    fn default() -> Self {
        panic!("CoreAgent::default() is not supported; use CoreAgent::new().await instead")
    }
}
