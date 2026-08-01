use std::ops::Deref;

use aisdk::core::capabilities::{ReasoningSupport, ToolCallSupport};
use aisdk::core::language_model::{
    LanguageModelOptions, LanguageModelResponseContentType, LanguageModelStreamChunk,
    ReasoningEffort,
};
use aisdk::core::tools::{ToolCallInfo, ToolList, ToolResultInfo};
use aisdk::core::{
    AssistantMessage, LanguageModel, LanguageModelRequest, LanguageModelStreamChunkType, Message,
    Messages, SystemMessage, UserMessage,
};
use futures::StreamExt;
use tokio::sync::{broadcast, mpsc};
use tokio::time::{Duration, sleep};
use tracing::{info, warn};

use crate::agent::GenerationOutcome;
use crate::config::ApiFormat;
use crate::error::Result;
use crate::models::{ModelProvider, dispatch_model};
use crate::{AgentEvent, Session, SteerCommand, tools};

/// Internal agent runner holding all parameters needed to build and execute a model request.
pub struct AgentRunner {
    /// Display name of the configured provider.
    pub provider_name: String,
    /// Model id to run against.
    pub provider_model: String,
    /// Optional API key for the provider.
    pub provider_api_key: Option<String>,
    /// Provider endpoint base URL.
    pub provider_base_url: String,
    /// Wire format of the provider endpoint.
    pub api_format: ApiFormat,
    /// Compiled system prompt (context, tools, skills).
    pub system_prompt: String,
    /// The user's original prompt.
    pub user_prompt: String,
    /// Maximum tool-call steps before the run stops.
    pub max_steps: usize,
    /// Optional sampling temperature (0.0–1.0).
    pub temperature: Option<f32>,
    /// Optional cap on output tokens.
    pub max_tokens: Option<u32>,
    /// Optional reasoning effort override.
    pub reasoning_effort: Option<ReasoningEffort>,
    /// Enables debug logging and event persistence.
    pub debug: bool,
    /// Id of the session this run belongs to (for debug logs).
    pub session_id: String,
    /// Tools disabled for this session, excluded from the model.
    pub disabled_tools: std::collections::BTreeSet<String>,
    /// Number of retries per failed generation step.
    pub retry_attempts: usize,
    /// Delay in seconds between retries.
    pub retry_timeout_secs: u64,
    /// Project root whose filesystem is snapshotted before the run, enabling
    /// the UI to revert file changes when undoing the prompt.
    pub project_dir: Option<std::path::PathBuf>,
    /// Whether git-backed filesystem snapshots are enabled (see `Settings`).
    pub snapshots_enabled: bool,
}

impl AgentRunner {
    /// Validates that a provider and a model were selected before a run.
    ///
    /// Returns a `Config` error naming the missing selection so the caller can
    /// surface it to the user.
    pub fn validate_selection(&self) -> Result<()> {
        if self.provider_name.trim().is_empty() {
            return Err(crate::error::CustomError::Config(
                "No provider selected. Choose a provider before running the agent".to_string(),
            ));
        }
        if self.provider_model.trim().is_empty() {
            return Err(crate::error::CustomError::Config(format!(
                "No model selected for provider {}",
                self.provider_name
            )));
        }
        Ok(())
    }

    /// Builds a [`ModelProvider`] from the runner's config fields.
    pub async fn build_model(&self) -> Result<ModelProvider> {
        ModelProvider::build(
            &self.api_format,
            &self.provider_base_url,
            &self.provider_model,
            self.provider_api_key.clone(),
        )
        .await
    }

    /// Runs the agent with streaming — emits [`AgentEvent`]s into the channel.
    pub async fn execute(
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
        dispatch_model!(model, self, execute_loop, tx, session, steer_rx)
    }

