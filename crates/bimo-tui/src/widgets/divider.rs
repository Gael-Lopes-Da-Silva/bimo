use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    widgets::Widget,
};

pub struct Divider {
    vertical: bool,
    style: Style,
}

impl Divider {
    pub fn horizontal() -> Self {
        Self {
            vertical: false,
            style: Style::default(),
        }
    }

    pub fn vertical() -> Self {
        Self {
            vertical: true,
            style: Style::default(),
        }
    }

    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }
}

impl Widget for Divider {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if self.vertical {
            for y in area.y..area.y + area.height {
                buf.get_mut(area.x, y).set_char('│').set_style(self.style);
            }
        } else {
            for x in area.x..area.x + area.width {
                buf.get_mut(x, area.y).set_char('─').set_style(self.style);
            }
        }
    }
}

pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
