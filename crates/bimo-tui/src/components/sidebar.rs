use crossterm::event::{KeyCode, KeyEvent, MouseEvent, MouseEventKind};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    prelude::Stylize,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Clear, List as RatatuiList, ListItem, ListState, Paragraph, StatefulWidget,
        Widget, Wrap,
    },
};

use crate::theme::{Styles, Theme, ThemeVariant};
use crate::widgets::{
    Button, Divider, Input, Modal, ProportionalScrollbar, SelectableList, Toast, centered_rect,
};

#[derive(Debug, Clone, PartialEq)]
pub enum SidebarTab {
    Sessions,
    Providers,
    Settings,
    Skills,
    Snapshots,
    Todo,
}

impl SidebarTab {
    pub fn label(&self) -> &'static str {
        match self {
            SidebarTab::Sessions => "Sessions",
            SidebarTab::Providers => "Providers",
            SidebarTab::Settings => "Settings",
            SidebarTab::Skills => "Skills",
            SidebarTab::Snapshots => "Snapshots",
            SidebarTab::Todo => "Todo",
        }
    }

    pub fn all() -> Vec<SidebarTab> {
        vec![
            SidebarTab::Sessions,
            SidebarTab::Providers,
            SidebarTab::Settings,
            SidebarTab::Skills,
            SidebarTab::Snapshots,
            SidebarTab::Todo,
        ]
    }
}

#[derive(Debug, Clone)]
pub struct SessionItem {
    pub id: String,
    pub name: String,
    pub model: String,
    pub updated: String,
    pub message_count: usize,
    pub is_active: bool,
}

#[derive(Debug, Clone)]
pub struct Sidebar {
    tabs: Vec<SidebarTab>,
    active_tab: usize,
    sessions: Vec<SessionItem>,
    session_state: ListState,
    providers: Vec<ProviderItem>,
    provider_state: ListState,
    styles: Styles,
    theme_variant: ThemeVariant,
    show_new_session: bool,
    new_session_input: String,
    new_session_cursor: usize,
    show_settings_modal: bool,
    settings_tabs: Vec<SettingsTab>,
    settings_active_tab: usize,
    settings_scroll_offset: u16,
    hovered_tab: Option<usize>,
    divider_dragging: bool,
    sidebar_width: u16,
}

#[derive(Debug, Clone)]
pub struct ProviderItem {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub api_format: String,
    pub base_url: String,
    pub is_default: bool,
    pub models: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SettingsTab {
    General,
    Defaults,
    Retry,
    Providers,
    Skills,
    Appearance,
}

impl SettingsTab {
    pub fn label(&self) -> &'static str {
        match self {
            SettingsTab::General => "General",
            SettingsTab::Defaults => "Defaults",
            SettingsTab::Retry => "Retry",
            SettingsTab::Providers => "Providers",
            SettingsTab::Skills => "Skills",
            SettingsTab::Appearance => "Appearance",
        }
    }

    pub fn all() -> Vec<SettingsTab> {
        vec![
            SettingsTab::General,
            SettingsTab::Defaults,
            SettingsTab::Retry,
            SettingsTab::Providers,
            SettingsTab::Skills,
            SettingsTab::Appearance,
        ]
    }
}

impl Sidebar {
    pub fn new() -> Self {
        let mut session_state = ListState::default();
        session_state.select(Some(0));

        let mut provider_state = ListState::default();
        provider_state.select(Some(0));

        Self {
            tabs: SidebarTab::all(),
            active_tab: 0,
            sessions: Vec::new(),
            session_state,
            providers: Vec::new(),
            provider_state,
            styles: Styles::from_theme(&Theme::mocha()),
            theme_variant: ThemeVariant::Mocha,
            show_new_session: false,
            new_session_input: String::new(),
            new_session_cursor: 0,
            show_settings_modal: false,
            settings_tabs: SettingsTab::all(),
            settings_active_tab: 0,
            settings_scroll_offset: 0,
            hovered_tab: None,
            divider_dragging: false,
            sidebar_width: 40,
        }
    }

