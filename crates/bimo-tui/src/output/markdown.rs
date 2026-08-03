use cursive::View;
use cursive::views::{LinearLayout, TextView};
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

pub struct MarkdownView {
    layout: LinearLayout,
}

impl MarkdownView {
    pub fn new() -> Self {
        Self {
            layout: LinearLayout::vertical(),
        }
    }

    pub fn set_content(&mut self, markdown: &str) {
        while self.layout.get_child(0).is_some() {
            self.layout.remove_child(0);
        }
        let parsed = parse_markdown(markdown);
        for view in parsed {
            self.layout.add_child(view);
        }
    }

    pub fn append(&mut self, markdown: &str) {
        let parsed = parse_markdown(markdown);
        for view in parsed {
            self.layout.add_child(view);
        }
    }

    pub fn view(&self) -> &LinearLayout {
        &self.layout
    }

    pub fn view_mut(&mut self) -> &mut LinearLayout {
        &mut self.layout
    }
}

impl Default for MarkdownView {
    fn default() -> Self {
        Self::new()
    }
}

impl View for MarkdownView {
    cursive::wrap_impl!(self.layout: LinearLayout);
}

fn parse_markdown(markdown: &str) -> Vec<Box<dyn View>> {
    let mut views = Vec::new();
    let parser = Parser::new_ext(markdown, Options::all());

    let mut current_text = String::new();
    let mut in_code_block = false;
    let mut _code_block_lang = String::new();
    let mut in_blockquote = false;
    let mut heading_level = 0;

    for event in parser {
        match event {
            Event::Start(tag) => {
                if !current_text.is_empty() {
                    views.push(create_text_view(
                        &current_text,
                        in_blockquote,
                        heading_level,
                    ));
                    current_text.clear();
                }

                match tag {
                    Tag::Heading { level, .. } => {
                        heading_level = level as usize;
                    }
                    Tag::CodeBlock(_) => {
                        in_code_block = true;
                    }
                    Tag::BlockQuote(_) => {
                        in_blockquote = true;
                    }
                    Tag::List(_) => {}
                    Tag::Item => {
                        views.push(create_list_item_view());
                    }
                    _ => {}
                }
            }
            Event::End(tag_end) => match tag_end {
                TagEnd::Heading(_) => {
                    heading_level = 0;
                }
                TagEnd::CodeBlock => {
                    in_code_block = false;
                    _code_block_lang.clear();
                }
                TagEnd::BlockQuote => {
                    in_blockquote = false;
                }
                _ => {}
            },
            Event::Text(text) => {
                current_text.push_str(&text);
            }
            Event::Code(code) => {
                if in_code_block {
                } else {
                    views.push(create_inline_code_view(&code));
                }
            }
            Event::Html(_) => {}
            Event::FootnoteReference(_) => {}
            Event::SoftBreak => {
                current_text.push('\n');
            }
            Event::HardBreak => {
                views.push(create_text_view(
                    &current_text,
                    in_blockquote,
                    heading_level,
                ));
                current_text.clear();
            }
            Event::Rule => {
                views.push(create_horizontal_rule());
            }
        }
    }

    if !current_text.is_empty() {
        views.push(create_text_view(
            &current_text,
            in_blockquote,
            heading_level,
        ));
    }

    views
}

fn create_text_view(text: &str, in_blockquote: bool, heading_level: usize) -> Box<dyn View> {
    let color = if heading_level > 0 {
        cursive::style::Color::Rgb(100, 180, 255)
    } else if in_blockquote {
        cursive::style::Color::Rgb(120, 120, 120)
    } else {
        cursive::style::Color::TerminalDefault
    };

    let mut text_view = TextView::new(text);
    text_view.set_color(cursive::style::ColorStyle::front(color));
    if heading_level > 0 {
        text_view.set_style(cursive::style::Effect::Bold);
    }
    Box::new(text_view)
}

fn create_inline_code_view(code: &str) -> Box<dyn View> {
    let mut text_view = TextView::new(format!("`{}`", code));
    text_view.set_color(cursive::style::ColorStyle::front(
        cursive::style::Color::TerminalDefault,
    ));
    text_view.set_background(cursive::style::Color::Rgb(40, 40, 40));
    Box::new(text_view)
}

