use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    prelude::Stylize,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Paragraph, Widget, Wrap},
};
use syntect::{
    easy::HighlightLines,
    highlighting::{Style as SyntectStyle, ThemeSet},
    parsing::SyntaxSet,
    util::LinesWithEndings,
};

pub struct MarkdownRenderer {
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
    base_style: Style,
    code_style: Style,
    heading_styles: [Style; 6],
    link_style: Style,
    blockquote_style: Style,
}

impl Default for MarkdownRenderer {
    fn default() -> Self {
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let theme_set = ThemeSet::load_defaults();
        Self {
            syntax_set,
            theme_set,
            base_style: Style::default(),
            code_style: Style::default(),
            heading_styles: [
                Style::default().bold().add_modifier(Modifier::UNDERLINED),
                Style::default().bold(),
                Style::default().bold(),
                Style::default().bold(),
                Style::default().bold(),
                Style::default().bold(),
            ],
            link_style: Style::default().underlined(),
            blockquote_style: Style::default().italic(),
        }
    }
}

impl MarkdownRenderer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn base_style(mut self, style: Style) -> Self {
        self.base_style = style;
        self
    }

    pub fn code_style(mut self, style: Style) -> Self {
        self.code_style = style;
        self
    }

    pub fn heading_styles(mut self, styles: [Style; 6]) -> Self {
        self.heading_styles = styles;
        self
    }

    pub fn link_style(mut self, style: Style) -> Self {
        self.link_style = style;
        self
    }

    pub fn blockquote_style(mut self, style: Style) -> Self {
        self.blockquote_style = style;
        self
    }

    fn parse_markdown(&self, content: &str) -> Vec<Line> {
        let mut lines = Vec::new();
        let mut in_code_block = false;
        let mut code_lang = String::new();
        let mut code_content = String::new();

        for line in content.lines() {
            if line.starts_with("```") {
                if !in_code_block {
                    in_code_block = true;
                    code_lang = line[3..].trim().to_string();
                    code_content.clear();
                } else {
                    in_code_block = false;
                    let highlighted = self.highlight_code(&code_lang, &code_content);
                    lines.extend(highlighted);
                }
                continue;
            }

            if in_code_block {
                code_content.push_str(line);
                code_content.push('\n');
                continue;
            }

            if line.starts_with('#') {
                let level = line.chars().take_while(|&c| c == '#').count().min(6);
                let text = line[level..].trim_start();
                lines.push(Line::from(Span::styled(
                    text.to_string(),
                    self.heading_styles[level - 1],
                )));
            } else if line.starts_with("> ") {
                let text = &line[2..];
                lines.push(Line::from(Span::styled(
                    format!("▌ {}", text),
                    self.blockquote_style,
                )));
            } else if line.starts_with("- ") || line.starts_with("* ") {
                let text = &line[2..];
                lines.push(Line::from(vec![
                    Span::styled("• ", self.base_style),
                    Span::styled(text.to_string(), self.base_style),
                ]));
            } else if line.starts_with("1. ") || line.starts_with("2. ") || line.starts_with("3. ")
            {
                let text = &line[3..];
                lines.push(Line::from(vec![
                    Span::styled("1. ", self.base_style),
                    Span::styled(text.to_string(), self.base_style),
                ]));
            } else if line.trim().is_empty() {
                lines.push(Line::from(""));
            } else {
                let mut spans = self.parse_inline(line);
                lines.push(Line::from(spans));
            }
        }

        if in_code_block {
            let highlighted = self.highlight_code(&code_lang, &code_content);
            lines.extend(highlighted);
        }

        lines
    }

    fn parse_inline(&self, text: &str) -> Vec<Span> {
        let mut spans = Vec::new();
        let mut remaining = text;
        let mut last_end = 0;

        while let Some(start) = remaining.find('[') {
            if let Some(mid) = remaining[start..].find("](") {
                let mid = start + mid;
                if let Some(end) = remaining[mid..].find(')') {
                    let end = mid + end + 1;

                    if start > last_end {
                        spans.push(Span::styled(
                            remaining[last_end..start].to_string(),
                            self.base_style,
                        ));
                    }

                    let link_text = &remaining[start + 1..mid];
                    let url = &remaining[mid + 2..end];
                    spans.push(Span::styled(
                        format!("{} ({})", link_text, url),
                        self.link_style,
                    ));

                    last_end = end;
                    remaining = &remaining[end..];
                    continue;
                }
            }
            break;
        }

        if last_end < text.len() {
            spans.push(Span::styled(text[last_end..].to_string(), self.base_style));
        }

        if spans.is_empty() {
            spans.push(Span::styled(text.to_string(), self.base_style));
        }

        spans
    }

    fn highlight_code(&self, lang: &str, code: &str) -> Vec<Line> {
        let syntax = self
            .syntax_set
            .find_syntax_by_extension(lang)
            .or_else(|| self.syntax_set.find_syntax_by_name(lang))
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());

        let theme = &self.theme_set.themes["base16-ocean.dark"];
        let mut highlighter = HighlightLines::new(syntax, theme);

        let mut lines = Vec::new();
        for line in LinesWithEndings::from(code) {
            let ranges: Vec<(SyntectStyle, &str)> = highlighter
                .highlight_line(line, &self.syntax_set)
                .unwrap_or_default();
            let spans: Vec<Span> = ranges
                .into_iter()
                .map(|(style, text)| {
                    let fg = Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b);
                    let bg = Color::Rgb(style.background.r, style.background.g, style.background.b);
                    Span::styled(text.to_string(), Style::default().fg(fg).bg(bg))
                })
                .collect();
            lines.push(Line::from(spans));
        }
        lines
    }
}

impl Widget for MarkdownRenderer {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let lines = self.parse_markdown("");
        let text = Text::from(lines);
        Paragraph::new(text)
            .style(self.base_style)
            .wrap(Wrap { trim: true })
            .alignment(Alignment::Left)
            .render(area, buf);
    }
}

pub fn render_markdown(
    content: &str,
    area: Rect,
    buf: &mut Buffer,
    base_style: Style,
    code_style: Style,
) {
    let renderer = MarkdownRenderer::new()
        .base_style(base_style)
        .code_style(code_style);
    let lines = renderer.parse_markdown(content);
    let text = Text::from(lines);
    Paragraph::new(text)
        .style(base_style)
        .wrap(Wrap { trim: true })
        .render(area, buf);
}
