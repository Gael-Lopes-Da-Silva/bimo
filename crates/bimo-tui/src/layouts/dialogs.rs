use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget, Wrap},
};

use crate::theme::Styles;
use crate::widgets::{Button, Input, centered_rect};

pub struct ConfirmDialog {
    title: String,
    message: String,
    confirm_text: String,
    cancel_text: String,
    selected: usize, // 0 = confirm, 1 = cancel
    styles: Styles,
}

impl ConfirmDialog {
    pub fn new(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            confirm_text: "Confirm".to_string(),
            cancel_text: "Cancel".to_string(),
            selected: 1, // Default to cancel for safety
            styles: Styles::from_theme(&crate::theme::Theme::mocha()),
        }
    }

    pub fn confirm_text(mut self, text: impl Into<String>) -> Self {
        self.confirm_text = text.into();
        self
    }

    pub fn cancel_text(mut self, text: impl Into<String>) -> Self {
        self.cancel_text = text.into();
        self
    }

    pub fn selected(mut self, index: usize) -> Self {
        self.selected = index;
        self
    }

    pub fn styles(mut self, styles: Styles) -> Self {
        self.styles = styles;
        self
    }
}

impl Widget for ConfirmDialog {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let dialog_area = centered_rect(50, 25, area);
        Clear.render(dialog_area, buf);

        let block = Block::default()
            .title(self.title)
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_style(self.styles.border_focus)
            .style(self.styles.base);

        let inner = block.inner(dialog_area);
        block.render(dialog_area, buf);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),
                Constraint::Length(1),
                Constraint::Length(1),
            ])
            .split(inner);

        Paragraph::new(self.message)
            .style(self.styles.text)
            .wrap(Wrap { trim: true })
            .alignment(Alignment::Center)
            .render(chunks[0], buf);

        let buttons = vec![
            Span::styled(
                format!("  {}  ", self.confirm_text),
                if self.selected == 0 {
                    self.styles.selected_text
                } else {
                    self.styles.button
                },
            ),
            Span::styled("  ", self.styles.text),
            Span::styled(
                format!("  {}  ", self.cancel_text),
                if self.selected == 1 {
                    self.styles.selected_text
                } else {
                    self.styles.button
                },
            ),
        ];

        Paragraph::new(Line::from(buttons))
            .alignment(Alignment::Center)
            .render(chunks[2], buf);
    }
}

pub struct TextInputDialog {
    title: String,
    prompt: String,
    value: String,
    cursor_pos: usize,
    placeholder: String,
    masked: bool,
    confirm_text: String,
    cancel_text: String,
    styles: Styles,
    selected: usize,
}

