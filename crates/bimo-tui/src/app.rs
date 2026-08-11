use super::*;
use crate::state::agent_state::AgentStatus;
use ratatui::Terminal;
use ratatui::prelude::Widget;
use std::io;
use tokio::sync::mpsc;

pub struct App {
    terminal: Terminal<ratatui::backend::CrosstermBackend<io::Stdout>>,
    app_state: AppState,
    event_handler: crate::event::EventHandler,
}

impl App {
    pub fn new(
        app_state: AppState,
        terminal: Terminal<ratatui::backend::CrosstermBackend<io::Stdout>>,
    ) -> Self {
        Self {
            terminal,
            app_state,
            event_handler: crate::event::EventHandler::new(),
        }
    }

    pub fn terminal(&mut self) -> &mut Terminal<ratatui::backend::CrosstermBackend<io::Stdout>> {
        &mut self.terminal
    }

    pub async fn run(
        &mut self,
        mut event_rx: mpsc::Receiver<crate::event::AppEvent>,
    ) -> Result<()> {
        while let Some(event) = event_rx.recv().await {
            // Handle app event
            self.event_handler
                .handle_app_event(event.clone(), &mut self.app_state);

            // Handle specific events
            match event {
                crate::event::AppEvent::Quit => {
                    self.app_state.should_quit = true;
                    break;
                }
                crate::event::AppEvent::Tick => {
                    self.app_state.update_toasts();
                }
                crate::event::AppEvent::AgentEvent(agent_event) => {
                    self.handle_agent_event(agent_event).await;
                }
                crate::event::AppEvent::SendMessage(msg) => {
                    self.send_message(msg).await;
                }
                crate::event::AppEvent::NewSessionCreated(name) => {
                    self.create_session(name).await;
                }
                crate::event::AppEvent::LoadSession(id) => {
                    self.load_session(id).await;
                }
                crate::event::AppEvent::ForkSession(id) => {
                    self.fork_session(id).await;
                }
                crate::event::AppEvent::DeleteSession(id) => {
                    self.delete_session(id).await;
                }
                crate::event::AppEvent::Undo => {
                    self.undo().await;
                }
                crate::event::AppEvent::Redo => {
                    self.redo().await;
                }
                crate::event::AppEvent::SteerContinue => {
                    self.steer_continue();
                }
                crate::event::AppEvent::SteerInject(guidance) => {
                    self.steer_inject(guidance);
                }
                crate::event::AppEvent::AddProvider => {
                    self.show_add_provider_dialog();
                }
                crate::event::AppEvent::RemoveProvider(id) => {
                    self.remove_provider(id).await;
                }
                crate::event::AppEvent::SetDefaultProvider(id) => {
                    self.set_default_provider(id).await;
                }
                crate::event::AppEvent::DiscoverModels(id) => {
                    self.discover_models(id).await;
                }
                crate::event::AppEvent::RefreshCatalogue => {
                    self.refresh_catalogue().await;
                }
                crate::event::AppEvent::SettingChanged(key, value) => {
                    self.change_setting(key, value).await;
                }
                crate::event::AppEvent::ThemeChanged(variant) => {
                    self.app_state.set_theme(variant);
                }
                crate::event::AppEvent::ReducedMotionToggled => {
                    self.app_state
                        .set_reduced_motion(!self.app_state.reduced_motion);
                }
                _ => {}
            }

            // Render
            let layout = self.app_state.layout.clone();
            let toasts = self.app_state.toasts.clone();
            let mode = self.app_state.mode.clone();
            let confirm_dialog = self.app_state.confirm_dialog.clone();
            let text_input_dialog = self.app_state.text_input_dialog.clone();
            let progress_dialog = self.app_state.progress_dialog.clone();
            let styles = self.app_state.layout.get_styles();

            self.terminal.draw(|f| {
                layout.render(f.area(), f.buffer_mut());

                // Render toasts
                let area = f.area();
                let mut y = area.y + 2;
                for toast in &toasts {
                    if y + 1 >= area.y + area.height {
                        break;
                    }
                    let toast_area = ratatui::layout::Rect::new(area.x + 2, y, area.width - 4, 1);
                    crate::widgets::Toast::new(toast.message.clone(), toast.style, toast.duration)
                        .render(toast_area, f.buffer_mut());
                    y += 2;
                }

                // Render modals
                use crate::layouts::dialogs::{ConfirmDialog, ProgressDialog, TextInputDialog};
                use crate::state::app_state::AppMode;

                match mode {
                    AppMode::ConfirmDialog => {
                        if let Some(dialog) = confirm_dialog {
                            let modal = ConfirmDialog::new(&dialog.title, &dialog.message)
                                .confirm_text("Confirm")
                                .cancel_text("Cancel")
                                .styles(styles);
                            modal.render(f.area(), f.buffer_mut());
                        }
                    }
                    AppMode::TextInputDialog => {
                        if let Some(dialog) = text_input_dialog {
                            let modal = TextInputDialog::new(&dialog.title, &dialog.prompt)
                                .value(&dialog.value)
                                .placeholder(&dialog.placeholder)
                                .masked(dialog.masked)
                                .styles(styles);
                            modal.render(f.area(), f.buffer_mut());
                        }
                    }
                    AppMode::ProgressDialog => {
                        if let Some(dialog) = progress_dialog {
                            let modal = ProgressDialog::new(&dialog.title, &dialog.message)
                                .progress(dialog.progress)
                                .styles(styles);
                            modal.render(f.area(), f.buffer_mut());
                        }
                    }
                    AppMode::Help => {
                        // Help is rendered by chat view
                    }
                    _ => {}
                }
            })?;

            if self.app_state.should_quit {
                break;
            }
        }
        Ok(())
    }

