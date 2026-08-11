use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    widgets::{Block, Borders, Clear, Widget},
};

use crate::theme::Styles;

pub struct SettingsLayout {
    styles: Styles,
}

impl SettingsLayout {
    pub fn new() -> Self {
        Self {
            styles: Styles::from_theme(&crate::theme::Theme::mocha()),
        }
    }

    pub fn styles(mut self, styles: Styles) -> Self {
        self.styles = styles;
        self
    }
}

impl Widget for SettingsLayout {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);

        let block = Block::default()
            .title(" Settings ")
            .title_alignment(ratatui::layout::Alignment::Center)
            .borders(Borders::ALL)
            .border_style(self.styles.border_focus)
            .style(self.styles.base);

        let inner = block.inner(area);
        block.render(area, buf);

        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(22), Constraint::Min(0)])
            .split(inner);

        // Left: settings categories
        let categories = vec![
            "General",
            "Defaults",
            "Retry",
            "Providers",
            "Skills",
            "Appearance",
        ];

        let category_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                categories
                    .iter()
                    .map(|_| Constraint::Length(3))
                    .collect::<Vec<_>>(),
            )
            .split(chunks[0]);

        for (i, (cat, chunk)) in categories.iter().zip(category_chunks.iter()).enumerate() {
            let is_selected = false; // Would be passed in
            let style = if is_selected {
                self.styles.selected_text
            } else {
                self.styles.text
            };

            let block = Block::default()
                .borders(Borders::RIGHT)
                .border_style(if is_selected {
                    self.styles.border_focus
                } else {
                    self.styles.border
                })
                .style(style);

            let inner = block.inner(*chunk);
            block.render(*chunk, buf);

            ratatui::widgets::Paragraph::new(*cat)
                .style(style)
                .alignment(ratatui::layout::Alignment::Center)
                .render(inner, buf);
        }

        // Right: settings content
        let content_block = Block::default()
            .borders(Borders::NONE)
            .style(self.styles.base);
        content_block.render(chunks[1], buf);
    }
}

impl Default for SettingsLayout {
    fn default() -> Self {
        Self::new()
    }
}
