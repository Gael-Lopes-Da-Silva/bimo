use std::sync::Arc;
use tokio::sync::{RwLock, broadcast, mpsc};

use crate::layouts::main::MainLayout;
use bimo_core::{AgentEvent, Session, SteerCommand};

#[derive(Debug, Clone, PartialEq)]
pub enum AppMode {
    Normal,
    Settings,
    Help,
    ConfirmDialog,
    TextInputDialog,
    ProgressDialog,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FocusedPanel {
    Chat,
    Sidebar,
    StatusBar,
    Modal,
}

pub struct AppState {
    pub mode: AppMode,
    pub focused_panel: FocusedPanel,
    pub should_quit: bool,
    pub layout: MainLayout,
    pub theme_variant: crate::theme::ThemeVariant,
    pub reduced_motion: bool,

    // State modules
    pub session_state: crate::state::session_state::SessionState,
    pub agent_state: crate::state::agent_state::AgentState,
    pub config_state: crate::state::config_state::ConfigState,
    pub ui_state: crate::state::ui_state::UIState,

    // Modal state
    pub confirm_dialog: Option<ConfirmDialogState>,
    pub text_input_dialog: Option<TextInputDialogState>,
    pub progress_dialog: Option<ProgressDialogState>,

    // Toasts
    pub toasts: Vec<ToastState>,

    // Agent event handling
    pub agent_event_tx: Option<broadcast::Sender<AgentEvent>>,
    pub steer_tx: Option<mpsc::Sender<SteerCommand>>,

    // Session management
    pub session_manager: Option<Arc<RwLock<bimo_core::session::SessionManager>>>,

    // Pending actions
    pub pending_new_session: Option<String>,
    pub pending_fork_session: Option<String>,
    pub pending_delete_session: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ConfirmDialogState {
    pub title: String,
    pub message: String,
    pub on_confirm: ConfirmAction,
    pub on_cancel: CancelAction,
    pub selected: usize,
}

#[derive(Debug, Clone)]
pub enum ConfirmAction {
    DeleteSession(String),
    ForkSession(String),
    ClearHistory,
    ResetSettings,
    Custom(String),
}

#[derive(Debug, Clone)]
pub enum CancelAction {
    None,
    Custom(String),
}

#[derive(Debug, Clone)]
pub struct TextInputDialogState {
    pub title: String,
    pub prompt: String,
    pub value: String,
    pub cursor_pos: usize,
    pub placeholder: String,
    pub masked: bool,
    pub on_submit: TextInputAction,
    pub on_cancel: CancelAction,
}

#[derive(Debug, Clone)]
pub enum TextInputAction {
    NewSession,
    RenameSession(String),
    AddProvider,
    SetApiKey(String),
    InjectGuidance,
    Custom(String),
}

#[derive(Debug, Clone)]
pub struct ProgressDialogState {
    pub title: String,
    pub message: String,
    pub progress: f32,
}

#[derive(Debug, Clone)]
pub struct ToastState {
    pub id: u64,
    pub message: String,
    pub style: ratatui::style::Style,
    pub duration: std::time::Duration,
    pub start_time: std::time::Instant,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            mode: AppMode::Normal,
            focused_panel: FocusedPanel::Chat,
            should_quit: false,
            layout: MainLayout::new(),
            theme_variant: crate::theme::ThemeVariant::Mocha,
            reduced_motion: false,
            session_state: crate::state::session_state::SessionState::new(),
            agent_state: crate::state::agent_state::AgentState::new(),
            config_state: crate::state::config_state::ConfigState::new(),
            ui_state: crate::state::ui_state::UIState::new(),
            confirm_dialog: None,
            text_input_dialog: None,
            progress_dialog: None,
            toasts: Vec::new(),
            agent_event_tx: None,
            steer_tx: None,
            session_manager: None,
            pending_new_session: None,
            pending_fork_session: None,
            pending_delete_session: None,
        }
    }

