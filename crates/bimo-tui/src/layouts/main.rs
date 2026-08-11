use crate::components::chat::ChatView;
use crate::components::sidebar::Sidebar;
use crate::components::status_bar::StatusBar;
use crate::theme::Styles;
use crate::widgets::Divider;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    widgets::{Block, Widget},
};

#[derive(Debug, Clone, PartialEq)]
pub enum AppMode {
    Normal,
    Command,
    Settings,
    Help,
}

#[derive(Debug, Clone)]
pub struct MainLayout {
    chat: ChatView,
    sidebar: Sidebar,
    status_bar: StatusBar,
    sidebar_visible: bool,
    sidebar_width: u16,
    mode: AppMode,
    focused_panel: FocusedPanel,
    styles: Styles,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FocusedPanel {
    Chat,
    Sidebar,
    StatusBar,
}

impl MainLayout {
    pub fn new() -> Self {
        Self {
            chat: ChatView::new(),
            sidebar: Sidebar::new(),
            status_bar: StatusBar::new(),
            sidebar_visible: true,
            sidebar_width: 40,
            mode: AppMode::Normal,
            focused_panel: FocusedPanel::Chat,
            styles: Styles::from_theme(&crate::theme::Theme::mocha()),
        }
    }

    pub fn styles(mut self, styles: Styles) -> Self {
        self.styles = styles.clone();
        self.chat = self.chat.styles(styles.clone());
        self.sidebar = self.sidebar.styles(styles.clone());
        self.status_bar = self.status_bar.styles(styles);
        self
    }

    pub fn chat_mut(&mut self) -> &mut ChatView {
        &mut self.chat
    }

    pub fn sidebar_mut(&mut self) -> &mut Sidebar {
        &mut self.sidebar
    }

    pub fn status_bar_mut(&mut self) -> &mut StatusBar {
        &mut self.status_bar
    }

    pub fn toggle_sidebar(&mut self) {
        self.sidebar_visible = !self.sidebar_visible;
    }

    pub fn set_sidebar_width(&mut self, width: u16) {
        self.sidebar_width = width.clamp(20, 80);
    }

    pub fn set_mode(&mut self, mode: AppMode) {
        self.mode = mode;
    }

    pub fn set_focus(&mut self, panel: FocusedPanel) {
        self.focused_panel = panel;
    }

    pub fn chat(&self) -> &ChatView {
        &self.chat
    }

    pub fn sidebar(&self) -> &Sidebar {
        &self.sidebar
    }

    pub fn status_bar(&self) -> &StatusBar {
        &self.status_bar
    }

    pub fn sidebar_visible(&self) -> bool {
        self.sidebar_visible
    }

    pub fn sidebar_width(&self) -> u16 {
        self.sidebar_width
    }

    pub fn mode(&self) -> AppMode {
        self.mode.clone()
    }

    pub fn focused_panel(&self) -> FocusedPanel {
        self.focused_panel.clone()
    }

    pub fn get_styles(&self) -> Styles {
        self.styles.clone()
    }
}

impl Widget for MainLayout {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let main_block = Block::default().style(self.styles.base);
        main_block.render(area, buf);

        let (sidebar_area, chat_area) = if self.sidebar_visible {
            let sidebar_width = self.sidebar_width.min(area.width.saturating_sub(10));
            let sidebar_area = Rect::new(area.x, area.y, sidebar_width, area.height);
            let chat_area = Rect::new(
                area.x + sidebar_width,
                area.y,
                area.width.saturating_sub(sidebar_width),
                area.height,
            );
            (Some(sidebar_area), chat_area)
        } else {
            (None, area)
        };

        // Split chat area into messages and status bar
        let (messages_area, status_area) = {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(3), Constraint::Length(3)])
                .split(chat_area);
            (chunks[0], chunks[1])
        };

        // Render sidebar
        if let Some(sidebar_area) = sidebar_area {
            self.sidebar.render(sidebar_area, buf);
        }

        // Render chat messages
        self.chat.render(messages_area, buf);

        // Render status bar
        self.status_bar.render(status_area, buf);

        // Render divider between sidebar and chat
        if self.sidebar_visible {
            let divider_x = area.x + self.sidebar_width;
            if divider_x < area.x + area.width {
                let divider_area = Rect::new(divider_x, area.y, 1, area.height);
                Divider::vertical()
                    .style(self.styles.border)
                    .render(divider_area, buf);
            }
        }
    }
}

impl Default for MainLayout {
    fn default() -> Self {
        Self::new()
    }
}
