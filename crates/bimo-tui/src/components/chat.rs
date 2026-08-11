use crossterm::event::{KeyCode, KeyEvent, MouseEvent, MouseEventKind};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    prelude::Stylize,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{
        Block, Borders, Clear, List as RatatuiList, ListItem, ListState, Paragraph, Scrollbar,
        ScrollbarOrientation, ScrollbarState, StatefulWidget, Widget, Wrap,
    },
};

use crate::theme::{Styles, Theme};
use crate::widgets::message::MessageBubble;
use crate::widgets::{Button, Divider, Input, ProportionalScrollbar, centered_rect};

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub id: String,
    pub role: String,
    pub content: String,
    pub timestamp: String,
    pub is_streaming: bool,
    pub tool_calls: Vec<ToolCall>,
}

#[derive(Debug, Clone)]
pub struct ToolCall {
    pub name: String,
    pub args: String,
    pub result: Option<String>,
    pub is_expanded: bool,
    pub is_error: bool,
}

#[derive(Debug, Clone)]
pub struct ChatView {
    messages: Vec<ChatMessage>,
    input_value: String,
    input_cursor: usize,
    input_history: Vec<String>,
    history_index: Option<usize>,
    scroll_offset: u16,
    scroll_state: ScrollbarState,
    list_state: ListState,
    styles: Styles,
    is_streaming: bool,
    streaming_message_id: Option<String>,
    streaming_buffer: String,
    show_tool_details: bool,
    hovered_message: Option<usize>,
    hovered_tool: Option<usize>,
    input_focused: bool,
    show_help: bool,
}

