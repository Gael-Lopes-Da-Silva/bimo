use std::sync::Arc;

use aisdk::core::{DynamicModel, LanguageModelStreamChunkType};
use aisdk::providers::OpenAICompatible;
use futures::StreamExt;
use tokio::sync::broadcast;
use tracing::info;

use crate::config::LocalProvider;
use crate::error::Result;
use crate::models::ModelRegistry;
use crate::session::Session;

type ModelProvider = OpenAICompatible<DynamicModel>;

#[derive(Debug, Clone)]
pub enum AgentEvent {
    TextDelta(String),
    ReasoningDelta(String),
    ToolCallStart {
        tool_name: String,
        args: serde_json::Value,
    },
    ToolCallEnd {
        tool_name: String,
        result: std::result::Result<String, String>,
    },
    Error(String),
    Done,
}

pub struct Agent {
    pub(crate) runner: Option<AgentRunner>,
    pub session: Session,
}

#[allow(dead_code)]
pub(crate) struct AgentRunner {
    pub provider_name: String,
    pub provider_model: String,
    pub provider_api_key: Option<String>,
    pub provider_base_url: Option<String>,
    pub system_prompt: String,
    pub user_prompt: String,
    pub max_steps: usize,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub local_providers: Vec<LocalProvider>,
    pub registry: Option<Arc<ModelRegistry>>,
}

impl Agent {
    pub fn builder() -> super::AgentBuilder {
        super::AgentBuilder::new()
    }

    /// Run the agent as a oneshot (non-streaming). The response is not forwarded
    /// as text deltas — use `run_stream` for real-time deltas.
    pub async fn run(&mut self) -> Result<broadcast::Receiver<AgentEvent>> {
        let runner = self
            .runner
            .take()
            .ok_or_else(|| crate::error::BimoError::Agent("Agent already consumed".to_string()))?;

        let (tx, rx) = broadcast::channel(256);

        let todos: crate::tools::SharedTodoList = crate::tools::new_shared_todolist();
        crate::tools::init_todo_list(todos.clone());

        let session_id = self.session.id.clone();

        tokio::spawn(async move {
            info!("Starting agent session: {}", session_id);
            match runner.execute().await {
                Ok(()) => {
                    let _ = tx.send(AgentEvent::Done);
                }
                Err(e) => {
                    let _ = tx.send(AgentEvent::Error(e));
                    let _ = tx.send(AgentEvent::Done);
                }
            }
        });

        Ok(rx)
    }

    /// Run the agent with streaming. Each text/reasoning delta is forwarded
    /// as an `AgentEvent::TextDelta` / `AgentEvent::ReasoningDelta` in real time.
    pub async fn run_stream(&mut self) -> Result<broadcast::Receiver<AgentEvent>> {
        let runner = self
            .runner
            .take()
            .ok_or_else(|| crate::error::BimoError::Agent("Agent already consumed".to_string()))?;

        let (tx, rx) = broadcast::channel(256);

        let todos: crate::tools::SharedTodoList = crate::tools::new_shared_todolist();
        crate::tools::init_todo_list(todos.clone());

        let session_id = self.session.id.clone();

        tokio::spawn(async move {
            info!("Starting agent stream session: {}", session_id);
            runner.execute_stream(tx).await;
        });

        Ok(rx)
    }

    /// Oneshot helper that collects all text into a single string.
    pub async fn run_text(&mut self) -> Result<String> {
        let mut rx = self.run().await?;
        let mut output = String::new();

        while let Ok(event) = rx.recv().await {
            match event {
                AgentEvent::TextDelta(text) => output.push_str(&text),
                AgentEvent::ToolCallStart { tool_name, args } => {
                    output.push_str(&format!("\n[Tool: {tool_name}({args})]\n"));
                }
                AgentEvent::ToolCallEnd {
                    tool_name: _,
                    result,
                } => match result {
                    Ok(r) => output.push_str(&format!("[Result: {r}]\n")),
                    Err(e) => output.push_str(&format!("[Error: {e}]\n")),
                },
                AgentEvent::Done => break,
                AgentEvent::Error(e) => {
                    output.push_str(&format!("\n[Error: {e}]\n"));
                    break;
                }
                _ => {}
            }
        }

        Ok(output)
    }
}