    async fn handle_agent_event(&mut self, event: bimo_core::AgentEvent) {
        match event {
            bimo_core::AgentEvent::TextDelta(delta) => {
                self.app_state.layout.chat_mut().append_streaming(&delta);
            }
            bimo_core::AgentEvent::ReasoningDelta(delta) => {
                self.app_state.layout.chat_mut().append_streaming(&delta);
            }
            bimo_core::AgentEvent::ToolCallStart { tool_name, args } => {
                let tool = crate::components::chat::ToolCall {
                    name: tool_name,
                    args: serde_json::to_string(&args).unwrap_or_default(),
                    result: None,
                    is_expanded: false,
                    is_error: false,
                };
                if let Some(msg_id) = &self.app_state.agent_state.current_message_id {
                    self.app_state.layout.chat_mut().add_tool_call(msg_id, tool);
                }
            }
            bimo_core::AgentEvent::ToolCallEnd { tool_name, result } => {
                if let Some(msg_id) = &self.app_state.agent_state.current_message_id {
                    let is_error = result.is_err();
                    let result_str = result
                        .map(|s| s.to_string())
                        .unwrap_or_else(|e| e.to_string());
                    self.app_state
                        .layout
                        .chat_mut()
                        .update_tool_result(msg_id, &tool_name, result_str, is_error);
                }
            }
            bimo_core::AgentEvent::Steering(guidance) => {
                self.app_state.layout.chat_mut().add_message(
                    crate::components::chat::ChatMessage {
                        id: uuid::Uuid::new_v4().to_string(),
                        role: "user".to_string(),
                        content: guidance,
                        timestamp: chrono::Utc::now().format("%H:%M:%S").to_string(),
                        is_streaming: false,
                        tool_calls: Vec::new(),
                    },
                );
            }
            bimo_core::AgentEvent::Retrying { attempt, error } => {
                self.app_state.add_toast(
                    format!(
                        "Retrying ({}/{}): {}",
                        attempt, self.app_state.agent_state.max_steps, error
                    ),
                    self.app_state.layout.get_styles().warning,
                    std::time::Duration::from_secs(5),
                );
            }
            bimo_core::AgentEvent::Error(e) => {
                self.app_state.agent_state.status = AgentStatus::Error(e.clone());
                self.app_state.add_toast(
                    format!("Error: {}", e),
                    self.app_state.layout.get_styles().error,
                    std::time::Duration::from_secs(10),
                );
            }
            bimo_core::AgentEvent::Done => {
                self.app_state.agent_state.status = AgentStatus::Done;
                self.app_state.layout.chat_mut().end_streaming();
                self.app_state.steer_tx = None;
            }
        }
    }

