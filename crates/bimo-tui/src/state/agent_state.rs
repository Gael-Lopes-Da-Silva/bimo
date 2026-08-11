use bimo_core::{AgentEvent, SteerCommand};

#[derive(Debug, Clone, PartialEq)]
pub enum AgentStatus {
    Idle,
    Starting,
    Running,
    Streaming,
    Steering,
    Done,
    Error(String),
}

#[derive(Debug, Clone)]
pub struct AgentState {
    pub status: AgentStatus,
    pub current_session_id: Option<String>,
    pub current_message_id: Option<String>,
    pub step: usize,
    pub max_steps: usize,
    pub tokens_used: (u32, u32), // input, output
    pub cost_estimate: Option<f64>,
    pub last_error: Option<String>,
    pub is_steerable: bool,
    pub pending_tool_calls: Vec<PendingToolCall>,
    pub steering_guidance: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PendingToolCall {
    pub name: String,
    pub args: String,
    pub id: String,
}

impl AgentState {
    pub fn new() -> Self {
        Self {
            status: AgentStatus::Idle,
            current_session_id: None,
            current_message_id: None,
            step: 0,
            max_steps: 25,
            tokens_used: (0, 0),
            cost_estimate: None,
            last_error: None,
            is_steerable: false,
            pending_tool_calls: Vec::new(),
            steering_guidance: None,
        }
    }

    pub fn handle_event(&mut self, event: &AgentEvent) {
        match event {
            AgentEvent::TextDelta(_) | AgentEvent::ReasoningDelta(_) => {
                if matches!(self.status, AgentStatus::Running | AgentStatus::Starting) {
                    self.status = AgentStatus::Streaming;
                }
            }
            AgentEvent::ToolCallStart { tool_name, args } => {
                self.pending_tool_calls.push(PendingToolCall {
                    name: tool_name.clone(),
                    args: serde_json::to_string(args).unwrap_or_default(),
                    id: uuid::Uuid::new_v4().to_string(),
                });
            }
            AgentEvent::ToolCallEnd { tool_name, result } => {
                self.pending_tool_calls.retain(|t| t.name != *tool_name);
                self.step += 1;
                if result.is_err() {
                    self.last_error = Some(result.as_ref().err().unwrap().clone());
                }
            }
            AgentEvent::Steering(_) => {
                self.status = AgentStatus::Running;
            }
            AgentEvent::Retrying { attempt, error } => {
                self.last_error = Some(format!("Retry {}/{}: {}", attempt, self.max_steps, error));
            }
            AgentEvent::Error(e) => {
                self.status = AgentStatus::Error(e.clone());
                self.last_error = Some(e.clone());
            }
            AgentEvent::Done => {
                self.status = AgentStatus::Done;
            }
            _ => {}
        }
    }

    pub fn start_run(
        &mut self,
        session_id: String,
        message_id: String,
        max_steps: usize,
        steerable: bool,
    ) {
        self.status = AgentStatus::Starting;
        self.current_session_id = Some(session_id);
        self.current_message_id = Some(message_id);
        self.step = 0;
        self.max_steps = max_steps;
        self.tokens_used = (0, 0);
        self.cost_estimate = None;
        self.last_error = None;
        self.is_steerable = steerable;
        self.pending_tool_calls.clear();
        self.steering_guidance = None;
    }

    pub fn set_steering(&mut self, guidance: String) {
        self.steering_guidance = Some(guidance);
        self.status = AgentStatus::Steering;
    }

    pub fn clear_steering(&mut self) {
        self.steering_guidance = None;
        if self.status == AgentStatus::Steering {
            self.status = AgentStatus::Running;
        }
    }

    pub fn can_steer(&self) -> bool {
        self.is_steerable && matches!(self.status, AgentStatus::Steering | AgentStatus::Streaming)
    }

    pub fn progress(&self) -> f32 {
        if self.max_steps == 0 {
            0.0
        } else {
            (self.step as f32 / self.max_steps as f32).min(1.0)
        }
    }
}

impl Default for AgentState {
    fn default() -> Self {
        Self::new()
    }
}
