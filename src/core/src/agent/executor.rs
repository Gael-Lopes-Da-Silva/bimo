use std::sync::Arc;

use aisdk::core::DynamicModel;
use aisdk::providers::OpenAICompatible;
use tokio::sync::broadcast;
use tracing::info;

use crate::config::LocalProvider;
use crate::error::Result;
use crate::models::ModelRegistry;
use crate::session::Session;

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
    async fn execute(self) -> std::result::Result<(), String> {
        let tools = crate::tools::all_tools();

        // Resolve base URL: explicit config → local provider → registry → default
        let base_url = if let Some(url) = &self.provider_base_url {
            url.clone()
        } else if let Some(local) = self
            .local_providers
            .iter()
            .find(|p| p.name.to_lowercase() == self.provider_name.to_lowercase())
        {
            local.base_url.clone()
        } else if let Some(registry) = &self.registry {
            registry
                .provider_base_url(&self.provider_name)
                .await
                .unwrap_or_else(|| "https://api.openai.com/v1".to_string())
        } else {
            "https://api.openai.com/v1".to_string()
        };

        let mut builder_cfg = OpenAICompatible::<DynamicModel>::builder()
            .base_url(base_url)
            .model_name(&self.provider_model);

        if let Some(key) = &self.provider_api_key {
            builder_cfg = builder_cfg.api_key(key.clone());
        }

        let model = builder_cfg
            .build()
            .map_err(|e| format!("Failed to build provider: {e}"))?;

        let mut req_builder = aisdk::core::LanguageModelRequest::builder()
            .model(model)
            .system(&self.system_prompt)
            .prompt("I am ready to begin. Please provide your first task.");

        for tool in tools {
            req_builder = req_builder.with_tool(tool);
        }

        let mut request = req_builder
            .stop_when(aisdk::core::utils::step_count_is(self.max_steps))
            .build();

        request.generate_text().await.map_err(|e| e.to_string())?;

        Ok(())
    }
}
