//! Agent execution — model dispatching, streaming, event emission.

use std::ops::Deref;

use aisdk::core::capabilities::{TextInputSupport, ToolCallSupport};
use aisdk::core::language_model::{
    LanguageModelOptions, LanguageModelResponseContentType, LanguageModelStreamChunk,
};
use aisdk::core::tools::{ToolCallInfo, ToolList, ToolResultInfo};
use aisdk::core::{
    AssistantMessage, LanguageModel, LanguageModelRequest, LanguageModelStreamChunkType, Message,
    Messages, SystemMessage, UserMessage,
};
use futures::StreamExt;
use tokio::sync::{broadcast, mpsc};
use tracing::info;

use crate::config::ApiFormat;
use crate::error::{BimoError, Result};
use crate::models::{ModelProvider, dispatch_model};
use crate::prompt::PromptEngine;
use crate::session::Session;
use crate::tools;

/// Commands sent to a steerable agent run while it is paused between steps.
#[derive(Debug, Clone)]
pub enum SteerCommand {
    /// Execute the proposed tool call(s) and continue the run.
    Continue,
    /// Inject guidance as a user message and re-plan; the pending tool
    /// call(s) are cancelled.
    Inject(String),
}

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
    /// A proposed tool call was cancelled by a steering instruction.
    ToolCallCancelled { tool_name: String },
    /// A steering instruction was injected by the user mid-run.
    Steering(String),
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
    pub disabled_tools: std::collections::BTreeSet<String>,
    pub retry_attempts: usize,
    pub retry_timeout_secs: u64,
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
                    let _ = tx.send(AgentEvent::Error(e.to_string()));
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
        let session = self.session.clone();

        tokio::spawn(async move {
            info!("Starting agent stream session: {}", session_id);
            runner.execute_stream(tx, session, None).await;
        });

        Ok(rx)
    }

    /// Runs the agent with streaming and returns both the event receiver and a
    /// steer channel.
    ///
    /// The run pauses before each proposed tool call; the caller decides when
    /// to send [`SteerCommand::Continue`] (execute the tool and proceed) or
    /// [`SteerCommand::Inject`] (discard the tool call, inject guidance, and
    /// re-plan). Dropping the sender resumes execution.
    ///
    /// The agent is consumed (may only be run once).
    pub async fn run_stream_steerable(
        &mut self,
    ) -> Result<(broadcast::Receiver<AgentEvent>, mpsc::Sender<SteerCommand>)> {
        let runner = self
            .runner
            .take()
            .ok_or_else(|| crate::error::BimoError::Agent("Agent already consumed".to_string()))?;

        let (tx, rx) = broadcast::channel(256);
        let (steer_tx, steer_rx) = mpsc::channel(64);

        let todos: tools::SharedTodoList = tools::new_shared_todolist();
        tools::init_todo_list(todos.clone());

        let session_id = self.session.id.clone();
        let session = self.session.clone();

        tokio::spawn(async move {
            info!("Starting steerable agent stream session: {}", session_id);
            runner.execute_stream(tx, session, Some(steer_rx)).await;
        });

        Ok((rx, steer_tx))
    }

    /// Generates a concise session name/title using the model and session context.
    pub async fn generate_session_name(&mut self) -> Result<String> {
        let runner = self
            .runner
            .as_ref()
            .ok_or_else(|| crate::error::BimoError::Agent("Agent already consumed".to_string()))?;

        let conversation = if self.session.messages.is_empty() {
            return Err(crate::error::BimoError::Agent(
                "Cannot generate session name: no messages in session".to_string(),
            ));
        } else {
            PromptEngine::format_messages(&self.session.messages)
        };
        let name_prompt = PromptEngine::render_session_name(&conversation);

        let model = runner.build_model().await.map_err(|e| {
            crate::error::BimoError::Agent(format!("Session naming model build failed: {}", e))
        })?;
        dispatch_model!(model, self, name_with_model, &name_prompt)
    }

    async fn name_with_model<M>(&mut self, model: M, name_prompt: &str) -> Result<String>
    where
        M: LanguageModel + TextInputSupport,
    {
        let mut request = LanguageModelRequest::<M>::builder()
            .model(model)
            .prompt(name_prompt)
            .build();

        let response = request
            .generate_text()
            .await
            .map_err(|e| crate::error::BimoError::Agent(format!("Session naming failed: {}", e)))?;

        let title = response.text().unwrap_or_default().trim().to_string();
        Ok(title)
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
        dispatch_model!(model, self, compact_with_model, &compact_prompt)
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
        let tools = tools::all_tools(&self.disabled_tools);
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
    ) -> Result<()> {
        let mut request = self.build_request(model);
        request
            .generate_text()
            .await
            .map_err(|e| BimoError::Agent(e.to_string()))?;
        Ok(())
    }

    /// Writes an event to the debug log file when debug mode is enabled.
    fn persist_event(&self, event: &AgentEvent) {
        if self.debug {
            let path = Session::sessions_dir().join(format!("{}_events.json", self.session_id));
            if let Some(parent) = path.parent()
                && let Err(e) = std::fs::create_dir_all(parent)
            {
                tracing::warn!("Failed to create debug log directory: {e}");
                return;
            }
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
            {
                use std::io::Write;
                let _ = writeln!(file, "{}", serde_json::to_string(event).unwrap_or_default());
            }
        }
    }

    /// Sends an event and persists it to the debug log when enabled.
    fn emit_event(&self, event: AgentEvent, tx: &broadcast::Sender<AgentEvent>) {
        self.persist_event(&event);
        let _ = tx.send(event);
    }

    /// Best-effort persistence of the running conversation.
    fn persist_session(&self, session: &Session) {
        if let Err(e) = session.save() {
            tracing::warn!("Failed to persist session {}: {}", session.id, e);
        }
    }

    /// Builds a [`ModelProvider`] from the runner's config fields.
    async fn build_model(&self) -> Result<ModelProvider> {
        ModelProvider::build(
            &self.api_format,
            &self.provider_base_url,
            &self.provider_model,
            self.provider_api_key.clone(),
        )
        .await
    }

    /// Runs the agent (non-streaming) — builds the model then executes.
    async fn execute(self) -> Result<()> {
        dispatch_model!(self.build_model().await?, self, execute_model)
    }

    /// Runs the agent with streaming — emits [`AgentEvent`]s into the channel.
    async fn execute_stream(
        self,
        tx: broadcast::Sender<AgentEvent>,
        session: Session,
        steer_rx: Option<mpsc::Receiver<SteerCommand>>,
    ) {
        let model = match self.build_model().await {
            Ok(m) => m,
            Err(e) => {
                self.emit_event(AgentEvent::Error(e.to_string()), &tx);
                self.emit_event(AgentEvent::Done, &tx);
                return;
            }
        };
        dispatch_model!(model, self, execute_stream_loop, tx, session, steer_rx)
    }

    /// Builds a single-generation `LanguageModelOptions` carrying the running
    /// conversation and the enabled tools.
    fn build_stream_options<M: LanguageModel + ToolCallSupport>(
        &self,
        model: M,
        messages: Messages,
    ) -> LanguageModelOptions {
        let tools = tools::all_tools(&self.disabled_tools);
        let mut builder = LanguageModelRequest::<M>::builder()
            .model(model)
            .messages(messages);
        for tool in tools {
            builder = builder.with_tool(tool);
        }
        builder.build().deref().clone()
    }

    /// Runs the agent with streaming, driving the model one generation at a
    /// time so the run can be paused and steered between steps.
    ///
    /// When `steer_rx` is `Some`, the run pauses before executing each proposed
    /// tool call and waits for a [`SteerCommand`]. Conversation messages are
    /// appended to `session` and persisted as the run progresses.
    async fn execute_stream_loop<M: LanguageModel + ToolCallSupport>(
        self,
        model: M,
        tx: broadcast::Sender<AgentEvent>,
        mut session: Session,
        mut steer_rx: Option<mpsc::Receiver<SteerCommand>>,
    ) {
        let mut working: Messages = Vec::new();
        if !self.system_prompt.is_empty() {
            working.push(Message::System(SystemMessage::new(
                self.system_prompt.clone(),
            )));
        }
        working.push(Message::User(UserMessage::new(self.user_prompt.clone())));
        session.add_message("user".to_string(), self.user_prompt.clone());

        let tool_list = ToolList::new(tools::all_tools(&self.disabled_tools));

        let mut steps = 0usize;

        loop {
            if steps >= self.max_steps {
                self.emit_event(
                    AgentEvent::TextDelta("Stopped by max-steps hook".to_string()),
                    &tx,
                );
                break;
            }
            steps += 1;

            let options = self.build_stream_options(model.clone(), working.clone());
            let mut m = model.clone();
            let mut stream = match m.stream_text(options).await {
                Ok(s) => s,
                Err(e) => {
                    self.emit_event(
                        AgentEvent::Error(format!("Model streaming failed: {e}")),
                        &tx,
                    );
                    break;
                }
            };

            let mut text_acc = String::new();
            let mut assistant_dones: Vec<AssistantMessage> = Vec::new();
            let mut tool_calls: Vec<(AssistantMessage, ToolCallInfo)> = Vec::new();
            let mut errored = false;

            let mut record_done = |msg: AssistantMessage| {
                if let LanguageModelResponseContentType::ToolCall(info) = &msg.content {
                    tool_calls.push((msg.clone(), info.clone()));
                }
                assistant_dones.push(msg);
            };

            while let Some(chunk) = stream.next().await {
                match chunk {
                    Ok(chunks) => {
                        for c in chunks {
                            match c {
                                LanguageModelStreamChunk::Delta(delta) => match delta {
                                    LanguageModelStreamChunkType::Start => {}
                                    LanguageModelStreamChunkType::Text(t) => {
                                        self.emit_event(AgentEvent::TextDelta(t.clone()), &tx);
                                        text_acc.push_str(&t);
                                    }
                                    LanguageModelStreamChunkType::Reasoning(r) => {
                                        self.emit_event(AgentEvent::ReasoningDelta(r), &tx);
                                    }
                                    LanguageModelStreamChunkType::ToolCall(_) => {}
                                    LanguageModelStreamChunkType::End(msg) => record_done(msg),
                                    LanguageModelStreamChunkType::Failed(err) => {
                                        self.emit_event(AgentEvent::Error(err), &tx);
                                        errored = true;
                                    }
                                    LanguageModelStreamChunkType::Incomplete(msg) => {
                                        self.emit_event(AgentEvent::TextDelta(msg), &tx);
                                    }
                                    LanguageModelStreamChunkType::NotSupported(_) => {}
                                },
                                LanguageModelStreamChunk::Done(msg) => record_done(msg),
                            }
                        }
                    }
                    Err(e) => {
                        self.emit_event(AgentEvent::Error(e.to_string()), &tx);
                        errored = true;
                        break;
                    }
                }
            }

            if errored {
                break;
            }

            if tool_calls.is_empty() {
                for msg in &assistant_dones {
                    working.push(Message::Assistant(msg.clone()));
                }
                let final_text = if !text_acc.is_empty() {
                    text_acc
                } else if let Some(AssistantMessage {
                    content: LanguageModelResponseContentType::Reasoning { content, .. },
                    ..
                }) = assistant_dones.last()
                {
                    content.clone()
                } else {
                    String::new()
                };
                if !final_text.is_empty() {
                    session.add_message("assistant".to_string(), final_text);
                }
                self.persist_session(&session);
                break;
            }

            // Pause point: wait for a steering decision before executing tools.
            if let Some(rx) = steer_rx.as_mut() {
                let mut decision = None;
                if let Some(cmd) = rx.recv().await
                    && let SteerCommand::Inject(text) = cmd
                {
                    decision = Some(text);
                }
                if let Some(text) = decision {
                    for (msg, _) in &tool_calls {
                        if let LanguageModelResponseContentType::ToolCall(info) = &msg.content {
                            self.emit_event(
                                AgentEvent::ToolCallCancelled {
                                    tool_name: info.tool.name.clone(),
                                },
                                &tx,
                            );
                        }
                    }
                    for msg in &assistant_dones {
                        if !matches!(msg.content, LanguageModelResponseContentType::ToolCall(_)) {
                            working.push(Message::Assistant(msg.clone()));
                        }
                    }
                    working.push(Message::User(UserMessage::new(text.clone())));
                    session.add_message("user".to_string(), text.clone());
                    self.persist_session(&session);
                    self.emit_event(AgentEvent::Steering(text), &tx);
                    continue;
                }
            }

            for msg in &assistant_dones {
                working.push(Message::Assistant(msg.clone()));
            }

            for (_, info) in &tool_calls {
                self.emit_event(
                    AgentEvent::ToolCallStart {
                        tool_name: info.tool.name.clone(),
                        args: info.input.clone(),
                    },
                    &tx,
                );

                let handle = tool_list.execute(info.clone()).await;
                let result: std::result::Result<String, String> = match handle.await {
                    Ok(Ok(text)) => Ok(text),
                    Ok(Err(e)) => Err(e.to_string()),
                    Err(e) => Err(format!("Tool task failed: {e}")),
                };

                let mut tool_result = ToolResultInfo::new(&info.tool.name);
                tool_result.id(&info.tool.id);
                let output = match &result {
                    Ok(text) => serde_json::Value::String(text.clone()),
                    Err(e) => serde_json::Value::String(format!("Error: {e}")),
                };
                tool_result.output(output);
                working.push(Message::Tool(tool_result));

                let result_text = match &result {
                    Ok(text) => text.clone(),
                    Err(e) => e.clone(),
                };
                session.add_message(
                    "tool".to_string(),
                    format!("{}({}): {}", info.tool.name, info.tool.id, result_text),
                );
                self.persist_session(&session);

                self.emit_event(
                    AgentEvent::ToolCallEnd {
                        tool_name: info.tool.name.clone(),
                        result,
                    },
                    &tx,
                );
            }
        }

        self.persist_session(&session);
        self.emit_event(AgentEvent::Done, &tx);
    }
}
