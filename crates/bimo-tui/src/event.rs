use crossterm::event::{
    Event as CrosstermEvent, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent,
    MouseEventKind,
};

use crate::components::chat::ChatAction;
use crate::components::sidebar::SidebarAction;
use crate::state::app_state::{AppMode, AppState, FocusedPanel};
use crate::state::ui_state::DragElement;

#[derive(Debug, Clone)]
pub enum AppEvent {
    None,
    Quit,
    Tick,
    Resize(u16, u16),

    // Chat events
    SendMessage(String),
    ChatFocusGained,
    ChatFocusLost,
    ChatScrolled,
    MessageClicked(usize),
    ChatContextMenu,

    // Sidebar events
    SidebarTabChanged,
    SidebarItemSelected,
    SidebarContextMenu(usize),
    NewSessionRequested,
    NewSessionCreated(String),
    NewSessionCancelled,
    SettingsRequested,
    SettingsTabChanged,
    SettingsClosed,

    // Agent events
    AgentStarted,
    AgentEvent(bimo_core::AgentEvent),
    AgentDone,
    AgentError(String),
    SteerContinue,
    SteerInject(String),
    SteerRequested,

    // Session events
    LoadSession(String),
    ForkSession(String),
    DeleteSession(String),
    CompactSession,
    Undo,
    Redo,

    // Provider events
    AddProvider,
    RemoveProvider(String),
    SetDefaultProvider(String),
    DiscoverModels(String),
    RefreshCatalogue,
    TestConnection(String),

    // Settings events
    SettingChanged(String, String),
    ThemeChanged(crate::theme::ThemeVariant),
    ReducedMotionToggled,

    // Dialog events
    ConfirmDialogConfirm,
    ConfirmDialogCancel,
    TextInputSubmit(String),
    TextInputCancel,
    ProgressUpdate(f32),

    // UI events
    MouseMove(u16, u16),
    MouseClick(MouseButton, u16, u16),
    MouseDrag(u16, u16),
    MouseRelease,
    MouseScroll(i32),
    KeyPress(KeyEvent),
}

impl PartialEq for AppEvent {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (AppEvent::None, AppEvent::None) => true,
            (AppEvent::Quit, AppEvent::Quit) => true,
            (AppEvent::Tick, AppEvent::Tick) => true,
            (AppEvent::Resize(a1, a2), AppEvent::Resize(b1, b2)) => a1 == b1 && a2 == b2,
            (AppEvent::SendMessage(a), AppEvent::SendMessage(b)) => a == b,
            (AppEvent::ChatFocusGained, AppEvent::ChatFocusGained) => true,
            (AppEvent::ChatFocusLost, AppEvent::ChatFocusLost) => true,
            (AppEvent::ChatScrolled, AppEvent::ChatScrolled) => true,
            (AppEvent::MessageClicked(a), AppEvent::MessageClicked(b)) => a == b,
            (AppEvent::ChatContextMenu, AppEvent::ChatContextMenu) => true,
            (AppEvent::SidebarTabChanged, AppEvent::SidebarTabChanged) => true,
            (AppEvent::SidebarItemSelected, AppEvent::SidebarItemSelected) => true,
            (AppEvent::SidebarContextMenu(a), AppEvent::SidebarContextMenu(b)) => a == b,
            (AppEvent::NewSessionRequested, AppEvent::NewSessionRequested) => true,
            (AppEvent::NewSessionCreated(a), AppEvent::NewSessionCreated(b)) => a == b,
            (AppEvent::NewSessionCancelled, AppEvent::NewSessionCancelled) => true,
            (AppEvent::SettingsRequested, AppEvent::SettingsRequested) => true,
            (AppEvent::SettingsTabChanged, AppEvent::SettingsTabChanged) => true,
            (AppEvent::SettingsClosed, AppEvent::SettingsClosed) => true,
            (AppEvent::AgentStarted, AppEvent::AgentStarted) => true,
            // AgentEvent doesn't implement PartialEq, so we compare by variant only
            (AppEvent::AgentEvent(_), AppEvent::AgentEvent(_)) => true,
            (AppEvent::AgentDone, AppEvent::AgentDone) => true,
            (AppEvent::AgentError(a), AppEvent::AgentError(b)) => a == b,
            (AppEvent::SteerContinue, AppEvent::SteerContinue) => true,
            (AppEvent::SteerInject(a), AppEvent::SteerInject(b)) => a == b,
            (AppEvent::SteerRequested, AppEvent::SteerRequested) => true,
            (AppEvent::LoadSession(a), AppEvent::LoadSession(b)) => a == b,
            (AppEvent::ForkSession(a), AppEvent::ForkSession(b)) => a == b,
            (AppEvent::DeleteSession(a), AppEvent::DeleteSession(b)) => a == b,
            (AppEvent::CompactSession, AppEvent::CompactSession) => true,
            (AppEvent::Undo, AppEvent::Undo) => true,
            (AppEvent::Redo, AppEvent::Redo) => true,
            (AppEvent::AddProvider, AppEvent::AddProvider) => true,
            (AppEvent::RemoveProvider(a), AppEvent::RemoveProvider(b)) => a == b,
            (AppEvent::SetDefaultProvider(a), AppEvent::SetDefaultProvider(b)) => a == b,
            (AppEvent::DiscoverModels(a), AppEvent::DiscoverModels(b)) => a == b,
            (AppEvent::RefreshCatalogue, AppEvent::RefreshCatalogue) => true,
            (AppEvent::TestConnection(a), AppEvent::TestConnection(b)) => a == b,
            (AppEvent::SettingChanged(k1, v1), AppEvent::SettingChanged(k2, v2)) => {
                k1 == k2 && v1 == v2
            }
            (AppEvent::ThemeChanged(a), AppEvent::ThemeChanged(b)) => a == b,
            (AppEvent::ReducedMotionToggled, AppEvent::ReducedMotionToggled) => true,
            (AppEvent::ConfirmDialogConfirm, AppEvent::ConfirmDialogConfirm) => true,
            (AppEvent::ConfirmDialogCancel, AppEvent::ConfirmDialogCancel) => true,
            (AppEvent::TextInputSubmit(a), AppEvent::TextInputSubmit(b)) => a == b,
            (AppEvent::TextInputCancel, AppEvent::TextInputCancel) => true,
            (AppEvent::ProgressUpdate(a), AppEvent::ProgressUpdate(b)) => a == b,
            (AppEvent::MouseMove(x1, y1), AppEvent::MouseMove(x2, y2)) => x1 == x2 && y1 == y2,
            (AppEvent::MouseClick(b1, x1, y1), AppEvent::MouseClick(b2, x2, y2)) => {
                b1 == b2 && x1 == x2 && y1 == y2
            }
            (AppEvent::MouseDrag(x1, y1), AppEvent::MouseDrag(x2, y2)) => x1 == x2 && y1 == y2,
            (AppEvent::MouseRelease, AppEvent::MouseRelease) => true,
            (AppEvent::MouseScroll(a), AppEvent::MouseScroll(b)) => a == b,
            (AppEvent::KeyPress(a), AppEvent::KeyPress(b)) => a == b,
            _ => false,
        }
    }
}

