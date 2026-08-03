use cursive::style::{ColorStyle, Effect, Style};
use cursive::utils::markup::StyledString;
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

use crate::theme::ThemeColors;

/// Renders a markdown document as a styled string using the given theme.
///
/// Supports headings, bold, italic, inline code, code blocks, block quotes,
/// lists, links and horizontal rules. Every other element falls back to plain
/// text styling.
pub fn render_markdown(markdown: &str, colors: &ThemeColors) -> StyledString {
    let base = Style::from(ColorStyle::front(colors.text));
    let code_style = Style::from(ColorStyle::new(colors.text, colors.surface_alt));
    let heading_style = Style::from(ColorStyle::front(colors.primary)).combine(Effect::Bold);
    let muted_style = Style::from(ColorStyle::front(colors.muted));
    let link_style = Style::from(ColorStyle::front(colors.primary)).combine(Effect::Underline);
    let bold_style = Style::from(Effect::Bold).combine(base);
    let italic_style = Style::from(Effect::Italic).combine(base);

    let mut out = StyledString::plain("");
    let mut line = StyledString::plain("");
    let mut styles: Vec<Style> = vec![base];
    let mut prefixes: Vec<(String, Style)> = Vec::new();
    let mut prefix_applied = false;
    let mut in_code_block = false;

    for event in Parser::new_ext(markdown, Options::all()) {
        match event {
            Event::Start(tag) => match tag {
                Tag::Heading { .. } => {
                    flush_line(&mut out, &mut line, &mut prefix_applied);
                    styles.push(heading_style);
                }
                Tag::BlockQuote(_) => {
                    prefixes.push(("> ".to_string(), muted_style));
                }
                Tag::Item => {
                    prefixes.push(("• ".to_string(), muted_style));
                }
                Tag::Emphasis => {
                    styles.push(italic_style);
                }
                Tag::Strong => {
                    styles.push(bold_style);
                }
                Tag::Link { .. } => {
                    styles.push(link_style);
                }
                Tag::CodeBlock(_) => {
                    flush_line(&mut out, &mut line, &mut prefix_applied);
                    in_code_block = true;
                }
                _ => {}
            },
            Event::End(tag_end) => match tag_end {
                TagEnd::Heading(_) => {
                    styles.pop();
                    flush_line(&mut out, &mut line, &mut prefix_applied);
                }
                TagEnd::BlockQuote(_) => {
                    prefixes.pop();
                }
                TagEnd::Item => {
                    prefixes.pop();
                    flush_line(&mut out, &mut line, &mut prefix_applied);
                }
                TagEnd::Emphasis | TagEnd::Strong | TagEnd::Link => {
                    styles.pop();
                }
                TagEnd::CodeBlock => {
                    in_code_block = false;
                    flush_line(&mut out, &mut line, &mut prefix_applied);
                }
                _ => {}
            },
            Event::Text(text) => {
                let text = text.to_string();
                if in_code_block {
                    out.append_styled(text, code_style);
                } else {
                    if !prefix_applied {
                        apply_prefixes(&mut line, &prefixes);
                        prefix_applied = true;
                    }
                    let style = *styles.last().unwrap_or(&base);
                    line.append_styled(text, style);
                }
            }
            Event::Code(code) => {
                if !prefix_applied {
                    apply_prefixes(&mut line, &prefixes);
                    prefix_applied = true;
                }
                line.append_styled(format!("`{}`", code), code_style);
            }
            Event::SoftBreak => {
                if !prefix_applied {
                    apply_prefixes(&mut line, &prefixes);
                    prefix_applied = true;
                }
                line.append_plain(' ');
            }
            Event::HardBreak => {
                flush_line(&mut out, &mut line, &mut prefix_applied);
            }
            Event::Rule => {
                flush_line(&mut out, &mut line, &mut prefix_applied);
                out.append_styled("────────────────────────────────────────", muted_style);
                flush_line(&mut out, &mut line, &mut prefix_applied);
            }
            _ => {}
        }
    }

    flush_line(&mut out, &mut line, &mut prefix_applied);
    out
}

fn flush_line(out: &mut StyledString, line: &mut StyledString, prefix_applied: &mut bool) {
    let content = std::mem::take(line);
    *prefix_applied = false;
    if content.source().is_empty() {
        return;
    }
    out.append(content);
    out.append_plain('\n');
}

fn apply_prefixes(line: &mut StyledString, prefixes: &[(String, Style)]) {
    for (prefix, style) in prefixes {
        line.append_styled(prefix.as_str(), *style);
    }
}