    pub fn styles(mut self, styles: Styles) -> Self {
        self.styles = styles;
        self
    }

    pub fn theme_variant(mut self, variant: ThemeVariant) -> Self {
        self.theme_variant = variant;
        self.styles = Styles::from_theme(&Theme::from_variant(variant));
        self
    }

    pub fn set_sessions(&mut self, sessions: Vec<SessionItem>) {
        self.sessions = sessions;
        if self.session_state.selected().is_none() && !self.sessions.is_empty() {
            self.session_state.select(Some(0));
        }
    }

    pub fn set_providers(&mut self, providers: Vec<ProviderItem>) {
        self.providers = providers;
        if self.provider_state.selected().is_none() && !self.providers.is_empty() {
            self.provider_state.select(Some(0));
        }
    }

    pub fn active_tab(&self) -> SidebarTab {
        self.tabs[self.active_tab].clone()
    }

    pub fn selected_session(&self) -> Option<&SessionItem> {
        self.session_state
            .selected()
            .and_then(|i| self.sessions.get(i))
    }

    pub fn selected_provider(&self) -> Option<&ProviderItem> {
        self.provider_state
            .selected()
            .and_then(|i| self.providers.get(i))
    }

    pub fn show_new_session(&self) -> bool {
        self.show_new_session
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> SidebarAction {
        match key.code {
            KeyCode::Tab
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                self.active_tab = (self.active_tab + 1) % self.tabs.len();
                SidebarAction::TabChanged
            }
            KeyCode::BackTab
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                self.active_tab = (self.active_tab + self.tabs.len() - 1) % self.tabs.len();
                SidebarAction::TabChanged
            }
            KeyCode::Up => {
                self.handle_list_up();
                SidebarAction::None
            }
            KeyCode::Down => {
                self.handle_list_down();
                SidebarAction::None
            }
            KeyCode::Enter => {
                self.handle_enter();
                SidebarAction::ItemSelected
            }
            KeyCode::Char('n')
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                self.show_new_session = true;
                self.new_session_input.clear();
                self.new_session_cursor = 0;
                SidebarAction::ShowNewSession
            }
            KeyCode::Char('s')
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                self.show_settings_modal = true;
                SidebarAction::ShowSettings
            }
            KeyCode::Esc => {
                if self.show_new_session {
                    self.show_new_session = false;
                    SidebarAction::HideNewSession
                } else if self.show_settings_modal {
                    self.show_settings_modal = false;
                    SidebarAction::HideSettings
                } else {
                    SidebarAction::None
                }
            }
            _ => SidebarAction::None,
        }
    }

    pub fn handle_mouse(&mut self, mouse: MouseEvent, area: Rect) -> SidebarAction {
        let sidebar_area = Rect::new(area.x, area.y, self.sidebar_width, area.height);
        let mouse_pos = ratatui::layout::Position::new(mouse.column, mouse.row);
        if !sidebar_area.contains(mouse_pos) {
            return SidebarAction::None;
        }

        let tab_height = 3;
        let tab_area = Rect::new(
            sidebar_area.x,
            sidebar_area.y,
            sidebar_area.width,
            tab_height,
        );

        if tab_area.contains(mouse_pos) {
            let tab_width = sidebar_area.width / self.tabs.len() as u16;
            let tab_index = ((mouse.column - sidebar_area.x) / tab_width)
                .min((self.tabs.len() - 1) as u16) as usize;

            match mouse.kind {
                MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
                    self.active_tab = tab_index;
                    return SidebarAction::TabChanged;
                }
                MouseEventKind::Moved => {
                    self.hovered_tab = Some(tab_index);
                    return SidebarAction::None;
                }
                _ => {}
            }
        }

        let content_y = sidebar_area.y + tab_height + 1;
        let content_height = sidebar_area.height - tab_height - 2;
        let content_area = Rect::new(
            sidebar_area.x + 1,
            content_y,
            sidebar_area.width - 2,
            content_height,
        );

        if content_area.contains(mouse_pos) {
            match self.tabs[self.active_tab] {
                SidebarTab::Sessions => self.handle_session_mouse(mouse, content_area),
                SidebarTab::Providers => self.handle_provider_mouse(mouse, content_area),
                SidebarTab::Settings => self.handle_settings_mouse(mouse, content_area),
                _ => SidebarAction::None,
            }
        } else {
            SidebarAction::None
        }
    }

    fn handle_list_up(&mut self) {
        match self.tabs[self.active_tab] {
            SidebarTab::Sessions => {
                if let Some(selected) = self.session_state.selected() {
                    if selected > 0 {
                        self.session_state.select(Some(selected - 1));
                    }
                }
            }
            SidebarTab::Providers => {
                if let Some(selected) = self.provider_state.selected() {
                    if selected > 0 {
                        self.provider_state.select(Some(selected - 1));
                    }
                }
            }
            SidebarTab::Settings => {
                if self.settings_active_tab > 0 {
                    self.settings_active_tab -= 1;
                }
            }
            _ => {}
        }
    }

    fn handle_list_down(&mut self) {
        match self.tabs[self.active_tab] {
            SidebarTab::Sessions => {
                if let Some(selected) = self.session_state.selected() {
                    if selected + 1 < self.sessions.len() {
                        self.session_state.select(Some(selected + 1));
                    }
                } else if !self.sessions.is_empty() {
                    self.session_state.select(Some(0));
                }
            }
            SidebarTab::Providers => {
                if let Some(selected) = self.provider_state.selected() {
                    if selected + 1 < self.providers.len() {
                        self.provider_state.select(Some(selected + 1));
                    }
                } else if !self.providers.is_empty() {
                    self.provider_state.select(Some(0));
                }
            }
            SidebarTab::Settings => {
                if self.settings_active_tab + 1 < self.settings_tabs.len() {
                    self.settings_active_tab += 1;
                }
            }
            _ => {}
        }
    }

    fn handle_enter(&mut self) {
        match self.tabs[self.active_tab] {
            SidebarTab::Sessions => {
                if self.session_state.selected().is_some() {
                    // Session selected
                }
            }
            SidebarTab::Providers => {}
            SidebarTab::Settings => {}
            _ => {}
        }
    }

    fn handle_session_mouse(&mut self, mouse: MouseEvent, area: Rect) -> SidebarAction {
        let item_height = 3;
        let relative_y = mouse.row.saturating_sub(area.y);
        let index = (relative_y / item_height) as usize;

        if index < self.sessions.len() {
            match mouse.kind {
                MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
                    self.session_state.select(Some(index));
                    return SidebarAction::ItemSelected;
                }
                MouseEventKind::Down(crossterm::event::MouseButton::Right) => {
                    self.session_state.select(Some(index));
                    return SidebarAction::ContextMenu(index);
                }
                _ => {}
            }
        }
        SidebarAction::None
    }

    fn handle_provider_mouse(&mut self, mouse: MouseEvent, area: Rect) -> SidebarAction {
        let item_height = 2;
        let relative_y = mouse.row.saturating_sub(area.y);
        let index = (relative_y / item_height) as usize;

        if index < self.providers.len() {
            match mouse.kind {
                MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
                    self.provider_state.select(Some(index));
                    return SidebarAction::ItemSelected;
                }
                _ => {}
            }
        }
        SidebarAction::None
    }

    fn handle_settings_mouse(&mut self, mouse: MouseEvent, area: Rect) -> SidebarAction {
        let tab_height = 1;
        let settings_tab_area = Rect::new(
            area.x,
            area.y,
            area.width,
            self.settings_tabs.len() as u16 * tab_height,
        );
        let mouse_pos = ratatui::layout::Position::new(mouse.column, mouse.row);

        if settings_tab_area.contains(mouse_pos) {
            let relative_y = mouse.row.saturating_sub(area.y);
            let index = relative_y as usize;
            if index < self.settings_tabs.len() {
                match mouse.kind {
                    MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
                        self.settings_active_tab = index;
                        return SidebarAction::SettingsTabChanged;
                    }
                    _ => {}
                }
            }
        }
        SidebarAction::None
    }

    pub fn handle_new_session_input(&mut self, key: KeyEvent) -> bool {
        if !self.show_new_session {
            return false;
        }

        match key.code {
            KeyCode::Char(c) => {
                self.new_session_input.insert(self.new_session_cursor, c);
                self.new_session_cursor += 1;
                true
            }
            KeyCode::Backspace => {
                if self.new_session_cursor > 0 {
                    self.new_session_cursor -= 1;
                    self.new_session_input.remove(self.new_session_cursor);
                    true
                } else {
                    false
                }
            }
            KeyCode::Delete => {
                if self.new_session_cursor < self.new_session_input.len() {
                    self.new_session_input.remove(self.new_session_cursor);
                    true
                } else {
                    false
                }
            }
            KeyCode::Left => {
                if self.new_session_cursor > 0 {
                    self.new_session_cursor -= 1;
                    true
                } else {
                    false
                }
            }
            KeyCode::Right => {
                if self.new_session_cursor < self.new_session_input.len() {
                    self.new_session_cursor += 1;
                    true
                } else {
                    false
                }
            }
            KeyCode::Enter => {
                self.show_new_session = false;
                true
            }
            KeyCode::Esc => {
                self.show_new_session = false;
                self.new_session_input.clear();
                self.new_session_cursor = 0;
                true
            }
            _ => false,
        }
    }

    pub fn new_session_name(&self) -> Option<String> {
        if !self.new_session_input.trim().is_empty() {
            Some(self.new_session_input.trim().to_string())
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SidebarAction {
    None,
    TabChanged,
    ItemSelected,
    ShowNewSession,
    HideNewSession,
    ShowSettings,
    HideSettings,
    SettingsTabChanged,
    ContextMenu(usize),
    NewSessionCreated(String),
}

impl Widget for Sidebar {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let sidebar_area = Rect::new(
            area.x,
            area.y,
            self.sidebar_width.min(area.width),
            area.height,
        );

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Tabs
                Constraint::Min(0),    // Content
                Constraint::Length(1), // Status
            ])
            .split(sidebar_area);

        // Render tabs
        self.render_tabs(chunks[0], buf);

        // Render content
        self.render_content(chunks[1], buf);

        // Render status
        self.render_status(chunks[2], buf);

        // Render overlays
        if self.show_new_session {
            self.render_new_session_modal(area, buf);
        }
        if self.show_settings_modal {
            self.render_settings_modal(area, buf);
        }
    }
}