fn create_list_item_view() -> Box<dyn View> {
    let mut layout = LinearLayout::horizontal();
    let mut bullet = TextView::new("• ");
    bullet.set_color(cursive::style::ColorStyle::front(
        cursive::style::Color::Rgb(120, 120, 120),
    ));
    layout.add_child(bullet);
    Box::new(layout)
}

fn create_horizontal_rule() -> Box<dyn View> {
    let mut text_view = TextView::new("────────────────────────────────────────");
    text_view.set_color(cursive::style::ColorStyle::front(
        cursive::style::Color::Rgb(120, 120, 120),
    ));
    Box::new(text_view)
}

pub fn render_markdown(markdown: &str) -> Vec<Box<dyn View>> {
    parse_markdown(markdown)
}

pub fn markdown_to_styled_text(markdown: &str) -> Vec<(String, String)> {
    let mut result = Vec::new();
    let parser = Parser::new_ext(markdown, Options::all());

    let mut current_text = String::new();
    let mut current_style = "text".to_string();
    let mut in_code_block = false;
    let mut heading_level = 0;

    for event in parser {
        match event {
            Event::Start(tag) => {
                if !current_text.is_empty() {
                    result.push((current_style.clone(), current_text.clone()));
                    current_text.clear();
                }

                match tag {
                    Tag::Heading { level, .. } => {
                        heading_level = level as usize;
                        current_style = match level {
                            HeadingLevel::H1 => "markdown_heading_h1".to_string(),
                            HeadingLevel::H2 => "markdown_heading_h2".to_string(),
                            HeadingLevel::H3 => "markdown_heading_h3".to_string(),
                            _ => "markdown_heading".to_string(),
                        };
                    }
                    Tag::CodeBlock(_) => {
                        in_code_block = true;
                        current_style = "markdown_code_block".to_string();
                    }
                    Tag::BlockQuote(_) => {
                        current_style = "markdown_blockquote".to_string();
                    }
                    Tag::Emphasis => {
                        current_style = "markdown_italic".to_string();
                    }
                    Tag::Strong => {
                        current_style = "markdown_bold".to_string();
                    }
                    Tag::Link { .. } => {
                        current_style = "markdown_link".to_string();
                    }
                    _ => {}
                }
            }
            Event::End(tag_end) => match tag_end {
                TagEnd::Heading(_) => {
                    heading_level = 0;
                    current_style = "text".to_string();
                }
                TagEnd::CodeBlock => {
                    in_code_block = false;
                    current_style = "text".to_string();
                }
                TagEnd::BlockQuote => {
                    current_style = "text".to_string();
                }
                TagEnd::Emphasis | TagEnd::Strong | TagEnd::Link => {
                    current_style = if heading_level > 0 {
                        "markdown_heading".to_string()
                    } else {
                        "text".to_string()
                    };
                }
                _ => {}
            },
            Event::Text(text) => {
                current_text.push_str(&text);
            }
            Event::Code(code) => {
                if in_code_block {
                    current_text.push_str(&code);
                } else {
                    if !current_text.is_empty() {
                        result.push((current_style.clone(), current_text.clone()));
                        current_text.clear();
                    }
                    result.push(("markdown_code".to_string(), format!("`{}`", code)));
                }
            }
            Event::SoftBreak => {
                current_text.push('\n');
            }
            Event::HardBreak => {
                if !current_text.is_empty() {
                    result.push((current_style.clone(), current_text.clone()));
                    current_text.clear();
                }
                result.push((current_style.clone(), "\n".to_string()));
            }
            Event::Rule => {
                if !current_text.is_empty() {
                    result.push((current_style.clone(), current_text.clone()));
                    current_text.clear();
                }
                result.push((
                    "muted".to_string(),
                    "────────────────────────────────────────\n".to_string(),
                ));
            }
            _ => {}
        }
    }

    if !current_text.is_empty() {
        result.push((current_style, current_text));
    }

    result
}