    async fn send_message(&mut self, msg: String) {
        use bimo_core::Agent;
        use bimo_core::config::SettingsConfig;

        let session = if let Some(current) = &self.app_state.session_state.current_session {
            current.clone()
        } else {
            // Create new session
            if let Some(manager) = &self.app_state.session_manager {
                let manager = manager.read().await;
                manager.create().await.unwrap()
            } else {
                bimo_core::Session::new()
            }
        };

        // Get provider/model from config
        let settings = SettingsConfig::load().unwrap_or_default();
        let providers = bimo_core::config::ProvidersConfig::load().unwrap_or(
            bimo_core::config::ProvidersConfig {
                providers: Vec::new(),
                default: None,
            },
        );
        let provider = providers.default_provider().cloned().unwrap_or_else(|| {
            bimo_core::Provider::local(
                "ollama",
                "Ollama",
                "http://localhost:11434/v1",
                bimo_core::ApiFormat::OpenAICompatible,
            )
        });
        let model = "llama3".to_string();

        let mut agent = Agent::builder()
            .with_settings(settings)
            .with_provider(provider)
            .with_model(model)
            .with_session(session)
            .with_user_prompt(msg.clone())
            .build()
            .unwrap();

        // Add user message to chat
        self.app_state
            .layout
            .chat_mut()
            .add_message(crate::components::chat::ChatMessage {
                id: uuid::Uuid::new_v4().to_string(),
                role: "user".to_string(),
                content: msg,
                timestamp: chrono::Utc::now().format("%H:%M:%S").to_string(),
                is_streaming: false,
                tool_calls: Vec::new(),
            });

        // Start agent
        let event_rx = agent.run().await.unwrap();
        self.app_state.agent_state.start_run(
            self.app_state
                .session_state
                .current_session
                .as_ref()
                .unwrap()
                .id
                .clone(),
            uuid::Uuid::new_v4().to_string(),
            25,
            false,
        );

        // Forward events
        let event_tx = self.app_state.agent_event_tx.clone().unwrap();
        tokio::spawn(async move {
            let mut rx = event_rx;
            while let Ok(event) = rx.recv().await {
                if event_tx.send(event).is_err() {
                    break;
                }
            }
        });
    }

    async fn create_session(&mut self, name: String) {
        if let Some(manager) = &self.app_state.session_manager {
            let manager = manager.read().await;
            let mut session = manager.create().await.unwrap();
            if !name.is_empty() {
                session.metadata["name"] = serde_json::json!(name);
                session.save().unwrap();
            }
            self.app_state.layout.sidebar_mut().set_sessions(vec![
                crate::components::sidebar::SessionItem {
                    id: session.id.clone(),
                    name: session
                        .metadata
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unnamed")
                        .to_string(),
                    model: "Unknown".to_string(),
                    updated: chrono::Utc::now().format("%Y-%m-%d %H:%M").to_string(),
                    message_count: 0,
                    is_active: true,
                },
            ]);
        }
    }

    async fn load_session(&mut self, _id: String) {
        // Implementation would load session
    }

    async fn fork_session(&mut self, _id: String) {
        // Implementation would fork session
    }

    async fn delete_session(&mut self, _id: String) {
        // Implementation would delete session
    }

    async fn undo(&mut self) {
        // Implementation would undo
    }

    async fn redo(&mut self) {
        // Implementation would redo
    }

    fn steer_continue(&mut self) {
        if let Err(e) = self.app_state.send_steer(bimo_core::SteerCommand::Continue) {
            self.app_state.add_toast(
                format!("Failed to continue: {}", e),
                self.app_state.layout.get_styles().error,
                std::time::Duration::from_secs(3),
            );
        }
    }

    fn steer_inject(&mut self, guidance: String) {
        if let Err(e) = self
            .app_state
            .send_steer(bimo_core::SteerCommand::Inject(guidance))
        {
            self.app_state.add_toast(
                format!("Failed to inject: {}", e),
                self.app_state.layout.get_styles().error,
                std::time::Duration::from_secs(3),
            );
        }
    }

    fn show_add_provider_dialog(&mut self) {
        self.app_state.show_text_input(
            "Add Provider".to_string(),
            "Provider ID:".to_string(),
            crate::state::app_state::TextInputAction::AddProvider,
        );
    }

    async fn remove_provider(&mut self, id: String) {
        self.app_state.config_state.remove_provider(&id);
        self.app_state.config_state.save_providers().unwrap();
    }

    async fn set_default_provider(&mut self, id: String) {
        self.app_state.config_state.set_default_provider(&id);
        self.app_state.config_state.save_providers().unwrap();
    }

    async fn discover_models(&mut self, id: String) {
        if let Ok(models) = self.app_state.config_state.discover_models(&id).await {
            if let Some(idx) = self
                .app_state
                .config_state
                .providers
                .providers
                .iter()
                .position(|p| p.id == id)
            {
                self.app_state.config_state.providers.providers[idx].models = models;
                self.app_state.config_state.save_providers().unwrap();
            }
        }
    }

    async fn refresh_catalogue(&mut self) {
        let _ = self.app_state.config_state.refresh_catalogue().await;
    }

    async fn change_setting(&mut self, _key: String, _value: String) {
        // Implementation would change setting
    }
}