pub struct EventHandler {
    last_mouse_pos: (u16, u16),
    mouse_captured: bool,
}

impl EventHandler {
    pub fn new() -> Self {
        Self {
            last_mouse_pos: (0, 0),
            mouse_captured: false,
        }
    }

    pub fn handle_event(&mut self, event: CrosstermEvent, app_state: &mut AppState) -> AppEvent {
        match event {
            CrosstermEvent::Key(key) => self.handle_key(key, app_state),
            CrosstermEvent::Mouse(mouse) => self.handle_mouse(mouse, app_state),
            CrosstermEvent::Resize(width, height) => AppEvent::Resize(width, height),
            CrosstermEvent::FocusGained => AppEvent::None,
            CrosstermEvent::FocusLost => AppEvent::None,
            CrosstermEvent::Paste(_) => AppEvent::None,
        }
    }

    fn handle_key(&mut self, key: KeyEvent, app_state: &mut AppState) -> AppEvent {
        // Global shortcuts
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('c') => return AppEvent::Quit,
                KeyCode::Char('q') => return AppEvent::Quit,
                KeyCode::Char('n') => return AppEvent::NewSessionRequested,
                KeyCode::Char('s') => return AppEvent::SettingsRequested,
                KeyCode::Char('b') => {
                    app_state.layout.toggle_sidebar();
                    return AppEvent::None;
                }
                KeyCode::Char('z') => {
                    if key.modifiers.contains(KeyModifiers::SHIFT) {
                        return AppEvent::Redo;
                    }
                    return AppEvent::Undo;
                }
                KeyCode::Char('t') => {
                    // Cycle themes
                    let themes = crate::theme::ThemeVariant::ALL;
                    let current_idx = themes
                        .iter()
                        .position(|t| *t == app_state.theme_variant)
                        .unwrap_or(0);
                    let next = themes[(current_idx + 1) % themes.len()];
                    app_state.set_theme(next);
                    return AppEvent::ThemeChanged(next);
                }
                _ => {}
            }
        }

        // F1 for help
        if key.code == KeyCode::F(1) {
            app_state.mode = if app_state.mode == AppMode::Help {
                AppMode::Normal
            } else {
                AppMode::Help
            };
            return AppEvent::None;
        }

        // Handle modal keys
        match app_state.mode {
            AppMode::ConfirmDialog => return self.handle_confirm_keys(key, app_state),
            AppMode::TextInputDialog => return self.handle_text_input_keys(key, app_state),
            AppMode::ProgressDialog => return AppEvent::None,
            AppMode::Help => {
                if key.code == KeyCode::Esc || key.code == KeyCode::F(1) {
                    app_state.mode = AppMode::Normal;
                }
                return AppEvent::None;
            }
            _ => {}
        }

        // Handle focused panel keys
        match app_state.focused_panel {
            FocusedPanel::Chat => self.handle_chat_keys(key, app_state),
            FocusedPanel::Sidebar => self.handle_sidebar_keys(key, app_state),
            FocusedPanel::Modal => self.handle_modal_keys(key, app_state),
            _ => AppEvent::None,
        }
    }

    fn handle_confirm_keys(&mut self, key: KeyEvent, app_state: &mut AppState) -> AppEvent {
        if let Some(dialog) = &mut app_state.confirm_dialog {
            match key.code {
                KeyCode::Tab | KeyCode::Right => {
                    dialog.selected = (dialog.selected + 1) % 2;
                    AppEvent::None
                }
                KeyCode::BackTab | KeyCode::Left => {
                    dialog.selected = (dialog.selected + 1) % 2;
                    AppEvent::None
                }
                KeyCode::Enter => {
                    if dialog.selected == 0 {
                        AppEvent::ConfirmDialogConfirm
                    } else {
                        AppEvent::ConfirmDialogCancel
                    }
                }
                KeyCode::Esc => AppEvent::ConfirmDialogCancel,
                _ => AppEvent::None,
            }
        } else {
            AppEvent::None
        }
    }

    fn handle_text_input_keys(&mut self, key: KeyEvent, app_state: &mut AppState) -> AppEvent {
        if let Some(dialog) = &mut app_state.text_input_dialog {
            match key.code {
                KeyCode::Char(c) => {
                    dialog.value.insert(dialog.cursor_pos, c);
                    dialog.cursor_pos += 1;
                    AppEvent::None
                }
                KeyCode::Backspace => {
                    if dialog.cursor_pos > 0 {
                        dialog.cursor_pos -= 1;
                        dialog.value.remove(dialog.cursor_pos);
                    }
                    AppEvent::None
                }
                KeyCode::Delete => {
                    if dialog.cursor_pos < dialog.value.len() {
                        dialog.value.remove(dialog.cursor_pos);
                    }
                    AppEvent::None
                }
                KeyCode::Left => {
                    if dialog.cursor_pos > 0 {
                        dialog.cursor_pos -= 1;
                    }
                    AppEvent::None
                }
                KeyCode::Right => {
                    if dialog.cursor_pos < dialog.value.len() {
                        dialog.cursor_pos += 1;
                    }
                    AppEvent::None
                }
                KeyCode::Home => {
                    dialog.cursor_pos = 0;
                    AppEvent::None
                }
                KeyCode::End => {
                    dialog.cursor_pos = dialog.value.len();
                    AppEvent::None
                }
                KeyCode::Tab => {
                    // Could cycle between input and buttons
                    AppEvent::None
                }
                KeyCode::Enter => AppEvent::TextInputSubmit(dialog.value.clone()),
                KeyCode::Esc => AppEvent::TextInputCancel,
                _ => AppEvent::None,
            }
        } else {
            AppEvent::None
        }
    }

    fn handle_chat_keys(&mut self, key: KeyEvent, app_state: &mut AppState) -> AppEvent {
        let action = app_state.layout.chat_mut().handle_key(key);
        match action {
            ChatAction::SendMessage(msg) => AppEvent::SendMessage(msg),
            ChatAction::FocusGained => AppEvent::ChatFocusGained,
            ChatAction::FocusLost => {
                app_state.focused_panel = FocusedPanel::Sidebar;
                AppEvent::ChatFocusLost
            }
            ChatAction::Scrolled => AppEvent::ChatScrolled,
            ChatAction::MessageClicked(idx) => AppEvent::MessageClicked(idx),
            ChatAction::ContextMenu => AppEvent::ChatContextMenu,
            ChatAction::ToolExpanded(_, _) => AppEvent::None,
            _ => AppEvent::None,
        }
    }

    fn handle_sidebar_keys(&mut self, key: KeyEvent, app_state: &mut AppState) -> AppEvent {
        // Handle new session input first
        if app_state.layout.sidebar().show_new_session() {
            let consumed = app_state.layout.sidebar_mut().handle_new_session_input(key);
            if consumed {
                if key.code == KeyCode::Enter {
                    if let Some(name) = app_state.layout.sidebar().new_session_name() {
                        return AppEvent::NewSessionCreated(name);
                    }
                }
                return AppEvent::None;
            }
        }

        let action = app_state.layout.sidebar_mut().handle_key(key);
        match action {
            SidebarAction::TabChanged => AppEvent::SidebarTabChanged,
            SidebarAction::ItemSelected => AppEvent::SidebarItemSelected,
            SidebarAction::ShowNewSession => AppEvent::NewSessionRequested,
            SidebarAction::HideNewSession => AppEvent::NewSessionCancelled,
            SidebarAction::ShowSettings => AppEvent::SettingsRequested,
            SidebarAction::HideSettings => AppEvent::SettingsClosed,
            SidebarAction::SettingsTabChanged => AppEvent::SettingsTabChanged,
            SidebarAction::ContextMenu(idx) => AppEvent::SidebarContextMenu(idx),
            SidebarAction::NewSessionCreated(name) => AppEvent::NewSessionCreated(name),
            _ => AppEvent::None,
        }
    }

    fn handle_modal_keys(&mut self, _key: KeyEvent, _app_state: &mut AppState) -> AppEvent {
        AppEvent::None
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, app_state: &mut AppState) -> AppEvent {
        self.last_mouse_pos = (mouse.column, mouse.row);

        match mouse.kind {
            MouseEventKind::Down(btn) => {
                self.mouse_captured = true;
                AppEvent::MouseClick(btn, mouse.column, mouse.row)
            }
            MouseEventKind::Up(btn) => {
                self.mouse_captured = false;
                if let Some(drag_result) = app_state.ui_state.end_drag() {
                    match drag_result.0 {
                        DragElement::SidebarDivider => {
                            app_state.layout.set_sidebar_width(drag_result.1);
                        }
                        DragElement::ScrollbarThumb => {
                            // Handle scrollbar drag
                        }
                        _ => {}
                    }
                }
                AppEvent::MouseRelease
            }
            MouseEventKind::Drag(_) => {
                if self.mouse_captured {
                    if let Some(new_val) = app_state.ui_state.update_drag(mouse.column, mouse.row) {
                        if let Some(drag) = &app_state.ui_state.drag_state {
                            match drag.element {
                                DragElement::SidebarDivider => {
                                    app_state.layout.set_sidebar_width(new_val);
                                }
                                _ => {}
                            }
                        }
                    }
                    AppEvent::MouseDrag(mouse.column, mouse.row)
                } else {
                    AppEvent::None
                }
            }
            MouseEventKind::Moved => AppEvent::MouseMove(mouse.column, mouse.row),
            MouseEventKind::ScrollUp => AppEvent::MouseScroll(-1),
            MouseEventKind::ScrollDown => AppEvent::MouseScroll(1),
            MouseEventKind::ScrollLeft | MouseEventKind::ScrollRight => AppEvent::None,
        }
    }

    pub fn handle_app_event(&mut self, event: AppEvent, app_state: &mut AppState) {
        match event {
            AppEvent::SendMessage(msg) => {
                app_state.layout.chat_mut().set_input(msg);
            }
            AppEvent::NewSessionCreated(name) => {
                app_state.pending_new_session = Some(name);
            }
            AppEvent::ConfirmDialogConfirm => {
                if let Some(dialog) = app_state.confirm_dialog.take() {
                    match dialog.on_confirm {
                        crate::state::app_state::ConfirmAction::DeleteSession(id) => {
                            app_state.pending_delete_session = Some(id);
                        }
                        crate::state::app_state::ConfirmAction::ForkSession(id) => {
                            app_state.pending_fork_session = Some(id);
                        }
                        _ => {}
                    }
                }
                app_state.hide_modal();
            }
            AppEvent::ConfirmDialogCancel => {
                app_state.hide_modal();
            }
            AppEvent::TextInputSubmit(value) => {
                if let Some(dialog) = app_state.text_input_dialog.take() {
                    match dialog.on_submit {
                        crate::state::app_state::TextInputAction::NewSession => {
                            app_state.pending_new_session = Some(value);
                        }
                        crate::state::app_state::TextInputAction::InjectGuidance => {
                            if let Err(e) =
                                app_state.send_steer(bimo_core::SteerCommand::Inject(value))
                            {
                                app_state.add_toast(
                                    format!("Failed to inject guidance: {}", e),
                                    app_state.layout.get_styles().error,
                                    std::time::Duration::from_secs(3),
                                );
                            }
                        }
                        _ => {}
                    }
                }
                app_state.hide_modal();
            }
            AppEvent::TextInputCancel => {
                app_state.hide_modal();
            }
            AppEvent::ThemeChanged(variant) => {
                app_state.set_theme(variant);
            }
            AppEvent::ReducedMotionToggled => {
                app_state.set_reduced_motion(!app_state.reduced_motion);
            }
            AppEvent::Resize(_, _) => {
                // Handled by main loop
            }
            _ => {}
        }
    }
}

impl Default for EventHandler {
    fn default() -> Self {
        Self::new()
    }
}
