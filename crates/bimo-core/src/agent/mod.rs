mod builder;
mod runner;

pub use builder::AgentBuilder;

use aisdk::core::capabilities::TextInputSupport;
use aisdk::core::tools::ToolCallInfo;
use aisdk::core::{AssistantMessage, LanguageModel, LanguageModelRequest};
use tokio::sync::{broadcast, mpsc};
use tracing::info;

use crate::agent::runner::AgentRunner;
use crate::error::Result;
use crate::models::dispatch_model;
use crate::prompt::PromptEngine;
use crate::session::Session;
use crate::tools;

/// Commands sent to a steerable agent run while it is paused between steps.
#[derive(Debug, Clone)]
pub enum SteerCommand {
    /// Execute the proposed tool call(s) and continue the run.
    Continue,
    /// Let the pending tool call(s) run to completion, then inject the
    /// guidance as a user message before the next step.
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
    /// A steering instruction was injected by the user mid-run.
    Steering(String),
    /// A failed generation step is being retried (attempt number, error).
    Retrying { attempt: usize, error: String },
    /// An error occurred.
    Error(String),
    /// The agent run finished.
    Done,
}

/// The result of a single generation attempt.
pub enum GenerationOutcome {
    /// A complete response was received.
    Response {
        text_acc: String,
        assistant_dones: Vec<AssistantMessage>,
        tool_calls: Vec<(AssistantMessage, ToolCallInfo)>,
    },
    /// The request did not yield a response.
    Failed {
        error: String,
        emitted_content: bool,
    },
}

/// An agent ready to run.
pub struct Agent {
    pub runner: Option<AgentRunner>,
    pub session: Session,
}

impl Agent {
    /// Creates a new `AgentBuilder`.
    pub fn builder() -> super::AgentBuilder {
        super::AgentBuilder::new()
    }

    /// Runs the agent with streaming and returns a channel receiver.
    ///
    /// The agent is consumed (may only be run once).
    pub async fn run(&mut self) -> Result<broadcast::Receiver<AgentEvent>> {
        let runner = self.runner.take().ok_or_else(|| {
            crate::error::CustomError::Agent("Agent already consumed".to_string())
        })?;
        runner.validate_selection()?;

        let (tx, rx) = broadcast::channel(256);

        let todos: tools::SharedTodoList = tools::new_shared_todo_list();
        if let Ok(mut guard) = todos.lock() {
            *guard = self.session.todo_list.clone();
        }
        tools::init_todo_list(todos.clone());

        let log = format!(
            "Starting agent session: {} ({} / {})",
            self.session.id, runner.provider_name, runner.provider_model
        );
        self.record_run_metadata(&runner);
        let session = self.session.clone();
        if let Err(e) = session.save() {
            tracing::warn!("Failed to persist session metadata: {e}");
        }

        tokio::spawn(async move {
            info!("{log}");
            runner.execute(tx, session, None).await;
        });

        Ok(rx)
    }

    /// Runs the agent with streaming and returns both the event receiver and a
    /// steer channel.
    ///
    /// The run pauses before each proposed tool call; the caller decides when
    /// to send [`SteerCommand::Continue`] (execute the tool and proceed) or
    /// [`SteerCommand::Inject`] (let the tool run, then inject guidance before
    /// the next step). Dropping the sender resumes execution.
    ///
    /// The agent is consumed (may only be run once).
    pub async fn run_steerable(
        &mut self,
    ) -> Result<(broadcast::Receiver<AgentEvent>, mpsc::Sender<SteerCommand>)> {
        let runner = self.runner.take().ok_or_else(|| {
            crate::error::CustomError::Agent("Agent already consumed".to_string())
        })?;
        runner.validate_selection()?;

        let (tx, rx) = broadcast::channel(256);
        let (steer_tx, steer_rx) = mpsc::channel(64);

        let todos: tools::SharedTodoList = tools::new_shared_todo_list();
        if let Ok(mut guard) = todos.lock() {
            *guard = self.session.todo_list.clone();
        }
        tools::init_todo_list(todos.clone());

        let log = format!(
            "Starting steerable agent session: {} ({} / {})",
            self.session.id, runner.provider_name, runner.provider_model
        );
        self.record_run_metadata(&runner);
        let session = self.session.clone();
        if let Err(e) = session.save() {
            tracing::warn!("Failed to persist session metadata: {e}");
        }

        tokio::spawn(async move {
            info!("{log}");
            runner.execute(tx, session, Some(steer_rx)).await;
        });

        Ok((rx, steer_tx))
    }

