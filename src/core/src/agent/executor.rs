//! Agent execution — model dispatching, streaming, event emission.

use aisdk::core::capabilities::{TextInputSupport, ToolCallSupport};
use aisdk::core::{DynamicModel, LanguageModel, LanguageModelStreamChunkType};
use aisdk::providers::{Anthropic, OpenAICompatible};
use futures::StreamExt;
use tokio::sync::broadcast;
use tracing::info;

use crate::config::ApiFormat;
use crate::error::Result;
use crate::session::Session;
use crate::tools;

type OpenAIModel = OpenAICompatible<DynamicModel>;
type AnthropicModel = Anthropic<DynamicModel>;

/// Events emitted by the agent during a streaming run.
#[derive(Debug, Clone)]
pub enum AgentEvent {
    /// A text delta from the model.
    TextDelta(String),
    /// A reasoning/thinking delta from the model.
    ReasoningDelta(String),
    /// A tool call has started.
    ToolCallStart {
        tool_name: String,
        args: serde_json::Value,
    },
    /// A tool call has completed.
    ToolCallEnd {
        tool_name: String,
        result: std::result::Result<String, String>,
    },
    /// An error occurred.
    Error(String),
    /// The agent run finished.
    Done,
}

/// An agent ready to run.
pub struct Agent {
    pub(crate) runner: Option<AgentRunner>,
    pub session: Session,
}

#[allow(dead_code)]
pub(crate) struct AgentRunner {
    pub provider_name: String,
    pub provider_model: String,
    pub provider_api_key: Option<String>,
    pub provider_base_url: String,
    pub api_format: ApiFormat,
    pub system_prompt: String,
    pub user_prompt: String,
    pub max_steps: usize,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
}

enum ModelProvider {
    OpenAI(Box<OpenAIModel>),
    Anthropic(Box<AnthropicModel>),
}

impl ModelProvider {
    async fn build(
        api_format: &ApiFormat,
        base_url: &str,
        model_name: &str,
        api_key: Option<String>,
    ) -> std::result::Result<Self, String> {
        match api_format {
            ApiFormat::OpenAICompatible | ApiFormat::OpenAI | ApiFormat::Google => {
                Self::build_openai(base_url, model_name, api_key).await
            }
            ApiFormat::Anthropic => Self::build_anthropic(base_url, model_name, api_key).await,
            ApiFormat::Other(_) => Self::build_openai(base_url, model_name, api_key).await,
        }
    }

    async fn build_openai(
        base_url: &str,
        model_name: &str,
        api_key: Option<String>,
    ) -> std::result::Result<Self, String> {
        let mut builder = OpenAICompatible::<DynamicModel>::builder()
            .base_url(base_url)
            .model_name(model_name);
        if let Some(key) = api_key {
            builder = builder.api_key(key);
        }
        builder
            .build()
            .map(|m| Self::OpenAI(Box::new(m)))
            .map_err(|e| format!("Failed to build OpenAI-compatible model: {e}"))
    }

    async fn build_anthropic(
        base_url: &str,
        model_name: &str,
        api_key: Option<String>,
    ) -> std::result::Result<Self, String> {
        let mut builder = Anthropic::<DynamicModel>::builder()
            .base_url(base_url)
            .model_name(model_name);
        if let Some(key) = api_key {
            builder = builder.api_key(key);
        }
        builder
            .build()
            .map(|m| Self::Anthropic(Box::new(m)))
            .map_err(|e| format!("Failed to build Anthropic model: {e}"))
    }

    async fn execute(
        self,
        system_prompt: &str,
        user_prompt: &str,
        max_steps: usize,
    ) -> std::result::Result<(), String> {
        match self {
            Self::OpenAI(model) => {
                execute_model(*model, system_prompt, user_prompt, max_steps).await
            }
            Self::Anthropic(model) => {
                execute_model(*model, system_prompt, user_prompt, max_steps).await
            }
        }
    }

    async fn execute_stream(
        self,
        system_prompt: &str,
        user_prompt: &str,
        max_steps: usize,
        tx: broadcast::Sender<AgentEvent>,
    ) {
        match self {
            Self::OpenAI(model) => {
                execute_model_stream(*model, system_prompt, user_prompt, max_steps, tx).await
            }
            Self::Anthropic(model) => {
                execute_model_stream(*model, system_prompt, user_prompt, max_steps, tx).await
            }
        }
    }
}

