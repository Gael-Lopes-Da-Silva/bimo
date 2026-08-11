use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};
use syntect::{
    easy::HighlightLines,
    highlighting::{Style as SyntectStyle, ThemeSet},
    parsing::SyntaxSet,
    util::LinesWithEndings,
};

use crate::theme::{Styles, Theme};

pub struct MessageBubble {
    role: String,
    content: String,
    timestamp: String,
    is_streaming: bool,
    styles: Styles,
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
}

impl MessageBubble {
    pub fn new(role: String, content: String, timestamp: String, is_streaming: bool) -> Self {
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let theme_set = ThemeSet::load_defaults();
        Self {
            role,
            content,
            timestamp,
            is_streaming,
            styles: Styles::from_theme(&Theme::mocha()),
            syntax_set,
            theme_set,
        }
    }

    pub fn styles(mut self, styles: Styles) -> Self {
        self.styles = styles;
        self
    }

    fn render_with_syntax(&self, area: Rect, buf: &mut Buffer, syntax: &str) {
        let syntax = self
            .syntax_set
            .find_syntax_by_extension(syntax)
            .or_else(|| self.syntax_set.find_syntax_by_name(syntax))
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());

        let theme = &self.theme_set.themes["base16-ocean.dark"];
        let mut highlighter = HighlightLines::new(syntax, theme);

        let mut y = area.y;
        for line in LinesWithEndings::from(&self.content) {
            if y >= area.y + area.height {
                break;
            }
            let ranges: Vec<(SyntectStyle, &str)> = highlighter
                .highlight_line(line, &self.syntax_set)
                .unwrap_or_default();
            let spans: Vec<Span> = ranges
                .into_iter()
                .map(|(style, text)| {
                    let fg = Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b);
                    let bg = Color::Rgb(style.background.r, style.background.g, style.background.b);
                    Span::styled(text, Style::default().fg(fg).bg(bg))
                })
                .collect();
            Paragraph::new(Line::from(spans))
                .style(self.styles.code_bg)
                .render(Rect::new(area.x, y, area.width, 1), buf);
            y += 1;
        }
    }
}

impl Widget for MessageBubble {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let (role_style, role_label) = match self.role.as_str() {
            "user" => (self.styles.user_msg, " You "),
            "assistant" => (self.styles.assistant_msg, " Assistant "),
            "tool" => (self.styles.tool_msg, " Tool "),
            "system" => (self.styles.system_msg, " System "),
            _ => (self.styles.text, " "),
        };

        let header = Line::from(vec![
            Span::styled(role_label, role_style.bold()),
            Span::styled(format!(" {}", self.timestamp), self.styles.text_muted),
        ]);

        let content_area = Rect::new(
            area.x,
            area.y + 1,
            area.width,
            area.height.saturating_sub(1),
        );

        let has_code_blocks = self.content.contains("```");

        if has_code_blocks {
            let parts: Vec<&str> = self.content.split("```").collect();
            let mut y = content_area.y;

            for (i, part) in parts.iter().enumerate() {
                if y >= content_area.y + content_area.height {
                    break;
                }

                if i % 2 == 0 {
                    let paragraph = Paragraph::new(*part)
                        .style(self.styles.text)
                        .wrap(Wrap { trim: true });
                    // Approximate line count based on content width
                    let height = (part.len() as u16 / content_area.width.max(1)).max(1) + 1;
                    let render_area = Rect::new(
                        content_area.x,
                        y,
                        content_area.width,
                        height.min(content_area.y + content_area.height - y),
                    );
                    paragraph.render(render_area, buf);
                    y += height;
                } else {
                    let lang_end = part.find('\n').unwrap_or(0);
                    let lang = &part[..lang_end];
                    let code = &part[lang_end + 1..];
                    let code_height = code.lines().count() as u16 + 2;
                    let render_area = Rect::new(
                        content_area.x,
                        y,
                        content_area.width,
                        code_height.min(content_area.y + content_area.height - y),
                    );
                    self.render_with_syntax(render_area, buf, lang);
                    y += code_height;
                }
            }
        } else {
            Paragraph::new(self.content)
                .style(self.styles.text)
                .wrap(Wrap { trim: true })
                .render(content_area, buf);
        }

        if self.is_streaming {
            let cursor_x = area.x + 2;
            let cursor_y = area.y + area.height.saturating_sub(1);
            if cursor_y < area.y + area.height {
                buf.get_mut(cursor_x, cursor_y)
                    .set_char('█')
                    .set_style(self.styles.text.add_modifier(Modifier::SLOW_BLINK));
            }
        }
    }
}

pub struct StreamingText {
    content: String,
    cursor_visible: bool,
    style: Style,
    cursor_style: Style,
}

impl StreamingText {
    pub fn new(content: String) -> Self {
        Self {
            content,
            cursor_visible: true,
            style: Style::default(),
            cursor_style: Style::default(),
        }
    }

    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    pub fn cursor_style(mut self, style: Style) -> Self {
        self.cursor_style = style;
        self
    }

    pub fn set_cursor_visible(&mut self, visible: bool) {
        self.cursor_visible = visible;
    }
}

impl Widget for StreamingText {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Paragraph::new(self.content.as_str())
            .style(self.style)
            .wrap(Wrap { trim: true })
            .render(area, buf);

        if self.cursor_visible {
            let lines: Vec<&str> = self.content.lines().collect();
            let last_line = lines.last().unwrap_or(&"");
            let cursor_x = area.x + last_line.len() as u16;
            let cursor_y = area.y + lines.len().saturating_sub(1) as u16;

            if cursor_x < area.x + area.width && cursor_y < area.y + area.height {
                buf.get_mut(cursor_x, cursor_y)
                    .set_char('█')
                    .set_style(self.cursor_style);
            }
        }
    }
}