    /// Runs one generation attempt: streams a response for `working` and
    /// surfaces text/reasoning deltas as events. Returns either the collected
    /// response pieces or a failure describing why the step did not produce a
    /// response.
    async fn generate_once<M: LanguageModel + ToolCallSupport + ReasoningSupport>(
        &self,
        model: &M,
        working: &Messages,
        tx: &broadcast::Sender<AgentEvent>,
    ) -> GenerationOutcome {
        let options = self.build_options(model.clone(), working.clone());
        let mut m = model.clone();
        let mut stream = match m.stream_text(options).await {
            Ok(s) => s,
            Err(e) => {
                return GenerationOutcome::Failed {
                    error: e.to_string(),
                    emitted_content: false,
                };
            }
        };

        let mut text_acc = String::new();
        let mut assistant_dones: Vec<AssistantMessage> = Vec::new();
        let mut tool_calls: Vec<(AssistantMessage, ToolCallInfo)> = Vec::new();
        let mut emitted_content = false;

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
                                    self.emit_event(AgentEvent::TextDelta(t.clone()), tx);
                                    text_acc.push_str(&t);
                                    emitted_content = true;
                                }
                                LanguageModelStreamChunkType::Reasoning(r) => {
                                    self.emit_event(AgentEvent::ReasoningDelta(r), tx);
                                    emitted_content = true;
                                }
                                LanguageModelStreamChunkType::ToolCall(_) => {}
                                LanguageModelStreamChunkType::End(msg) => record_done(msg),
                                LanguageModelStreamChunkType::Failed(err) => {
                                    return GenerationOutcome::Failed {
                                        error: err,
                                        emitted_content,
                                    };
                                }
                                LanguageModelStreamChunkType::Incomplete(msg) => {
                                    return GenerationOutcome::Failed {
                                        error: msg,
                                        emitted_content,
                                    };
                                }
                                LanguageModelStreamChunkType::NotSupported(_) => {}
                            },
                            LanguageModelStreamChunk::Done(msg) => record_done(msg),
                        }
                    }
                }
                Err(e) => {
                    return GenerationOutcome::Failed {
                        error: e.to_string(),
                        emitted_content,
                    };
                }
            }
        }

        GenerationOutcome::Response {
            text_acc,
            assistant_dones,
            tool_calls,
        }
    }

    /// Runs the agent with streaming, driving the model one generation at a
    /// time so the run can be paused and steered between steps.
    ///
    /// When `steer_rx` is `Some`, the run pauses before executing each proposed
    /// tool call and waits for a [`SteerCommand`]. Conversation messages are
    /// appended to `session` and persisted as the run progresses.
    async fn execute_loop<M: LanguageModel + ToolCallSupport + ReasoningSupport>(
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
        let user_msg_id = session.add_message("user".to_string(), self.user_prompt.clone());
        self.capture_before_snapshot(&mut session, &user_msg_id);

        let tool_list = ToolList::new(tools::all_tools(&self.disabled_tools));

        let mut steps = 0usize;

        'run: loop {
            if steps >= self.max_steps {
                self.emit_event(
                    AgentEvent::TextDelta("Stopped by max-steps hook".to_string()),
                    &tx,
                );
                break;
            }
            steps += 1;

            let (text_acc, assistant_dones, tool_calls) = {
                let mut attempt = 0usize;
                loop {
                    match self.generate_once(&model, &working, &tx).await {
                        GenerationOutcome::Response {
                            text_acc,
                            assistant_dones,
                            tool_calls,
                        } => break (text_acc, assistant_dones, tool_calls),
                        GenerationOutcome::Failed {
                            error,
                            emitted_content: true,
                        } => {
                            self.emit_event(
                                AgentEvent::Error(format!(
                                    "Model generation failed after streaming partial content: {error}"
                                )),
                                &tx,
                            );
                            break 'run;
                        }
                        GenerationOutcome::Failed { error, .. }
                            if attempt >= self.retry_attempts =>
                        {
                            self.emit_event(
                                AgentEvent::Error(format!(
                                    "Model generation failed after {} retries: {error}",
                                    self.retry_attempts
                                )),
                                &tx,
                            );
                            break 'run;
                        }
                        GenerationOutcome::Failed { error, .. } => {
                            attempt += 1;
                            warn!(
                                "Step generation failed (attempt {} of {}): {error}",
                                attempt, self.retry_attempts
                            );
                            self.emit_event(
                                AgentEvent::Retrying {
                                    attempt,
                                    error: error.clone(),
                                },
                                &tx,
                            );
                            sleep(Duration::from_secs(self.retry_timeout_secs)).await;
                        }
                    }
                }
            };

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
            // Injecting guidance never cancels a tool call; the guidance is
            // applied at the next safe point, after the tool results.
            let mut pending_steer: Option<String> = None;
            if let Some(rx) = steer_rx.as_mut()
                && let Some(cmd) = rx.recv().await
                && let SteerCommand::Inject(text) = cmd
            {
                pending_steer = Some(text);
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

            // Next safe point: inject queued steering after the tool results.
            if let Some(text) = pending_steer {
                working.push(Message::User(UserMessage::new(text.clone())));
                session.add_message("user".to_string(), text.clone());
                self.persist_session(&session);
                self.emit_event(AgentEvent::Steering(text), &tx);
            }
        }

        self.capture_after_snapshot(&mut session);
        self.persist_session(&session);
        self.emit_event(AgentEvent::Done, &tx);
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

    /// Starts a new run on the session: discards any pending undo history (a
    /// new prompt without a redo invalidates it), records the run against the
    /// user message `message_id`, and best-effort captures a filesystem
    /// snapshot of the project before the run so undoing this message can also
    /// revert the file changes. Failures (no project, not a git repository,
    /// snapshots disabled) are logged and skipped.
    fn capture_before_snapshot(&self, session: &mut Session, message_id: &str) {
        let dir = self
            .snapshots_enabled
            .then(|| self.project_dir.clone())
            .flatten();
        match session.begin_run(message_id, dir.as_deref()) {
            Some(id) => info!(
                "Captured filesystem snapshot {id} for session {}",
                session.id
            ),
            None => info!("No filesystem snapshot captured for session {}", session.id),
        }
    }

    /// Captures a snapshot of the project after the run and links it to the
    /// latest recorded run, so redo can reapply the file changes. Best-effort.
    fn capture_after_snapshot(&self, session: &mut Session) {
        if !self.snapshots_enabled {
            return;
        }
        let Some(dir) = self.project_dir.as_deref() else {
            return;
        };
        match crate::snapshot::capture_snapshot(dir) {
            Ok(snapshot) => {
                let id = snapshot.id.clone();
                session.set_after_snapshot(id.clone());
                info!(
                    "Captured after-run snapshot {id} for session {}",
                    session.id
                );
            }
            Err(e) => {
                warn!("After-run filesystem snapshot skipped: {e}");
            }
        }
    }

    /// Builds a single-generation `LanguageModelOptions` carrying the running
    /// conversation and the enabled tools.
    fn build_options<M: LanguageModel + ToolCallSupport + ReasoningSupport>(
        &self,
        model: M,
        messages: Messages,
    ) -> LanguageModelOptions {
        let tools = tools::all_tools(&self.disabled_tools);
        let mut builder = LanguageModelRequest::<M>::builder()
            .model(model)
            .messages(messages);
        if let Some(temp) = self.temperature {
            builder = builder.temperature((temp.clamp(0.0, 1.0) * 100.0).round() as u32);
        }
        if let Some(tokens) = self.max_tokens {
            builder.max_output_tokens = Some(tokens);
        }
        if let Some(effort) = self.reasoning_effort {
            builder = builder.reasoning_effort(effort);
        }
        for tool in tools {
            builder = builder.with_tool(tool);
        }
        builder.build().deref().clone()
    }
}