    pub fn set_theme(&mut self, variant: crate::theme::ThemeVariant) {
        self.theme_variant = variant;
        let styles = crate::theme::Styles::from_theme(&crate::theme::Theme::from_variant(variant));
        self.layout = self.layout.clone().styles(styles);
    }

    pub fn set_reduced_motion(&mut self, reduced: bool) {
        self.reduced_motion = reduced;
    }

    pub fn show_confirm(&mut self, title: String, message: String, on_confirm: ConfirmAction) {
        self.confirm_dialog = Some(ConfirmDialogState {
            title,
            message,
            on_confirm,
            on_cancel: CancelAction::None,
            selected: 1,
        });
        self.mode = AppMode::ConfirmDialog;
        self.focused_panel = FocusedPanel::Modal;
    }

    pub fn show_text_input(&mut self, title: String, prompt: String, on_submit: TextInputAction) {
        self.text_input_dialog = Some(TextInputDialogState {
            title,
            prompt,
            value: String::new(),
            cursor_pos: 0,
            placeholder: String::new(),
            masked: false,
            on_submit,
            on_cancel: CancelAction::None,
        });
        self.mode = AppMode::TextInputDialog;
        self.focused_panel = FocusedPanel::Modal;
    }

    pub fn show_progress(&mut self, title: String, message: String) {
        self.progress_dialog = Some(ProgressDialogState {
            title,
            message,
            progress: 0.0,
        });
        self.mode = AppMode::ProgressDialog;
        self.focused_panel = FocusedPanel::Modal;
    }

    pub fn update_progress(&mut self, progress: f32, message: Option<String>) {
        if let Some(dialog) = &mut self.progress_dialog {
            dialog.progress = progress.clamp(0.0, 1.0);
            if let Some(msg) = message {
                dialog.message = msg;
            }
        }
    }

    pub fn hide_modal(&mut self) {
        self.confirm_dialog = None;
        self.text_input_dialog = None;
        self.progress_dialog = None;
        self.mode = AppMode::Normal;
        self.focused_panel = FocusedPanel::Chat;
    }

    pub fn add_toast(
        &mut self,
        message: String,
        style: ratatui::style::Style,
        duration: std::time::Duration,
    ) {
        let id = rand::random::<u64>();
        self.toasts.push(ToastState {
            id,
            message,
            style,
            duration,
            start_time: std::time::Instant::now(),
        });
    }

    pub fn update_toasts(&mut self) {
        self.toasts.retain(|t| t.start_time.elapsed() < t.duration);
    }

    pub fn set_agent_channels(
        &mut self,
        event_tx: broadcast::Sender<AgentEvent>,
        steer_tx: mpsc::Sender<SteerCommand>,
    ) {
        self.agent_event_tx = Some(event_tx);
        self.steer_tx = Some(steer_tx);
    }

    pub fn send_steer(
        &self,
        cmd: SteerCommand,
    ) -> Result<(), mpsc::error::TrySendError<SteerCommand>> {
        if let Some(tx) = &self.steer_tx {
            tx.try_send(cmd)
        } else {
            Err(mpsc::error::TrySendError::Closed(cmd))
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for AppState {
    fn clone(&self) -> Self {
        Self {
            mode: self.mode.clone(),
            focused_panel: self.focused_panel.clone(),
            should_quit: self.should_quit,
            layout: self.layout.clone(),
            theme_variant: self.theme_variant,
            reduced_motion: self.reduced_motion,
            session_state: self.session_state.clone(),
            agent_state: self.agent_state.clone(),
            config_state: self.config_state.clone(),
            ui_state: self.ui_state.clone(),
            confirm_dialog: self.confirm_dialog.clone(),
            text_input_dialog: self.text_input_dialog.clone(),
            progress_dialog: self.progress_dialog.clone(),
            toasts: self.toasts.clone(),
            agent_event_tx: self.agent_event_tx.clone(),
            steer_tx: self.steer_tx.clone(),
            session_manager: self.session_manager.clone(),
            pending_new_session: self.pending_new_session.clone(),
            pending_fork_session: self.pending_fork_session.clone(),
            pending_delete_session: self.pending_delete_session.clone(),
        }
    }
}
