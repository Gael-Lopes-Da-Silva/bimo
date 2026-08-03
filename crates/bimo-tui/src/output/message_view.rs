use crate::output::markdown::MarkdownView;
use cursive::View;
use cursive::views::{LinearLayout, TextView};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MessageType {
    User,
    Assistant,
    ToolSuccess {
        name: String,
    },
    ToolError {
        name: String,
    },
    ToolCall {
        name: String,
        command: Option<String>,
    },
    System,
}

impl MessageType {
    pub fn title(&self) -> Option<String> {
        match self {
            MessageType::ToolSuccess { name } => Some(format!("✓ {}", name)),
            MessageType::ToolError { name } => Some(format!("✗ {}", name)),
            MessageType::ToolCall { name, .. } => Some(name.clone()),
            _ => None,
        }
    }

    fn text_style(&self) -> (cursive::style::ColorStyle, Option<cursive::style::Effect>) {
        match self {
            MessageType::User => (
                cursive::style::ColorStyle::front(cursive::style::Color::TerminalDefault),
                Some(cursive::style::Effect::Bold),
            ),
            MessageType::Assistant => (
                cursive::style::ColorStyle::front(cursive::style::Color::TerminalDefault),
                None,
            ),
            MessageType::ToolSuccess { .. } => (
                cursive::style::ColorStyle::front(cursive::style::Color::TerminalDefault)
                    .background(cursive::style::Color::Rgb(80, 200, 120)),
                Some(cursive::style::Effect::Bold),
            ),
            MessageType::ToolError { .. } => (
                cursive::style::ColorStyle::front(cursive::style::Color::TerminalDefault)
                    .background(cursive::style::Color::Rgb(255, 85, 85)),
                Some(cursive::style::Effect::Bold),
            ),
            MessageType::ToolCall { .. } => (
                cursive::style::ColorStyle::front(cursive::style::Color::TerminalDefault)
                    .background(cursive::style::Color::Rgb(40, 40, 40)),
                Some(cursive::style::Effect::Bold),
            ),
            MessageType::System => (
                cursive::style::ColorStyle::front(cursive::style::Color::Rgb(120, 120, 120)),
                None,
            ),
        }
    }
}

pub struct MessageView {
    layout: LinearLayout,
    message_type: MessageType,
    content: String,
}