async fn execute_model<M: LanguageModel + TextInputSupport + ToolCallSupport>(
    model: M,
    system_prompt: &str,
    user_prompt: &str,
    max_steps: usize,
) -> std::result::Result<(), String> {
    let tools = tools::all_tools();

    let mut req_builder = aisdk::core::LanguageModelRequest::<M>::builder()
        .model(model)
        .system(system_prompt)
        .prompt(user_prompt);

    for tool in tools {
        req_builder = req_builder.with_tool(tool);
    }

    let mut request = req_builder
        .stop_when(aisdk::core::utils::step_count_is(max_steps))
        .build();

    request.generate_text().await.map_err(|e| e.to_string())?;
    Ok(())
}

async fn execute_model_stream<M: LanguageModel + TextInputSupport + ToolCallSupport>(
    model: M,
    system_prompt: &str,
    user_prompt: &str,
    max_steps: usize,
    tx: broadcast::Sender<AgentEvent>,
) {
    let tools = tools::all_tools();

    let mut req_builder = aisdk::core::LanguageModelRequest::<M>::builder()
        .model(model)
        .system(system_prompt)
        .prompt(user_prompt);

    for tool in tools {
        req_builder = req_builder.with_tool(tool);
    }

    let mut request = req_builder
        .stop_when(aisdk::core::utils::step_count_is(max_steps))
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
            let _ = tx.send(AgentEvent::Done);
        }
        Err(e) => {
            let _ = tx.send(AgentEvent::Error(e.to_string()));
            let _ = tx.send(AgentEvent::Done);
        }
    }
}

impl Agent {
    /// Creates a new `AgentBuilder`.
    pub fn builder() -> super::AgentBuilder {
        super::AgentBuilder::new()
    }

    /// Runs the agent and returns a channel receiver for [`AgentEvent`]s.
    ///
    /// The agent is consumed (may only be run once).
    pub async fn run(&mut self) -> Result<broadcast::Receiver<AgentEvent>> {
        let runner = self
            .runner
            .take()
            .ok_or_else(|| crate::error::BimoError::Agent("Agent already consumed".to_string()))?;

        let (tx, rx) = broadcast::channel(256);

        let todos: tools::SharedTodoList = tools::new_shared_todolist();
        tools::init_todo_list(todos.clone());

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

    /// Runs the agent with streaming and returns a channel receiver.
    ///
    /// The agent is consumed (may only be run once).
    pub async fn run_stream(&mut self) -> Result<broadcast::Receiver<AgentEvent>> {
        let runner = self
            .runner
            .take()
            .ok_or_else(|| crate::error::BimoError::Agent("Agent already consumed".to_string()))?;

        let (tx, rx) = broadcast::channel(256);

        let todos: tools::SharedTodoList = tools::new_shared_todolist();
        tools::init_todo_list(todos.clone());

        let session_id = self.session.id.clone();

        tokio::spawn(async move {
            info!("Starting agent stream session: {}", session_id);
            runner.execute_stream(tx).await;
        });

        Ok(rx)
    }

    /// Runs the agent and collects the output as a plain string.
    ///
    /// The agent is consumed (may only be run once).
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
    async fn build_model(&self) -> std::result::Result<ModelProvider, String> {
        ModelProvider::build(
            &self.api_format,
            &self.provider_base_url,
            &self.provider_model,
            self.provider_api_key.clone(),
        )
        .await
    }

    async fn execute(self) -> std::result::Result<(), String> {
        let model = self.build_model().await?;
        model
            .execute(&self.system_prompt, &self.user_prompt, self.max_steps)
            .await
    }

    async fn execute_stream(self, tx: broadcast::Sender<AgentEvent>) {
        let model = match self.build_model().await {
            Ok(m) => m,
            Err(e) => {
                let _ = tx.send(AgentEvent::Error(e));
                let _ = tx.send(AgentEvent::Done);
                return;
            }
        };
        model
            .execute_stream(&self.system_prompt, &self.user_prompt, self.max_steps, tx)
            .await;
    }
}