    /// Generates a concise session name/title using the model and session context.
    pub async fn generate_session_name(&mut self) -> Result<String> {
        let runner = self.runner.as_ref().ok_or_else(|| {
            crate::error::CustomError::Agent("Agent already consumed".to_string())
        })?;

        let conversation = if self.session.messages.is_empty() {
            return Err(crate::error::CustomError::Agent(
                "Cannot generate session name: no messages in session".to_string(),
            ));
        } else {
            PromptEngine::format_messages(&self.session.messages)
        };
        let name_prompt = PromptEngine::render_session_name(&conversation);

        let model = runner.build_model().await.map_err(|e| {
            crate::error::CustomError::Agent(format!("Session naming model build failed: {}", e))
        })?;
        dispatch_model!(model, self, name_with_model, &name_prompt)
    }

    /// Compacts the current session's message history into a summary.
    ///
    /// 1. Renders COMPACT.md with the full conversation.
    /// 2. Calls the model (no tools, no stop condition) to get a summary.
    /// 3. Renders SUMMARY.md with that summary.
    /// 4. Archives the old messages in `session.archived_messages` (kept for
    ///    display, excluded from the agent context).
    /// 5. Clears session.messages.
    /// 6. Injects the rendered summary as a system message.
    /// 7. Persists the session.
    ///
    /// Returns the summary text on success.
    pub async fn compact(&mut self) -> Result<String> {
        let runner = self.runner.as_ref().ok_or_else(|| {
            crate::error::CustomError::Agent("Agent already consumed".to_string())
        })?;

        if self.session.messages.is_empty() {
            return Ok(String::new());
        }

        let conversation = PromptEngine::format_messages(&self.session.messages);
        let compact_prompt = PromptEngine::render_compact(&conversation);

        let model = runner.build_model().await.map_err(|e| {
            crate::error::CustomError::Agent(format!("Compaction model build failed: {}", e))
        })?;
        dispatch_model!(model, self, compact_with_model, &compact_prompt)
    }

    async fn name_with_model<M>(&mut self, model: M, name_prompt: &str) -> Result<String>
    where
        M: LanguageModel + TextInputSupport,
    {
        let mut request = LanguageModelRequest::<M>::builder()
            .model(model)
            .prompt(name_prompt)
            .build();

        let response = request.generate_text().await.map_err(|e| {
            crate::error::CustomError::Agent(format!("Session naming failed: {}", e))
        })?;

        let title = response.text().unwrap_or_default().trim().to_string();
        Ok(title)
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
            .map_err(|e| crate::error::CustomError::Agent(format!("Compaction failed: {}", e)))?;

        let summary = response.text().unwrap_or_default();
        let rendered_summary = PromptEngine::render_summary(&summary);

        self.session
            .archived_messages
            .push(self.session.messages.clone());
        self.session.messages.clear();
        self.session
            .add_message("system".to_string(), rendered_summary);
        self.session.save()?;

        Ok(summary)
    }

    /// Records which provider/model produced this run in the session metadata.
    fn record_run_metadata(&mut self, runner: &AgentRunner) {
        if !self.session.metadata.is_object() {
            self.session.metadata = serde_json::json!({});
        }
        self.session.metadata["provider"] = serde_json::json!(runner.provider_name);
        self.session.metadata["model"] = serde_json::json!(runner.provider_model);
    }
}