impl ChatView {
    pub fn new() -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));

        Self {
            messages: Vec::new(),
            input_value: String::new(),
            input_cursor: 0,
            input_history: Vec::new(),
            history_index: None,
            scroll_offset: 0,
            scroll_state: ScrollbarState::default(),
            list_state,
            styles: Styles::from_theme(&Theme::mocha()),
            is_streaming: false,
            streaming_message_id: None,
            streaming_buffer: String::new(),
            show_tool_details: false,
            hovered_message: None,
            hovered_tool: None,
            input_focused: true,
            show_help: false,
        }
    }

    pub fn styles(mut self, styles: Styles) -> Self {
        self.styles = styles;
        self
    }

    pub fn add_message(&mut self, message: ChatMessage) {
        self.messages.push(message);
        self.list_state
            .select(Some(self.messages.len().saturating_sub(1)));
        self.scroll_to_bottom();
    }

    pub fn append_streaming(&mut self, delta: &str) {
        self.streaming_buffer.push_str(delta);
        if let Some(id) = &self.streaming_message_id {
            if let Some(msg) = self.messages.iter_mut().find(|m| &m.id == id) {
                msg.content = self.streaming_buffer.clone();
                msg.is_streaming = true;
            }
        }
        self.scroll_to_bottom();
    }

    pub fn start_streaming(&mut self, message_id: String) {
        self.streaming_message_id = Some(message_id);
        self.streaming_buffer.clear();
        self.is_streaming = true;
    }

    pub fn end_streaming(&mut self) {
        self.is_streaming = false;
        self.streaming_message_id = None;
        self.streaming_buffer.clear();
    }

    pub fn add_tool_call(&mut self, message_id: &str, tool: ToolCall) {
        if let Some(msg) = self.messages.iter_mut().find(|m| m.id == message_id) {
            msg.tool_calls.push(tool);
        }
    }

    pub fn update_tool_result(
        &mut self,
        message_id: &str,
        tool_name: &str,
        result: String,
        is_error: bool,
    ) {
        if let Some(msg) = self.messages.iter_mut().find(|m| m.id == message_id) {
            if let Some(tool) = msg.tool_calls.iter_mut().find(|t| t.name == tool_name) {
                tool.result = Some(result);
                tool.is_error = is_error;
            }
        }
    }

    pub fn set_input(&mut self, value: String) {
        self.input_value = value;
        self.input_cursor = self.input_value.len();
    }

    pub fn input_value(&self) -> &str {
        &self.input_value
    }

    pub fn take_input(&mut self) -> String {
        let value = self.input_value.clone();
        if !value.trim().is_empty() {
            self.input_history.push(value.clone());
            self.history_index = None;
        }
        self.input_value.clear();
        self.input_cursor = 0;
        value
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> ChatAction {
        if self.input_focused {
            self.handle_input_key(key)
        } else {
            self.handle_list_key(key)
        }
    }

    fn handle_input_key(&mut self, key: KeyEvent) -> ChatAction {
        match key.code {
            KeyCode::Char(c) => {
                self.input_value.insert(self.input_cursor, c);
                self.input_cursor += 1;
                ChatAction::None
            }
            KeyCode::Backspace => {
                if self.input_cursor > 0 {
                    self.input_cursor -= 1;
                    self.input_value.remove(self.input_cursor);
                }
                ChatAction::None
            }
            KeyCode::Delete => {
                if self.input_cursor < self.input_value.len() {
                    self.input_value.remove(self.input_cursor);
                }
                ChatAction::None
            }
            KeyCode::Left => {
                if self.input_cursor > 0 {
                    self.input_cursor -= 1;
                }
                ChatAction::None
            }
            KeyCode::Right => {
                if self.input_cursor < self.input_value.len() {
                    self.input_cursor += 1;
                }
                ChatAction::None
            }
            KeyCode::Home => {
                self.input_cursor = 0;
                ChatAction::None
            }
            KeyCode::End => {
                self.input_cursor = self.input_value.len();
                ChatAction::None
            }
            KeyCode::Enter => {
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::SHIFT)
                {
                    self.input_value.insert(self.input_cursor, '\n');
                    self.input_cursor += 1;
                    ChatAction::None
                } else if !self.input_value.trim().is_empty() {
                    ChatAction::SendMessage(self.take_input())
                } else {
                    ChatAction::None
                }
            }
            KeyCode::Up => {
                if self.input_history.is_empty() {
                    ChatAction::None
                } else if self.history_index.is_none() {
                    self.history_index = Some(self.input_history.len() - 1);
                    self.input_value = self.input_history[self.history_index.unwrap()].clone();
                    self.input_cursor = self.input_value.len();
                    ChatAction::None
                } else if let Some(idx) = self.history_index {
                    if idx > 0 {
                        self.history_index = Some(idx - 1);
                        self.input_value = self.input_history[self.history_index.unwrap()].clone();
                        self.input_cursor = self.input_value.len();
                    }
                    ChatAction::None
                } else {
                    ChatAction::None
                }
            }
            KeyCode::Down => {
                if let Some(idx) = self.history_index {
                    if idx + 1 < self.input_history.len() {
                        self.history_index = Some(idx + 1);
                        self.input_value = self.input_history[self.history_index.unwrap()].clone();
                        self.input_cursor = self.input_value.len();
                    } else {
                        self.history_index = None;
                        self.input_value.clear();
                        self.input_cursor = 0;
                    }
                    ChatAction::None
                } else {
                    ChatAction::None
                }
            }
            KeyCode::Esc => {
                self.input_focused = false;
                ChatAction::FocusLost
            }
            KeyCode::Tab => {
                self.input_focused = false;
                ChatAction::FocusLost
            }
            KeyCode::F(1) => {
                self.show_help = !self.show_help;
                ChatAction::None
            }
            _ => ChatAction::None,
        }
    }

    fn handle_list_key(&mut self, key: KeyEvent) -> ChatAction {
        match key.code {
            KeyCode::Up => {
                if let Some(selected) = self.list_state.selected() {
                    if selected > 0 {
                        self.list_state.select(Some(selected - 1));
                    }
                }
                ChatAction::None
            }
            KeyCode::Down => {
                if let Some(selected) = self.list_state.selected() {
                    if selected + 1 < self.messages.len() {
                        self.list_state.select(Some(selected + 1));
                    }
                } else if !self.messages.is_empty() {
                    self.list_state.select(Some(0));
                }
                ChatAction::None
            }
            KeyCode::PageUp => {
                if let Some(selected) = self.list_state.selected() {
                    let new_selected = selected.saturating_sub(10);
                    self.list_state.select(Some(new_selected));
                }
                ChatAction::None
            }
            KeyCode::PageDown => {
                if let Some(selected) = self.list_state.selected() {
                    let new_selected = (selected + 10).min(self.messages.len().saturating_sub(1));
                    self.list_state.select(Some(new_selected));
                }
                ChatAction::None
            }
            KeyCode::Home => {
                self.list_state.select(Some(0));
                ChatAction::None
            }
            KeyCode::End => {
                self.list_state
                    .select(Some(self.messages.len().saturating_sub(1)));
                ChatAction::None
            }
            KeyCode::Enter | KeyCode::Tab | KeyCode::Char('i') => {
                self.input_focused = true;
                ChatAction::FocusGained
            }
            KeyCode::Char(' ') => {
                if let Some(selected) = self.list_state.selected() {
                    if let Some(msg) = self.messages.get_mut(selected) {
                        if !msg.tool_calls.is_empty() {
                            msg.tool_calls
                                .iter_mut()
                                .for_each(|t| t.is_expanded = !t.is_expanded);
                        }
                    }
                }
                ChatAction::None
            }
            KeyCode::F(1) => {
                self.show_help = !self.show_help;
                ChatAction::None
            }
            _ => ChatAction::None,
        }
    }

    pub fn handle_mouse(&mut self, mouse: MouseEvent, area: Rect) -> ChatAction {
        let input_area = self.input_area(area);
        let messages_area = self.messages_area(area);
        let mouse_pos = ratatui::layout::Position::new(mouse.column, mouse.row);

        if input_area.contains(mouse_pos) {
            match mouse.kind {
                MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
                    self.input_focused = true;
                    let relative_x = mouse.column.saturating_sub(input_area.x);
                    self.input_cursor = relative_x.min(self.input_value.len() as u16) as usize;
                    return ChatAction::FocusGained;
                }
                MouseEventKind::Down(crossterm::event::MouseButton::Right) => {
                    return ChatAction::ContextMenu;
                }
                _ => {}
            }
        }

        if messages_area.contains(mouse_pos) {
            self.input_focused = false;

            match mouse.kind {
                MouseEventKind::ScrollUp => {
                    self.scroll_up(3);
                    return ChatAction::Scrolled;
                }
                MouseEventKind::ScrollDown => {
                    self.scroll_down(3);
                    return ChatAction::Scrolled;
                }
                MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
                    let relative_y = mouse.row.saturating_sub(messages_area.y);
                    if let Some(selected) = self.list_state.selected() {
                        let msg = &self.messages[selected];
                        let msg_height = self.estimate_message_height(msg, messages_area.width);
                        if relative_y < msg_height {
                            return ChatAction::MessageClicked(selected);
                        }
                    }
                }
                _ => {}
            }
        }

        ChatAction::None
    }

    fn scroll_up(&mut self, lines: u16) {
        self.scroll_offset = self.scroll_offset.saturating_sub(lines);
        self.scroll_state = self.scroll_state.position(self.scroll_offset as usize);
    }

    fn scroll_down(&mut self, lines: u16) {
        let max_scroll = self
            .total_content_height()
            .saturating_sub(self.messages_area(Rect::default()).height);
        self.scroll_offset = (self.scroll_offset + lines).min(max_scroll);
        self.scroll_state = self.scroll_state.position(self.scroll_offset as usize);
    }

    fn scroll_to_bottom(&mut self) {
        let max_scroll = self.total_content_height();
        self.scroll_offset = max_scroll;
        self.scroll_state = self.scroll_state.position(self.scroll_offset as usize);
    }

    fn total_content_height(&self) -> u16 {
        self.messages
            .iter()
            .map(|m| self.estimate_message_height(m, 80))
            .sum::<u16>()
    }

    fn estimate_message_height(&self, msg: &ChatMessage, width: u16) -> u16 {
        let base_height = 3; // header + padding
        let content_lines = msg.content.lines().count() as u16;
        let wrapped_lines = (content_lines * 80 / width.max(1)).max(content_lines);
        let tool_height = if msg.tool_calls.is_empty() {
            0
        } else {
            msg.tool_calls.len() as u16 * 3
        };
        base_height + wrapped_lines + tool_height
    }

    fn messages_area(&self, area: Rect) -> Rect {
        let input_height = 5;
        Rect::new(
            area.x,
            area.y,
            area.width,
            area.height.saturating_sub(input_height),
        )
    }

    fn input_area(&self, area: Rect) -> Rect {
        let input_height = 5;
        Rect::new(
            area.x,
            area.y + area.height.saturating_sub(input_height),
            area.width,
            input_height.min(area.height),
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ChatAction {
    None,
    SendMessage(String),
    FocusGained,
    FocusLost,
    Scrolled,
    MessageClicked(usize),
    ContextMenu,
    ToolExpanded(usize, usize),
}

impl Widget for ChatView {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height < 8 {
            return;
        }

        let messages_area = self.messages_area(area);
        let input_area = self.input_area(area);

        // Render messages
        self.render_messages(messages_area, buf);

        // Render divider
        Divider::horizontal().style(self.styles.border).render(
            Rect::new(
                area.x,
                messages_area.y + messages_area.height,
                area.width,
                1,
            ),
            buf,
        );

        // Render input
        self.render_input(input_area, buf);

        // Render help overlay
        if self.show_help {
            self.render_help_overlay(area, buf);
        }
    }
}

impl ChatView {
    fn render_messages(&self, area: Rect, buf: &mut Buffer) {
        if self.messages.is_empty() {
            let welcome = vec![
                Line::from(""),
                Line::from(vec![Span::styled(
                    "Welcome to Bimo TUI",
                    self.styles.title.bold(),
                )]),
                Line::from(""),
                Line::from(vec![
                    Span::styled("Press ", self.styles.text_muted),
                    Span::styled("Ctrl+N", self.styles.keybind),
                    Span::styled(" for new session", self.styles.text_muted),
                ]),
                Line::from(vec![
                    Span::styled("Press ", self.styles.text_muted),
                    Span::styled("Ctrl+S", self.styles.keybind),
                    Span::styled(" for settings", self.styles.text_muted),
                ]),
                Line::from(vec![
                    Span::styled("Press ", self.styles.text_muted),
                    Span::styled("F1", self.styles.keybind),
                    Span::styled(" for help", self.styles.text_muted),
                ]),
            ];

            Paragraph::new(welcome)
                .style(self.styles.text)
                .alignment(ratatui::layout::Alignment::Center)
                .wrap(Wrap { trim: true })
                .render(area, buf);
            return;
        }

        // Custom rendering for messages since we need variable heights
        let mut y = area.y;
        for (i, msg) in self.messages.iter().enumerate() {
            if y >= area.y + area.height {
                break;
            }

            let is_selected = self.list_state.selected() == Some(i);
            let style = if is_selected {
                self.styles.selected
            } else {
                self.styles.text
            };

            let msg_height = self.estimate_message_height(msg, area.width);
            let msg_area = Rect::new(
                area.x,
                y,
                area.width,
                msg_height.min(area.y + area.height - y),
            );

            let bubble = MessageBubble::new(
                msg.role.clone(),
                msg.content.clone(),
                msg.timestamp.clone(),
                msg.is_streaming,
            )
            .styles(self.styles.clone());

            bubble.render(msg_area, buf);

            // Render tool calls if expanded
            if !msg.tool_calls.is_empty() {
                let tool_y = y + 2;
                for (ti, tool) in msg.tool_calls.iter().enumerate() {
                    if tool_y + 3 >= area.y + area.height {
                        break;
                    }
                    let tool_area = Rect::new(area.x + 2, tool_y, area.width - 4, 3);
                    self.render_tool_call(
                        tool,
                        tool_area,
                        buf,
                        ti == self.hovered_tool.unwrap_or(usize::MAX),
                    );
                }
            }

            y += msg_height;
        }

        // Render scrollbar
        let scrollbar_area = Rect::new(area.x + area.width - 1, area.y, 1, area.height);
        let mut scrollbar_state = ScrollbarState::default()
            .content_length(self.total_content_height() as usize)
            .viewport_content_length(area.height as usize)
            .position(self.scroll_offset as usize);

        Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .style(self.styles.scrollbar)
            .thumb_style(self.styles.scrollbar_thumb)
            .render(scrollbar_area, buf, &mut scrollbar_state);
    }

    fn render_tool_call(&self, tool: &ToolCall, area: Rect, buf: &mut Buffer, is_hovered: bool) {
        let style = if is_hovered {
            self.styles.button_hover
        } else {
            self.styles.tool_msg
        };

        let block = Block::default()
            .title(format!(" {} ", tool.name))
            .title_alignment(ratatui::layout::Alignment::Left)
            .borders(Borders::ALL)
            .border_style(if tool.is_error {
                self.styles.error
            } else {
                self.styles.border
            })
            .style(style);

        let inner = block.inner(area);
        block.render(area, buf);

        if tool.is_expanded {
            let content = if let Some(result) = &tool.result {
                format!("Args: {}\nResult: {}", tool.args, result)
            } else {
                format!("Args: {}\n[Running...]", tool.args)
            };
            Paragraph::new(content)
                .style(self.styles.text_dim)
                .wrap(Wrap { trim: true })
                .render(inner, buf);
        } else {
            Paragraph::new(if tool.result.is_some() {
                "Click to expand"
            } else {
                "Running..."
            })
            .style(self.styles.text_muted)
            .render(inner, buf);
        }
    }

    fn render_input(&self, area: Rect, buf: &mut Buffer) {
        let input = Input::new(&self.input_value, self.input_cursor)
            .placeholder("Type a message... (Shift+Enter for newline, Enter to send)")
            .styles(self.styles.input, self.styles.input_focus)
            .focused(self.input_focused);

        let block = Block::default()
            .borders(Borders::TOP)
            .border_style(self.styles.border)
            .style(self.styles.base);

        let inner = block.inner(area);
        block.render(area, buf);

        input.render(inner, buf);
    }

    fn render_help_overlay(&self, area: Rect, buf: &mut Buffer) {
        let help_area = centered_rect(60, 60, area);
        Clear.render(help_area, buf);

        let block = Block::default()
            .title(" Keyboard Shortcuts ")
            .title_alignment(ratatui::layout::Alignment::Center)
            .borders(Borders::ALL)
            .border_style(self.styles.border_focus)
            .style(self.styles.base);

        let inner = block.inner(help_area);
        block.render(help_area, buf);

        let shortcuts = vec![
            Line::from(vec![Span::styled("Global", self.styles.title.bold())]),
            Line::from(vec![
                Span::styled("  Ctrl+N  ", self.styles.keybind),
                Span::styled("New session", self.styles.text),
            ]),
            Line::from(vec![
                Span::styled("  Ctrl+S  ", self.styles.keybind),
                Span::styled("Settings", self.styles.text),
            ]),
            Line::from(vec![
                Span::styled("  Ctrl+B  ", self.styles.keybind),
                Span::styled("Toggle sidebar", self.styles.text),
            ]),
            Line::from(vec![
                Span::styled("  Ctrl+Z  ", self.styles.keybind),
                Span::styled("Undo", self.styles.text),
            ]),
            Line::from(vec![
                Span::styled("  Ctrl+Shift+Z  ", self.styles.keybind),
                Span::styled("Redo", self.styles.text),
            ]),
            Line::from(""),
            Line::from(vec![Span::styled("Chat", self.styles.title.bold())]),
            Line::from(vec![
                Span::styled("  Enter  ", self.styles.keybind),
                Span::styled("Send message", self.styles.text),
            ]),
            Line::from(vec![
                Span::styled("  Shift+Enter  ", self.styles.keybind),
                Span::styled("New line", self.styles.text),
            ]),
            Line::from(vec![
                Span::styled("  ↑/↓  ", self.styles.keybind),
                Span::styled("History / Navigate messages", self.styles.text),
            ]),
            Line::from(vec![
                Span::styled("  Tab  ", self.styles.keybind),
                Span::styled("Focus input/list", self.styles.text),
            ]),
            Line::from(vec![
                Span::styled("  Space  ", self.styles.keybind),
                Span::styled("Expand tool calls", self.styles.text),
            ]),
            Line::from(vec![
                Span::styled("  PgUp/PgDn  ", self.styles.keybind),
                Span::styled("Scroll messages", self.styles.text),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  F1  ", self.styles.keybind),
                Span::styled("Toggle this help", self.styles.text),
            ]),
            Line::from(vec![
                Span::styled("  Esc  ", self.styles.keybind),
                Span::styled("Close modal / Unfocus", self.styles.text),
            ]),
        ];

        Paragraph::new(shortcuts)
            .style(self.styles.text)
            .wrap(Wrap { trim: true })
            .render(inner, buf);
    }
}

impl Default for ChatView {
    fn default() -> Self {
        Self::new()
    }
}
