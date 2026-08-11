use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::Style,
    widgets::{Paragraph, Widget},
};

pub struct Toast {
    message: String,
    style: Style,
    duration: std::time::Duration,
    start_time: std::time::Instant,
}

impl Toast {
    pub fn new(message: String, style: Style, duration: std::time::Duration) -> Self {
        Self {
            message,
            style,
            duration,
            start_time: std::time::Instant::now(),
        }
    }

    pub fn is_expired(&self) -> bool {
        self.start_time.elapsed() >= self.duration
    }

    pub fn progress(&self) -> f32 {
        (self.start_time.elapsed().as_secs_f32() / self.duration.as_secs_f32()).min(1.0)
    }
}

impl Widget for Toast {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let toast_width = (self.message.len() as u16 + 4).min(area.width - 4);
        let x = area.x + 2;
        let y = area.y + 2;

        let toast_area = Rect::new(x, y, toast_width, 1);
        Paragraph::new(self.message)
            .style(self.style)
            .alignment(Alignment::Left)
            .render(toast_area, buf);
    }
}
