use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget, Wrap},
};

pub struct Modal<'a> {
    title: &'a str,
    content: Vec<Line<'a>>,
    buttons: Vec<&'a str>,
    selected_button: usize,
    focused: bool,
    styles: crate::theme::Styles,
}

impl<'a> Modal<'a> {
    pub fn new(title: &'a str) -> Self {
        Self {
            title,
            content: Vec::new(),
            buttons: vec!["OK", "Cancel"],
            selected_button: 0,
            focused: true,
            styles: crate::theme::Styles::from_theme(&crate::theme::Theme::mocha()),
        }
    }

    pub fn content(mut self, content: Vec<Line<'a>>) -> Self {
        self.content = content;
        self
    }

    pub fn buttons(mut self, buttons: Vec<&'a str>) -> Self {
        self.buttons = buttons;
        self
    }

    pub fn selected_button(mut self, index: usize) -> Self {
        self.selected_button = index;
        self
    }

    pub fn styles(mut self, styles: crate::theme::Styles) -> Self {
        self.styles = styles;
        self
    }
}

impl Widget for Modal<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);

        let modal_width = (area.width as f32 * 0.6).max(50.0).min(80.0) as u16;
        let modal_height = (10 + self.content.len() as u16 + 3).min(area.height - 4);
        let x = (area.width.saturating_sub(modal_width)) / 2;
        let y = (area.height.saturating_sub(modal_height)) / 2;
        let modal_area = Rect::new(x, y, modal_width, modal_height);

        let block = Block::default()
            .title(self.title)
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_style(self.styles.border_focus)
            .style(self.styles.base);

        let inner = block.inner(modal_area);
        block.render(modal_area, buf);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),
                Constraint::Length(1),
                Constraint::Length(1),
            ])
            .split(inner);

        Paragraph::new(self.content)
            .style(self.styles.text)
            .wrap(Wrap { trim: true })
            .render(chunks[0], buf);

        let button_text: Vec<Span> = self
            .buttons
            .iter()
            .enumerate()
            .map(|(i, b)| {
                let style = if i == self.selected_button {
                    self.styles.selected_text
                } else {
                    self.styles.button
                };
                Span::styled(format!("  {}  ", b), style)
            })
            .collect();

        Paragraph::new(Line::from(button_text))
            .alignment(Alignment::Center)
            .render(chunks[2], buf);
    }
}
