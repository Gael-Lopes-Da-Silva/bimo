use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};

pub struct Input<'a> {
    label: Option<&'a str>,
    placeholder: &'a str,
    value: &'a str,
    cursor_pos: usize,
    style: Style,
    focus_style: Style,
    focused: bool,
    masked: bool,
    show_cursor: bool,
}

impl<'a> Input<'a> {
    pub fn new(value: &'a str, cursor_pos: usize) -> Self {
        Self {
            label: None,
            placeholder: "",
            value,
            cursor_pos,
            style: Style::default(),
            focus_style: Style::default(),
            focused: false,
            masked: false,
            show_cursor: true,
        }
    }

    pub fn label(mut self, label: &'a str) -> Self {
        self.label = Some(label);
        self
    }

    pub fn placeholder(mut self, placeholder: &'a str) -> Self {
        self.placeholder = placeholder;
        self
    }

    pub fn styles(mut self, style: Style, focus_style: Style) -> Self {
        self.style = style;
        self.focus_style = focus_style;
        self
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    pub fn masked(mut self, masked: bool) -> Self {
        self.masked = masked;
        self
    }

    pub fn show_cursor(mut self, show: bool) -> Self {
        self.show_cursor = show;
        self
    }
}

impl Widget for Input<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let style = if self.focused {
            self.focus_style
        } else {
            self.style
        };

        let display_value = if self.masked {
            "●".repeat(self.value.len())
        } else if self.value.is_empty() && !self.focused {
            self.placeholder.to_string()
        } else {
            self.value.to_string()
        };

        let prefix = self.label.map(|l| format!("{} ", l)).unwrap_or_default();
        let content = format!("{}{}", prefix, display_value);

        Paragraph::new(content).style(style).render(area, buf);

        if self.focused && self.show_cursor && !self.value.is_empty() {
            let cursor_x =
                area.x + prefix.len() as u16 + self.cursor_pos.min(self.value.len()) as u16;
            if cursor_x < area.x + area.width {
                buf.get_mut(cursor_x, area.y)
                    .set_char('█')
                    .set_style(style.add_modifier(Modifier::REVERSED));
            }
        }
    }
}

use ratatui::style::Modifier;
