//! Agent execution — model dispatching, streaming, event emission.

use aisdk::core::capabilities::{TextInputSupport, ToolCallSupport};
use aisdk::core::{
    DynamicModel, LanguageModel, LanguageModelRequest, LanguageModelStreamChunkType,
};
use aisdk::providers::{Anthropic, Google, OpenAICompatible};
use futures::StreamExt;
use tokio::sync::broadcast;
use tracing::info;

use crate::config::ApiFormat;
use crate::error::Result;
use crate::prompt::PromptEngine;
use crate::session::Session;
use crate::tools;

/// Erased OpenAI-compatible model type used at runtime.
type OpenAIModel = OpenAICompatible<DynamicModel>;
/// Erased Anthropic model type used at runtime.
type AnthropicModel = Anthropic<DynamicModel>;
/// Erased Google model type used at runtime.
type GoogleModel = Google<DynamicModel>;

/// Events emitted by the agent during a streaming run.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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

/// Internal agent runner holding all parameters needed to build and execute a model request.
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
    pub debug: bool,
    pub session_id: String,
    pub retry_attempts: usize,
    pub retry_timeout_secs: u64,
}

/// Erased model type — dispatches to the concrete provider SDK at build time.
enum ModelProvider {
    OpenAI(Box<OpenAIModel>),
    Anthropic(Box<AnthropicModel>),
    Google(Box<GoogleModel>),
}

impl ModelProvider {
    /// Builds the appropriate model variant from config fields.
    async fn build(
        api_format: &ApiFormat,
        base_url: &str,
        model_name: &str,
        api_key: Option<String>,
    ) -> std::result::Result<Self, String> {
        match api_format {
            ApiFormat::OpenAICompatible | ApiFormat::OpenAI => {
                Self::build_openai(base_url, model_name, api_key).await
            }
            ApiFormat::Google => Self::build_google(base_url, model_name, api_key).await,
            ApiFormat::Anthropic => Self::build_anthropic(base_url, model_name, api_key).await,
            ApiFormat::Other(fmt) => Err(format!("unsupported API format: {fmt}")),
        }
    }

    /// Builds an OpenAI-compatible model client.
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

    /// Builds an Anthropic model client.
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

    /// Builds a Google model client.
    async fn build_google(
        base_url: &str,
        model_name: &str,
        api_key: Option<String>,
    ) -> std::result::Result<Self, String> {
        let mut builder = Google::<DynamicModel>::builder()
            .base_url(base_url)
            .model_name(model_name);
        if let Some(key) = api_key {
            builder = builder.api_key(key);
        }
        builder
            .build()
            .map(|m| Self::Google(Box::new(m)))
            .map_err(|e| format!("Failed to build Google model: {e}"))
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

    /// Compacts the current session's message history into a summary.
    ///
    /// 1. Renders COMPACT.md with the full conversation.
    /// 2. Calls the model (no tools, no stop condition) to get a summary.
    /// 3. Renders SUMMARY.md with that summary.
    /// 4. Clears session.messages.
    /// 5. Injects the rendered summary as a system message.
    /// 6. Persists the session.
    ///
    /// Returns the summary text on success.
    pub async fn compact(&mut self) -> Result<String> {
        let runner = self
            .runner
            .as_ref()
            .ok_or_else(|| crate::error::BimoError::Agent("Agent already consumed".to_string()))?;

        if self.session.messages.is_empty() {
            return Ok(String::new());
        }

        let conversation = PromptEngine::format_messages(&self.session.messages);
        let compact_prompt = PromptEngine::render_compact(&conversation);

        let model = runner.build_model().await.map_err(|e| {
            crate::error::BimoError::Agent(format!("Compaction model build failed: {}", e))
        })?;
        match model {
            ModelProvider::OpenAI(model) => self.compact_with_model(*model, &compact_prompt).await,
            ModelProvider::Anthropic(model) => {
                self.compact_with_model(*model, &compact_prompt).await
            }
            ModelProvider::Google(model) => self.compact_with_model(*model, &compact_prompt).await,
        }
    }

    async fn compact_with_model<M>(&mut self, model: M, conversation_prompt: &str) -> Result<String>
    where
        M: LanguageModel + TextInputSupport,
    {
        let mut request = LanguageModelRequest::<M>::builder()
            .model(model)
            .prompt(conversation_prompt)
            .build();

        let response = request
            .generate_text()
            .await
            .map_err(|e| crate::error::BimoError::Agent(format!("Compaction failed: {}", e)))?;

        let summary = response.text().unwrap_or_default();
        let rendered_summary = PromptEngine::render_summary(&summary);

        self.session.messages.clear();
        self.session
            .add_message("system".to_string(), rendered_summary);
        self.session.save()?;

        Ok(summary)
    }
}

impl AgentRunner {
    /// Builds a fully-configured `LanguageModelRequest` (tools + stop condition).
    fn build_request<M: LanguageModel + TextInputSupport + ToolCallSupport>(
        &self,
        model: M,
    ) -> LanguageModelRequest<M> {
        let tools = tools::all_tools();
        let mut req_builder = LanguageModelRequest::<M>::builder()
            .model(model)
            .system(&self.system_prompt)
            .prompt(&self.user_prompt);
        for tool in tools {
            req_builder = req_builder.with_tool(tool);
        }
        req_builder
            .stop_when(aisdk::core::utils::step_count_is(self.max_steps))
            .build()
    }

