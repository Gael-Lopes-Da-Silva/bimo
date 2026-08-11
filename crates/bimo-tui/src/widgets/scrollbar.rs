use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::Widget,
};

pub struct ProportionalScrollbar {
    track_style: Style,
    thumb_style: Style,
    hover_thumb_style: Style,
    content_height: u16,
    viewport_height: u16,
    scroll_offset: u16,
    is_hovered: bool,
    is_dragging: bool,
    drag_start_y: u16,
    drag_start_offset: u16,
}

impl ProportionalScrollbar {
    pub fn new() -> Self {
        Self {
            track_style: Style::default(),
            thumb_style: Style::default(),
            hover_thumb_style: Style::default(),
            content_height: 0,
            viewport_height: 0,
            scroll_offset: 0,
            is_hovered: false,
            is_dragging: false,
            drag_start_y: 0,
            drag_start_offset: 0,
        }
    }

    pub fn styles(mut self, track: Style, thumb: Style, hover_thumb: Style) -> Self {
        self.track_style = track;
        self.thumb_style = thumb;
        self.hover_thumb_style = hover_thumb;
        self
    }

    pub fn content_height(mut self, height: u16) -> Self {
        self.content_height = height;
        self
    }

    pub fn viewport_height(mut self, height: u16) -> Self {
        self.viewport_height = height;
        self
    }

    pub fn scroll_offset(mut self, offset: u16) -> Self {
        self.scroll_offset = offset.min(self.content_height.saturating_sub(self.viewport_height));
        self
    }

    pub fn hovered(mut self, hovered: bool) -> Self {
        self.is_hovered = hovered;
        self
    }

    pub fn dragging(mut self, dragging: bool) -> Self {
        self.is_dragging = dragging;
        self
    }

    pub fn drag_start(mut self, y: u16, offset: u16) -> Self {
        self.drag_start_y = y;
        self.drag_start_offset = offset;
        self
    }

    pub fn thumb_height(&self) -> u16 {
        if self.content_height == 0 || self.viewport_height == 0 {
            return 1;
        }
        let ratio = self.viewport_height as f32 / self.content_height as f32;
        let height = (self.viewport_height as f32 * ratio).max(1.0) as u16;
        height.min(self.viewport_height)
    }

    pub fn thumb_position(&self) -> u16 {
        if self.content_height <= self.viewport_height {
            return 0;
        }
        let max_offset = self.content_height - self.viewport_height;
        let max_thumb_y = self.viewport_height - self.thumb_height();
        ((self.scroll_offset as f32 / max_offset as f32) * max_thumb_y as f32) as u16
    }

    pub fn contains_thumb(&self, area: Rect, y: u16) -> bool {
        let thumb_y = area.y + self.thumb_position();
        let thumb_h = self.thumb_height();
        y >= thumb_y && y < thumb_y + thumb_h
    }

    pub fn calculate_offset_from_drag(&self, area: Rect, current_y: u16) -> u16 {
        if self.content_height <= self.viewport_height {
            return 0;
        }
        let thumb_h = self.thumb_height();
        let max_thumb_y = self.viewport_height - thumb_h;
        let drag_delta = current_y.saturating_sub(self.drag_start_y);
        let new_thumb_y = (self.drag_start_offset as i16 + drag_delta as i16)
            .max(0)
            .min(max_thumb_y as i16) as u16;
        let max_offset = self.content_height - self.viewport_height;
        ((new_thumb_y as f32 / max_thumb_y as f32) * max_offset as f32) as u16
    }
}

impl Widget for ProportionalScrollbar {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if self.content_height <= self.viewport_height || area.height < 3 {
            return;
        }

        let thumb_y = area.y + self.thumb_position();
        let thumb_h = self.thumb_height();

        for y in area.y..area.y + area.height {
            let style = if y >= thumb_y && y < thumb_y + thumb_h {
                if self.is_dragging || self.is_hovered {
                    self.hover_thumb_style
                } else {
                    self.thumb_style
                }
            } else {
                self.track_style
            };

            buf.get_mut(area.x, y)
                .set_char(if y >= thumb_y && y < thumb_y + thumb_h {
                    '█'
                } else {
                    '│'
                })
                .set_style(style);
        }
    }
}