impl Sidebar {
    fn render_tabs(&self, area: Rect, buf: &mut Buffer) {
        let tab_count = self.tabs.len() as u16;
        let tab_width = area.width / tab_count;

        for (i, tab) in self.tabs.iter().enumerate() {
            let x = area.x + i as u16 * tab_width;
            let tab_area = Rect::new(x, area.y, tab_width, area.height);

            let is_active = i == self.active_tab;
            let is_hovered = self.hovered_tab == Some(i);

            let style = if is_active {
                self.styles.selected_text
            } else if is_hovered {
                self.styles.button_hover
            } else {
                self.styles.button
            };

            let block = Block::default()
                .borders(Borders::BOTTOM)
                .border_style(if is_active {
                    self.styles.border_focus
                } else {
                    self.styles.border
                })
                .style(style);

            let inner = block.inner(tab_area);
            block.render(tab_area, buf);

            Paragraph::new(tab.label())
                .style(style)
                .alignment(ratatui::layout::Alignment::Center)
                .render(inner, buf);
        }
    }

    fn render_content(&self, area: Rect, buf: &mut Buffer) {
        match self.tabs[self.active_tab] {
            SidebarTab::Sessions => self.render_sessions(area, buf),
            SidebarTab::Providers => self.render_providers(area, buf),
            SidebarTab::Settings => self.render_settings_list(area, buf),
            SidebarTab::Skills => self.render_skills(area, buf),
            SidebarTab::Snapshots => self.render_snapshots(area, buf),
            SidebarTab::Todo => self.render_todo(area, buf),
        }
    }