impl TextInputDialog {
    pub fn new(title: impl Into<String>, prompt: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            prompt: prompt.into(),
            value: String::new(),
            cursor_pos: 0,
            placeholder: String::new(),
            masked: false,
            confirm_text: "OK".to_string(),
            cancel_text: "Cancel".to_string(),
            styles: Styles::from_theme(&crate::theme::Theme::mocha()),
            selected: 0,
        }
    }

    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.value = value.into();
        self.cursor_pos = self.value.len();
        self
    }

    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    pub fn masked(mut self, masked: bool) -> Self {
        self.masked = masked;
        self
    }

    pub fn confirm_text(mut self, text: impl Into<String>) -> Self {
        self.confirm_text = text.into();
        self
    }

    pub fn cancel_text(mut self, text: impl Into<String>) -> Self {
        self.cancel_text = text.into();
        self
    }

    pub fn styles(mut self, styles: Styles) -> Self {
        self.styles = styles;
        self
    }

    pub fn get_value(&self) -> &str {
        &self.value
    }

    pub fn set_value(&mut self, value: String) {
        self.value = value;
        self.cursor_pos = self.value.len();
    }

    pub fn handle_key(&mut self, key: crossterm::event::KeyEvent) -> DialogAction {
        use crossterm::event::KeyCode;

        match key.code {
            KeyCode::Char(c) => {
                self.value.insert(self.cursor_pos, c);
                self.cursor_pos += 1;
                DialogAction::None
            }
            KeyCode::Backspace => {
                if self.cursor_pos > 0 {
                    self.cursor_pos -= 1;
                    self.value.remove(self.cursor_pos);
                }
                DialogAction::None
            }
            KeyCode::Delete => {
                if self.cursor_pos < self.value.len() {
                    self.value.remove(self.cursor_pos);
                }
                DialogAction::None
            }
            KeyCode::Left => {
                if self.cursor_pos > 0 {
                    self.cursor_pos -= 1;
                }
                DialogAction::None
            }
            KeyCode::Right => {
                if self.cursor_pos < self.value.len() {
                    self.cursor_pos += 1;
                }
                DialogAction::None
            }
            KeyCode::Home => {
                self.cursor_pos = 0;
                DialogAction::None
            }
            KeyCode::End => {
                self.cursor_pos = self.value.len();
                DialogAction::None
            }
            KeyCode::Tab => {
                self.selected = (self.selected + 1) % 2;
                DialogAction::None
            }
            KeyCode::BackTab => {
                self.selected = (self.selected + 1) % 2;
                DialogAction::None
            }
            KeyCode::Enter => {
                if self.selected == 0 {
                    DialogAction::Confirm(self.value.clone())
                } else {
                    DialogAction::Cancel
                }
            }
            KeyCode::Esc => DialogAction::Cancel,
            _ => DialogAction::None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum DialogAction {
    None,
    Confirm(String),
    Cancel,
}

impl Widget for TextInputDialog {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let dialog_area = centered_rect(60, 30, area);
        Clear.render(dialog_area, buf);

        let block = Block::default()
            .title(self.title)
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_style(self.styles.border_focus)
            .style(self.styles.base);

        let inner = block.inner(dialog_area);
        block.render(dialog_area, buf);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(3),
                Constraint::Length(1),
                Constraint::Length(1),
            ])
            .split(inner);

        Paragraph::new(self.prompt)
            .style(self.styles.text_muted)
            .alignment(Alignment::Center)
            .render(chunks[0], buf);

        let display_value = if self.masked {
            "●".repeat(self.value.len())
        } else if self.value.is_empty() {
            self.placeholder.clone()
        } else {
            self.value.clone()
        };

        let input_style = if self.value.is_empty() && !self.masked {
            self.styles.text_muted
        } else {
            self.styles.input_focus
        };

        Paragraph::new(display_value)
            .style(input_style)
            .alignment(Alignment::Center)
            .render(chunks[1], buf);

        // Cursor
        if !self.masked && !self.value.is_empty() {
            let cursor_x = chunks[1].x + (chunks[1].width / 2) + self.cursor_pos as u16
                - self.value.len() as u16 / 2;
            if cursor_x < chunks[1].x + chunks[1].width {
                buf.get_mut(cursor_x, chunks[1].y).set_char('█').set_style(
                    self.styles
                        .input_focus
                        .add_modifier(ratatui::style::Modifier::REVERSED),
                );
            }
        }

        let buttons = vec![
            Span::styled(format!("  {}  ", self.confirm_text), self.styles.button),
            Span::styled("  ", self.styles.text),
            Span::styled(format!("  {}  ", self.cancel_text), self.styles.button),
        ];

        Paragraph::new(Line::from(buttons))
            .alignment(Alignment::Center)
            .render(chunks[3], buf);
    }
}

pub struct ProgressDialog {
    title: String,
    message: String,
    progress: f32, // 0.0 to 1.0
    styles: Styles,
}

impl ProgressDialog {
    pub fn new(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            progress: 0.0,
            styles: Styles::from_theme(&crate::theme::Theme::mocha()),
        }
    }

    pub fn progress(mut self, progress: f32) -> Self {
        self.progress = progress.clamp(0.0, 1.0);
        self
    }

    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.message = message.into();
        self
    }

    pub fn styles(mut self, styles: Styles) -> Self {
        self.styles = styles;
        self
    }
}

impl Widget for ProgressDialog {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let dialog_area = centered_rect(60, 20, area);
        Clear.render(dialog_area, buf);

        let block = Block::default()
            .title(self.title)
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_style(self.styles.border_focus)
            .style(self.styles.base);

        let inner = block.inner(dialog_area);
        block.render(dialog_area, buf);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
            ])
            .split(inner);

        Paragraph::new(self.message)
            .style(self.styles.text)
            .alignment(Alignment::Center)
            .render(chunks[0], buf);

        // Progress bar
        let bar_width = (chunks[1].width as f32 * self.progress) as u16;
        let bar =
            "█".repeat(bar_width as usize) + &"░".repeat((chunks[1].width - bar_width) as usize);

        Paragraph::new(bar)
            .style(self.styles.primary)
            .alignment(Alignment::Center)
            .render(chunks[1], buf);

        Paragraph::new(format!("{:.0}%", self.progress * 100.0))
            .style(self.styles.text_muted)
            .alignment(Alignment::Center)
            .render(chunks[2], buf);
    }
}
