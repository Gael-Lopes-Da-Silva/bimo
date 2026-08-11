use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};

pub struct Button<'a> {
    label: &'a str,
    style: Style,
    hover_style: Style,
    active_style: Style,
    disabled_style: Style,
    is_hovered: bool,
    is_active: bool,
    is_disabled: bool,
    selected: bool,
    width: Option<u16>,
}

impl<'a> Button<'a> {
    pub fn new(label: &'a str) -> Self {
        Self {
            label,
            style: Style::default(),
            hover_style: Style::default(),
            active_style: Style::default(),
            disabled_style: Style::default(),
            is_hovered: false,
            is_active: false,
            is_disabled: false,
            selected: false,
            width: None,
        }
    }

    pub fn styles(mut self, style: Style, hover: Style, active: Style, disabled: Style) -> Self {
        self.style = style;
        self.hover_style = hover;
        self.active_style = active;
        self.disabled_style = disabled;
        self
    }

    pub fn hovered(mut self, hovered: bool) -> Self {
        self.is_hovered = hovered;
        self
    }

    pub fn active(mut self, active: bool) -> Self {
        self.is_active = active;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.is_disabled = disabled;
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn width(mut self, width: u16) -> Self {
        self.width = Some(width);
        self
    }
}

impl Widget for Button<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let style = if self.is_disabled {
            self.disabled_style
        } else if self.is_active {
            self.active_style
        } else if self.is_hovered || self.selected {
            self.hover_style
        } else {
            self.style
        };

        let width = self
            .width
            .unwrap_or_else(|| (self.label.len() as u16 + 4).min(area.width));
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + area.height / 2;

        let button_area = Rect::new(x, y, width, 1);
        Paragraph::new(self.label)
            .style(style)
            .alignment(Alignment::Center)
            .render(button_area, buf);
    }
}