    fn render_sessions(&self, area: Rect, buf: &mut Buffer) {
        if self.sessions.is_empty() {
            Paragraph::new("No sessions\nPress Ctrl+N to create")
                .style(self.styles.text_muted)
                .alignment(ratatui::layout::Alignment::Center)
                .render(area, buf);
            return;
        }

        let items: Vec<ListItem> = self
            .sessions
            .iter()
            .map(|s| {
                let style = if s.is_active {
                    self.styles.selected
                } else {
                    self.styles.text
                };

                let content = vec![
                    Line::from(vec![
                        Span::styled(&s.name, style.bold()),
                        Span::styled(format!(" ({})", s.message_count), self.styles.text_muted),
                    ]),
                    Line::from(vec![
                        Span::styled(&s.model, self.styles.text_dim),
                        Span::styled(format!("  ·  {}", s.updated), self.styles.text_dim),
                    ]),
                ];
                ListItem::new(content).style(style)
            })
            .collect();

        let list = RatatuiList::new(items)
            .highlight_style(self.styles.selected)
            .highlight_symbol("► ");

        StatefulWidget::render(list, area, buf, &mut self.session_state.clone());
    }

    fn render_providers(&self, area: Rect, buf: &mut Buffer) {
        if self.providers.is_empty() {
            Paragraph::new("No providers configured")
                .style(self.styles.text_muted)
                .alignment(ratatui::layout::Alignment::Center)
                .render(area, buf);
            return;
        }

        let items: Vec<ListItem> = self
            .providers
            .iter()
            .map(|p| {
                let default_mark = if p.is_default { " ★" } else { "" };
                let kind_badge = if p.kind == "local" {
                    "[LOCAL]"
                } else {
                    "[CLOUD]"
                };

                ListItem::new(vec![
                    Line::from(vec![
                        Span::styled(&p.name, self.styles.text.bold()),
                        Span::styled(default_mark, self.styles.warning),
                        Span::styled(format!(" {}", kind_badge), self.styles.text_muted),
                    ]),
                    Line::from(vec![
                        Span::styled(&p.base_url, self.styles.text_dim),
                        Span::styled(format!(" ({})", p.api_format), self.styles.text_dim),
                    ]),
                ])
            })
            .collect();

        let list = RatatuiList::new(items)
            .highlight_style(self.styles.selected)
            .highlight_symbol("► ");

        StatefulWidget::render(list, area, buf, &mut self.provider_state.clone());
    }