impl AgentRunner {
    /// Build the provider model from the runner configuration.
    async fn build_model(&self) -> std::result::Result<ModelProvider, String> {
        let base_url = self.resolve_base_url().await?;

        let mut builder_cfg = OpenAICompatible::<DynamicModel>::builder()
            .base_url(base_url)
            .model_name(&self.provider_model);

        if let Some(key) = &self.provider_api_key {
            builder_cfg = builder_cfg.api_key(key.clone());
        }

        builder_cfg
            .build()
            .map_err(|e| format!("Failed to build provider: {e}"))
    }

    async fn resolve_base_url(&self) -> std::result::Result<String, String> {
        if let Some(url) = &self.provider_base_url {
            return Ok(url.clone());
        }

        if let Some(local) = self
            .local_providers
            .iter()
            .find(|p| p.name.to_lowercase() == self.provider_name.to_lowercase())
        {
            return Ok(local.base_url.clone());
        }

        if let Some(registry) = &self.registry {
            return registry
                .provider_base_url(&self.provider_name)
                .await
                .ok_or_else(|| {
                    format!(
                        "Unknown provider '{}' — not found in registry or local config",
                        self.provider_name
                    )
                });
        }

        Err(format!(
            "Unknown provider '{}' — no base_url configured and no registry available",
            self.provider_name
        ))
    }

    /// Oneshot execution — consumes self, runs to completion, returns Ok/Err.
    async fn execute(self) -> std::result::Result<(), String> {
        let tools = crate::tools::all_tools();
        let model = self.build_model().await?;

        let mut req_builder = aisdk::core::LanguageModelRequest::<ModelProvider>::builder()
            .model(model)
            .system(&self.system_prompt)
            .prompt(&self.user_prompt);

        for tool in tools {
            req_builder = req_builder.with_tool(tool);
        }

        let mut request = req_builder
            .stop_when(aisdk::core::utils::step_count_is(self.max_steps))
            .build();

        request.generate_text().await.map_err(|e| e.to_string())?;

        Ok(())
    }

    /// Streaming execution — forwards real-time deltas through the broadcast channel.
    async fn execute_stream(self, tx: broadcast::Sender<AgentEvent>) {
        let tools = crate::tools::all_tools();

        let model = match self.build_model().await {
            Ok(m) => m,
            Err(e) => {
                let _ = tx.send(AgentEvent::Error(e));
                let _ = tx.send(AgentEvent::Done);
                return;
            }
        };

        let mut req_builder = aisdk::core::LanguageModelRequest::<ModelProvider>::builder()
            .model(model)
            .system(&self.system_prompt)
            .prompt(&self.user_prompt);

        for tool in tools {
            req_builder = req_builder.with_tool(tool);
        }

        let mut request = req_builder
            .stop_when(aisdk::core::utils::step_count_is(self.max_steps))
            .build();

        match request.stream_text().await {
            Ok(response) => {
                let mut stream = response.stream;
                while let Some(chunk) = stream.next().await {
                    match chunk {
                        LanguageModelStreamChunkType::Text(text) => {
                            let _ = tx.send(AgentEvent::TextDelta(text));
                        }
                        LanguageModelStreamChunkType::Reasoning(text) => {
                            let _ = tx.send(AgentEvent::ReasoningDelta(text));
                        }
                        LanguageModelStreamChunkType::End(_) => {
                            let _ = tx.send(AgentEvent::Done);
                            return;
                        }
                        LanguageModelStreamChunkType::Failed(err) => {
                            let _ = tx.send(AgentEvent::Error(err));
                            let _ = tx.send(AgentEvent::Done);
                            return;
                        }
                        LanguageModelStreamChunkType::Incomplete(msg) => {
                            let _ = tx.send(AgentEvent::TextDelta(msg));
                            let _ = tx.send(AgentEvent::Done);
                            return;
                        }
                        _ => {}
                    }
                }
                // Stream ended without a terminal event (shouldn't normally happen)
                let _ = tx.send(AgentEvent::Done);
            }
            Err(e) => {
                let _ = tx.send(AgentEvent::Error(e.to_string()));
                let _ = tx.send(AgentEvent::Done);
            }
        }
    }
}