    /// Runs the model non-streaming — all tool calls complete before returning.
    async fn execute_model<M: LanguageModel + TextInputSupport + ToolCallSupport>(
        &self,
        model: M,
    ) -> std::result::Result<(), String> {
        let mut request = self.build_request(model);
        request.generate_text().await.map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Runs the model with streaming, emitting [`AgentEvent`]s into the channel.
    async fn execute_model_stream<M: LanguageModel + TextInputSupport + ToolCallSupport>(
        &self,
        model: M,
        tx: broadcast::Sender<AgentEvent>,
    ) {
        let debug_path = if self.debug {
            Some(Session::sessions_dir().join(format!("{}_events.json", self.session_id)))
        } else {
            None
        };
        let mut request = self.build_request(model);
        match request.stream_text().await {
            Ok(response) => {
                let mut stream = response.stream;
                while let Some(chunk) = stream.next().await {
                    match chunk {
                        LanguageModelStreamChunkType::Text(text) => {
                            let event = AgentEvent::TextDelta(text);
                            if self.debug {
                                if let Some(path) = &debug_path {
                                    let mut file = std::fs::OpenOptions::new()
                                        .create(true)
                                        .append(true)
                                        .open(path)
                                        .unwrap();
                                    use std::io::Write;
                                    let _ = writeln!(
                                        file,
                                        "{}",
                                        serde_json::to_string(&event).unwrap_or_default()
                                    );
                                }
                            }
                            let _ = tx.send(event);
                        }
                        LanguageModelStreamChunkType::Reasoning(text) => {
                            let event = AgentEvent::ReasoningDelta(text);
                            if self.debug {
                                if let Some(path) = &debug_path {
                                    let mut file = std::fs::OpenOptions::new()
                                        .create(true)
                                        .append(true)
                                        .open(path)
                                        .unwrap();
                                    use std::io::Write;
                                    let _ = writeln!(
                                        file,
                                        "{}",
                                        serde_json::to_string(&event).unwrap_or_default()
                                    );
                                }
                            }
                            let _ = tx.send(event);
                        }
                        LanguageModelStreamChunkType::End(_) => {
                            let event = AgentEvent::Done;
                            if self.debug {
                                if let Some(path) = &debug_path {
                                    let mut file = std::fs::OpenOptions::new()
                                        .create(true)
                                        .append(true)
                                        .open(path)
                                        .unwrap();
                                    use std::io::Write;
                                    let _ = writeln!(
                                        file,
                                        "{}",
                                        serde_json::to_string(&event).unwrap_or_default()
                                    );
                                }
                            }
                            let _ = tx.send(event);
                            return;
                        }
                        LanguageModelStreamChunkType::Failed(err) => {
                            let event = AgentEvent::Error(err);
                            if self.debug {
                                if let Some(path) = &debug_path {
                                    let mut file = std::fs::OpenOptions::new()
                                        .create(true)
                                        .append(true)
                                        .open(path)
                                        .unwrap();
                                    use std::io::Write;
                                    let _ = writeln!(
                                        file,
                                        "{}",
                                        serde_json::to_string(&event).unwrap_or_default()
                                    );
                                }
                            }
                            let done = AgentEvent::Done;
                            if self.debug {
                                if let Some(path) = &debug_path {
                                    let mut file = std::fs::OpenOptions::new()
                                        .create(true)
                                        .append(true)
                                        .open(path)
                                        .unwrap();
                                    use std::io::Write;
                                    let _ = writeln!(
                                        file,
                                        "{}",
                                        serde_json::to_string(&done).unwrap_or_default()
                                    );
                                }
                            }
                            let _ = tx.send(event);
                            let _ = tx.send(done);
                            return;
                        }
                        LanguageModelStreamChunkType::Incomplete(msg) => {
                            let event = AgentEvent::TextDelta(msg);
                            if self.debug {
                                if let Some(path) = &debug_path {
                                    let mut file = std::fs::OpenOptions::new()
                                        .create(true)
                                        .append(true)
                                        .open(path)
                                        .unwrap();
                                    use std::io::Write;
                                    let _ = writeln!(
                                        file,
                                        "{}",
                                        serde_json::to_string(&event).unwrap_or_default()
                                    );
                                }
                            }
                            let done = AgentEvent::Done;
                            if self.debug {
                                if let Some(path) = &debug_path {
                                    let mut file = std::fs::OpenOptions::new()
                                        .create(true)
                                        .append(true)
                                        .open(path)
                                        .unwrap();
                                    use std::io::Write;
                                    let _ = writeln!(
                                        file,
                                        "{}",
                                        serde_json::to_string(&done).unwrap_or_default()
                                    );
                                }
                            }
                            let _ = tx.send(event);
                            let _ = tx.send(done);
                            return;
                        }
                        _ => {}
                    }
                }
                let event = AgentEvent::Done;
                if self.debug {
                    if let Some(path) = &debug_path {
                        let mut file = std::fs::OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open(path)
                            .unwrap();
                        use std::io::Write;
                        let _ = writeln!(
                            file,
                            "{}",
                            serde_json::to_string(&event).unwrap_or_default()
                        );
                    }
                }
                let _ = tx.send(event);
            }
            Err(e) => {
                let error_event = AgentEvent::Error(e.to_string());
                if self.debug {
                    if let Some(path) = &debug_path {
                        let mut file = std::fs::OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open(path)
                            .unwrap();
                        use std::io::Write;
                        let _ = writeln!(
                            file,
                            "{}",
                            serde_json::to_string(&error_event).unwrap_or_default()
                        );
                    }
                }
                let done = AgentEvent::Done;
                if self.debug {
                    if let Some(path) = &debug_path {
                        let mut file = std::fs::OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open(path)
                            .unwrap();
                        use std::io::Write;
                        let _ =
                            writeln!(file, "{}", serde_json::to_string(&done).unwrap_or_default());
                    }
                }
                let _ = tx.send(error_event);
                let _ = tx.send(done);
            }
        }
    }

    /// Builds a [`ModelProvider`] from the runner's config fields.
    async fn build_model(&self) -> std::result::Result<ModelProvider, String> {
        ModelProvider::build(
            &self.api_format,
            &self.provider_base_url,
            &self.provider_model,
            self.provider_api_key.clone(),
        )
        .await
    }

    /// Runs the agent (non-streaming) — builds the model then executes.
    async fn execute(self) -> std::result::Result<(), String> {
        match self.build_model().await? {
            ModelProvider::OpenAI(model) => self.execute_model(*model).await,
            ModelProvider::Anthropic(model) => self.execute_model(*model).await,
            ModelProvider::Google(model) => self.execute_model(*model).await,
        }
    }

    /// Runs the agent with streaming — emits [`AgentEvent`]s into the channel.
    async fn execute_stream(self, tx: broadcast::Sender<AgentEvent>) {
        let debug_path = if self.debug {
            Some(Session::sessions_dir().join(format!("{}_events.json", self.session_id)))
        } else {
            None
        };
        let model = match self.build_model().await {
            Ok(m) => m,
            Err(e) => {
                let event = AgentEvent::Error(e);
                if self.debug {
                    if let Some(path) = &debug_path {
                        let mut file = std::fs::OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open(path)
                            .unwrap();
                        use std::io::Write;
                        let _ = writeln!(
                            file,
                            "{}",
                            serde_json::to_string(&event).unwrap_or_default()
                        );
                    }
                }
                let _ = tx.send(event.clone());
                let _ = tx.send(AgentEvent::Done);
                return;
            }
        };
        match model {
            ModelProvider::OpenAI(m) => self.execute_model_stream(*m, tx).await,
            ModelProvider::Anthropic(m) => self.execute_model_stream(*m, tx).await,
            ModelProvider::Google(m) => self.execute_model_stream(*m, tx).await,
        }
    }
}