    fn render_settings_list(&self, area: Rect, buf: &mut Buffer) {
        let items: Vec<ListItem> = self
            .settings_tabs
            .iter()
            .enumerate()
            .map(|(i, tab)| {
                let is_active = i == self.settings_active_tab;
                let style = if is_active {
                    self.styles.selected_text
                } else {
                    self.styles.text
                };

                ListItem::new(tab.label()).style(style)
            })
            .collect();

        let list = RatatuiList::new(items)
            .highlight_style(self.styles.selected)
            .highlight_symbol("► ");

        let mut state = ListState::default();
        state.select(Some(self.settings_active_tab));
        StatefulWidget::render(list, area, buf, &mut state);
    }

    fn render_skills(&self, area: Rect, buf: &mut Buffer) {
        Paragraph::new("Skills browser\n(Coming in Milestone 4)")
            .style(self.styles.text_muted)
            .alignment(ratatui::layout::Alignment::Center)
            .render(area, buf);
    }

    fn render_snapshots(&self, area: Rect, buf: &mut Buffer) {
        Paragraph::new("Snapshots viewer\n(Coming in Milestone 5)")
            .style(self.styles.text_muted)
            .alignment(ratatui::layout::Alignment::Center)
            .render(area, buf);
    }

    fn render_todo(&self, area: Rect, buf: &mut Buffer) {
        Paragraph::new("Todo board\n(Coming in Milestone 6)")
            .style(self.styles.text_muted)
            .alignment(ratatui::layout::Alignment::Center)
            .render(area, buf);
    }