impl MessageView {
    pub fn new(message_type: MessageType, content: impl Into<String>) -> Self {
        let content = content.into();
        let mut layout = LinearLayout::vertical();

        if let Some(title) = message_type.title() {
            let mut title_view = TextView::new(title);
            title_view.set_color(cursive::style::ColorStyle::front(
                cursive::style::Color::TerminalDefault,
            ));
            title_view.set_background(cursive::style::Color::Rgb(40, 40, 40));
            title_view.set_style(cursive::style::Effect::Bold);
            layout.add_child(title_view);
        }

        match message_type {
            MessageType::Assistant | MessageType::System => {
                let mut markdown = MarkdownView::new();
                markdown.set_content(&content);
                layout.add_child(markdown);
            }
            MessageType::ToolCall { command, .. } => {
                if let Some(cmd) = command {
                    let mut cmd_view = TextView::new(format!("$ {}", cmd));
                    cmd_view.set_color(cursive::style::ColorStyle::front(
                        cursive::style::Color::Rgb(170, 170, 170),
                    ));
                    cmd_view.set_background(cursive::style::Color::Rgb(40, 40, 40));
                    layout.add_child(cmd_view);
                }
                let mut content_view = TextView::new(content);
                let (color, effect) = message_type.text_style();
                content_view.set_color(color);
                if let Some(effect) = effect {
                    content_view.set_style(effect);
                }
                layout.add_child(content_view);
            }
            _ => {
                let mut content_view = TextView::new(content);
                let (color, effect) = message_type.text_style();
                content_view.set_color(color);
                if let Some(effect) = effect {
                    content_view.set_style(effect);
                }
                layout.add_child(content_view);
            }
        }

        Self {
            layout,
            message_type,
            content,
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self::new(MessageType::User, content)
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self::new(MessageType::Assistant, content)
    }

    pub fn tool_success(name: impl Into<String>, content: impl Into<String>) -> Self {
        Self::new(MessageType::ToolSuccess { name: name.into() }, content)
    }

    pub fn tool_error(name: impl Into<String>, content: impl Into<String>) -> Self {
        Self::new(MessageType::ToolError { name: name.into() }, content)
    }

    pub fn tool_call(
        name: impl Into<String>,
        command: Option<String>,
        content: impl Into<String>,
    ) -> Self {
        Self::new(
            MessageType::ToolCall {
                name: name.into(),
                command,
            },
            content,
        )
    }

    pub fn system(content: impl Into<String>) -> Self {
        Self::new(MessageType::System, content)
    }

    pub fn append_content(&mut self, delta: &str) {
        self.content.push_str(delta);
        if let MessageType::Assistant = self.message_type {
            if let Some(markdown) = self
                .layout
                .get_child_mut(0)
                .and_then(|v| v.as_any_mut().downcast_mut::<MarkdownView>())
            {
                markdown.append(delta);
            }
        } else if let Some(text) = self
            .layout
            .get_child_mut(self.layout.len().saturating_sub(1))
            .and_then(|v| v.as_any_mut().downcast_mut::<TextView>())
        {
            text.set_content(&self.content);
        }
    }

    pub fn view(&self) -> &LinearLayout {
        &self.layout
    }

    pub fn message_type(&self) -> &MessageType {
        &self.message_type
    }
}

impl View for MessageView {
    cursive::wrap_impl!(self.layout: LinearLayout);
}

impl Default for MessageView {
    fn default() -> Self {
        Self::new(MessageType::System, "")
    }
}

pub struct ToolCallView {
    layout: LinearLayout,
    name: String,
    command: Option<String>,
    output: String,
    success: bool,
}

impl ToolCallView {
    pub fn new(name: impl Into<String>, command: Option<String>) -> Self {
        let name = name.into();
        let mut layout = LinearLayout::vertical();

        let mut title = TextView::new(format!("⟳ {}", name));
        title.set_color(cursive::style::ColorStyle::front(
            cursive::style::Color::TerminalDefault,
        ));
        title.set_background(cursive::style::Color::Rgb(40, 40, 40));
        title.set_style(cursive::style::Effect::Bold);
        layout.add_child(title);

        if let Some(cmd) = &command {
            let mut cmd_view = TextView::new(format!("$ {}", cmd));
            cmd_view.set_color(cursive::style::ColorStyle::front(
                cursive::style::Color::Rgb(170, 170, 170),
            ));
            cmd_view.set_background(cursive::style::Color::Rgb(40, 40, 40));
            layout.add_child(cmd_view);
        }

        Self {
            layout,
            name,
            command,
            output: String::new(),
            success: false,
        }
    }

    pub fn update(&mut self, output: impl Into<String>, success: bool) {
        self.output = output.into();
        self.success = success;

        while self.layout.get_child(0).is_some() {
            self.layout.remove_child(0);
        }

        let title_text = if success {
            format!("✓ {}", self.name)
        } else {
            format!("✗ {}", self.name)
        };
        let mut title = TextView::new(title_text);
        title.set_color(cursive::style::ColorStyle::front(
            cursive::style::Color::TerminalDefault,
        ));
        title.set_background(if success {
            cursive::style::Color::Rgb(80, 200, 120)
        } else {
            cursive::style::Color::Rgb(255, 85, 85)
        });
        title.set_style(cursive::style::Effect::Bold);
        self.layout.add_child(title);

        if let Some(cmd) = &self.command {
            let mut cmd_view = TextView::new(format!("$ {}", cmd));
            cmd_view.set_color(cursive::style::ColorStyle::front(
                cursive::style::Color::Rgb(170, 170, 170),
            ));
            cmd_view.set_background(cursive::style::Color::Rgb(40, 40, 40));
            self.layout.add_child(cmd_view);
        }

        let mut output_view = TextView::new(&self.output);
        output_view.set_color(cursive::style::ColorStyle::front(
            cursive::style::Color::TerminalDefault,
        ));
        output_view.set_background(if success {
            cursive::style::Color::Rgb(80, 200, 120)
        } else {
            cursive::style::Color::Rgb(255, 85, 85)
        });
        self.layout.add_child(output_view);
    }

    pub fn append_output(&mut self, delta: &str) {
        self.output.push_str(delta);
        if let Some(text) = self
            .layout
            .get_child_mut(self.layout.len().saturating_sub(1))
            .and_then(|v| v.as_any_mut().downcast_mut::<TextView>())
        {
            text.set_content(&self.output);
        }
    }

    pub fn view(&self) -> &LinearLayout {
        &self.layout
    }
}

impl View for ToolCallView {
    cursive::wrap_impl!(self.layout: LinearLayout);
}
