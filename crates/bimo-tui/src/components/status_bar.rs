use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    prelude::Stylize,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph, Widget},
};

use crate::theme::{Styles, Theme};

#[derive(Debug, Clone)]
pub struct StatusBar {
    provider: String,
    model: String,
    mode: StatusMode,
    max_steps: usize,
    current_step: usize,
    token_usage: Option<(u32, u32)>, // (input, output)
    cost_estimate: Option<f64>,
    is_streaming: bool,
    styles: Styles,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StatusMode {
    Ready,
    Running,
    Steering,
    Error(String),
}

impl StatusBar {
    pub fn new() -> Self {
        Self {
            provider: "No provider".to_string(),
            model: "No model".to_string(),
            mode: StatusMode::Ready,
            max_steps: 25,
            current_step: 0,
            token_usage: None,
            cost_estimate: None,
            is_streaming: false,
            styles: Styles::from_theme(&Theme::mocha()),
        }
    }

    pub fn styles(mut self, styles: Styles) -> Self {
        self.styles = styles;
        self
    }

    pub fn set_provider(&mut self, provider: String, model: String) {
        self.provider = provider;
        self.model = model;
    }

    pub fn set_mode(&mut self, mode: StatusMode) {
        self.mode = mode;
    }

    pub fn set_steps(&mut self, current: usize, max: usize) {
        self.current_step = current;
        self.max_steps = max;
    }

    pub fn set_tokens(&mut self, input: u32, output: u32) {
        self.token_usage = Some((input, output));
    }

    pub fn set_cost(&mut self, cost: f64) {
        self.cost_estimate = Some(cost);
    }

    pub fn set_streaming(&mut self, streaming: bool) {
        self.is_streaming = streaming;
    }
}

impl Widget for StatusBar {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height < 1 {
            return;
        }

        let block = Block::default()
            .borders(Borders::TOP)
            .border_style(self.styles.border)
            .style(self.styles.base);

        let inner = block.inner(area);
        block.render(area, buf);

        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(30), // Provider/Model
                Constraint::Percentage(40), // Mode/Steps
                Constraint::Percentage(30), // Tokens/Cost
            ])
            .split(inner);

        // Provider/Model
        let provider_text = if self.provider.is_empty() || self.provider == "No provider" {
            Line::from(vec![
                Span::styled("⚡ ", self.styles.warning),
                Span::styled("No provider", self.styles.text_muted),
            ])
        } else {
            Line::from(vec![
                Span::styled("⚡ ", self.styles.primary),
                Span::styled(&self.provider, self.styles.text.bold()),
                Span::styled(" / ", self.styles.text_dim),
                Span::styled(&self.model, self.styles.text),
            ])
        };
        Paragraph::new(provider_text)
            .style(self.styles.text)
            .alignment(Alignment::Left)
            .render(chunks[0], buf);

        // Mode/Steps
        let mode_text = match &self.mode {
            StatusMode::Ready => Line::from(vec![Span::styled("● Ready", self.styles.success)]),
            StatusMode::Running => Line::from(vec![
                Span::styled("● Running", self.styles.primary),
                if self.is_streaming {
                    Span::styled(" ⟳", self.styles.primary.add_modifier(Modifier::SLOW_BLINK))
                } else {
                    Span::styled("", self.styles.text)
                },
            ]),
            StatusMode::Steering => {
                Line::from(vec![Span::styled("◐ Steering", self.styles.warning.bold())])
            }
            StatusMode::Error(e) => Line::from(vec![
                Span::styled("✗ Error: ", self.styles.error),
                Span::styled(e, self.styles.text),
            ]),
        };

        let steps_text = if self.max_steps > 0 {
            let progress = self.current_step as f32 / self.max_steps as f32;
            let bar_width = 20;
            let filled = (bar_width as f32 * progress) as usize;
            let bar = "█".repeat(filled) + &"░".repeat(bar_width - filled);
            Line::from(vec![
                Span::styled(
                    format!("Steps: {}/{} ", self.current_step, self.max_steps),
                    self.styles.text_muted,
                ),
                Span::styled(
                    bar,
                    if progress > 0.8 {
                        self.styles.warning
                    } else {
                        self.styles.primary
                    },
                ),
            ])
        } else {
            Line::from("")
        };

        let mode_area = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Length(1)])
            .split(chunks[1]);

        Paragraph::new(mode_text)
            .style(self.styles.text)
            .alignment(Alignment::Center)
            .render(mode_area[0], buf);

        Paragraph::new(steps_text)
            .style(self.styles.text)
            .alignment(Alignment::Center)
            .render(mode_area[1], buf);

        // Tokens/Cost
        let tokens_text = if let Some((input, output)) = self.token_usage {
            let cost_text = self
                .cost_estimate
                .map(|c| format!(" ${:.4}", c))
                .unwrap_or_default();
            Line::from(vec![
                Span::styled(format!("↓{} ↑{}", input, output), self.styles.text_muted),
                Span::styled(cost_text, self.styles.success),
            ])
        } else {
            Line::from(vec![Span::styled("Tokens: --", self.styles.text_dim)])
        };

        Paragraph::new(tokens_text)
            .style(self.styles.text)
            .alignment(Alignment::Right)
            .render(chunks[2], buf);
    }
}

impl Default for StatusBar {
    fn default() -> Self {
        Self::new()
    }
}