    fn render_status(&self, area: Rect, buf: &mut Buffer) {
        let tab = &self.tabs[self.active_tab];
        Paragraph::new(format!("Tab: {}", tab.label()))
            .style(self.styles.text_dim)
            .alignment(ratatui::layout::Alignment::Center)
            .render(area, buf);
    }

    fn render_new_session_modal(&self, area: Rect, buf: &mut Buffer) {
        let modal_area = centered_rect(50, 20, area);
        Clear.render(modal_area, buf);

        let block = Block::default()
            .title("New Session")
            .title_alignment(ratatui::layout::Alignment::Center)
            .borders(Borders::ALL)
            .border_style(self.styles.border_focus)
            .style(self.styles.base);

        let inner = block.inner(modal_area);
        block.render(modal_area, buf);

        let input = Input::new(&self.new_session_input, self.new_session_cursor)
            .placeholder("Session name (optional)")
            .styles(self.styles.input, self.styles.input_focus)
            .focused(true);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(1),
                Constraint::Min(0),
            ])
            .split(inner);

        input.render(chunks[0], buf);

        let help = Paragraph::new("Enter: Create  |  Esc: Cancel")
            .style(self.styles.text_dim)
            .alignment(ratatui::layout::Alignment::Center);
        help.render(chunks[1], buf);
    }

    fn render_settings_modal(&self, area: Rect, buf: &mut Buffer) {
        let modal_area = centered_rect(80, 70, area);
        Clear.render(modal_area, buf);

        let block = Block::default()
            .title("Settings")
            .title_alignment(ratatui::layout::Alignment::Center)
            .borders(Borders::ALL)
            .border_style(self.styles.border_focus)
            .style(self.styles.base);

        let inner = block.inner(modal_area);
        block.render(modal_area, buf);

        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(20), // Settings tabs
                Constraint::Min(0),     // Settings content
            ])
            .split(inner);

        // Settings tabs
        self.render_settings_tabs(chunks[0], buf);

        // Settings content
        self.render_settings_content(chunks[1], buf);
    }

    fn render_settings_tabs(&self, area: Rect, buf: &mut Buffer) {
        let items: Vec<ListItem> = self
            .settings_tabs
            .iter()
            .enumerate()
            .map(|(i, tab)| {
                let is_active = i == self.settings_active_tab;
                let style = if is_active {
                    self.styles.selected_text
                } else {
                    self.styles.text
                };
                ListItem::new(tab.label()).style(style)
            })
            .collect();

        let list = RatatuiList::new(items)
            .highlight_style(self.styles.selected)
            .highlight_symbol("► ");

        let mut state = ListState::default();
        state.select(Some(self.settings_active_tab));
        StatefulWidget::render(list, area, buf, &mut state);
    }

    fn render_settings_content(&self, area: Rect, buf: &mut Buffer) {
        let tab = &self.settings_tabs[self.settings_active_tab];

        match tab {
            SettingsTab::General => self.render_general_settings(area, buf),
            SettingsTab::Defaults => self.render_defaults_settings(area, buf),
            SettingsTab::Retry => self.render_retry_settings(area, buf),
            SettingsTab::Providers => self.render_providers_settings(area, buf),
            SettingsTab::Skills => self.render_skills_settings(area, buf),
            SettingsTab::Appearance => self.render_appearance_settings(area, buf),
        }
    }

    fn render_general_settings(&self, area: Rect, buf: &mut Buffer) {
        let content = vec![
            Line::from(vec![
                Span::styled("Session TTL (hours): ", self.styles.text),
                Span::styled("24", self.styles.text_muted),
            ]),
            Line::from(vec![
                Span::styled("Max Sessions: ", self.styles.text),
                Span::styled("50", self.styles.text_muted),
            ]),
            Line::from(vec![
                Span::styled("Cleanup Interval (min): ", self.styles.text),
                Span::styled("30", self.styles.text_muted),
            ]),
            Line::from(vec![
                Span::styled("Max Steps: ", self.styles.text),
                Span::styled("25", self.styles.text_muted),
            ]),
            Line::from(vec![
                Span::styled("Debug Mode: ", self.styles.text),
                Span::styled("Off", self.styles.text_muted),
            ]),
            Line::from(vec![
                Span::styled("Snapshots: ", self.styles.text),
                Span::styled("On", self.styles.success),
            ]),
        ];

        Paragraph::new(content)
            .style(self.styles.text)
            .wrap(Wrap { trim: true })
            .render(area, buf);
    }

    fn render_defaults_settings(&self, area: Rect, buf: &mut Buffer) {
        let content = vec![
            Line::from(vec![
                Span::styled("Default Provider: ", self.styles.text),
                Span::styled("ollama", self.styles.text_muted),
            ]),
            Line::from(vec![
                Span::styled("Default Model: ", self.styles.text),
                Span::styled("llama3", self.styles.text_muted),
            ]),
        ];

        Paragraph::new(content)
            .style(self.styles.text)
            .wrap(Wrap { trim: true })
            .render(area, buf);
    }

    fn render_retry_settings(&self, area: Rect, buf: &mut Buffer) {
        let content = vec![
            Line::from(vec![
                Span::styled("Retry Attempts: ", self.styles.text),
                Span::styled("10", self.styles.text_muted),
            ]),
            Line::from(vec![
                Span::styled("Retry Timeout (sec): ", self.styles.text),
                Span::styled("5", self.styles.text_muted),
            ]),
        ];

        Paragraph::new(content)
            .style(self.styles.text)
            .wrap(Wrap { trim: true })
            .render(area, buf);
    }

    fn render_providers_settings(&self, area: Rect, buf: &mut Buffer) {
        Paragraph::new("Provider management\n(Coming in Milestone 4)")
            .style(self.styles.text_muted)
            .alignment(ratatui::layout::Alignment::Center)
            .render(area, buf);
    }

    fn render_skills_settings(&self, area: Rect, buf: &mut Buffer) {
        Paragraph::new("Skills configuration\n(Coming in Milestone 4)")
            .style(self.styles.text_muted)
            .alignment(ratatui::layout::Alignment::Center)
            .render(area, buf);
    }

    fn render_appearance_settings(&self, area: Rect, buf: &mut Buffer) {
        let content = vec![
            Line::from(vec![
                Span::styled("Theme: ", self.styles.text),
                Span::styled(self.theme_variant.name(), self.styles.primary),
            ]),
            Line::from(""),
            Line::from(Span::styled("Available themes:", self.styles.text_muted)),
        ];

        let mut lines = content;
        for variant in ThemeVariant::ALL {
            let is_current = *variant == self.theme_variant;
            lines.push(Line::from(vec![Span::styled(
                format!(
                    "  {} {}",
                    if is_current { "►" } else { "  " },
                    variant.name()
                ),
                if is_current {
                    self.styles.selected_text
                } else {
                    self.styles.text
                },
            )]));
        }

        Paragraph::new(lines)
            .style(self.styles.text)
            .wrap(Wrap { trim: true })
            .render(area, buf);
    }
}

impl Default for Sidebar {
    fn default() -> Self {
        Self::new()
    }
}
