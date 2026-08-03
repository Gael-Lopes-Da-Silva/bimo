use cursive::Cursive;
use cursive::style::{Color, ColorStyle, Effect, Style};
use cursive::view::Nameable;
use cursive::views::{LinearLayout, TextView};

use crate::output::markdown::render_markdown;
use crate::theme::ThemeColors;

fn user_style(colors: &ThemeColors) -> Style {
    Style::from(ColorStyle::new(colors.text, colors.surface)).combine(Effect::Bold)
}

fn system_style(colors: &ThemeColors) -> Style {
    Style::from(ColorStyle::front(colors.muted))
}

fn pending_style(colors: &ThemeColors) -> Style {
    Style::from(ColorStyle::new(colors.text_secondary, colors.surface_alt)).combine(Effect::Bold)
}

fn pending_content_style(colors: &ThemeColors) -> Style {
    Style::from(ColorStyle::new(colors.text_secondary, colors.surface_alt))
}

fn result_style(bg: Color, bold: bool) -> Style {
    let style = Style::from(ColorStyle::new(Color::Rgb(15, 15, 15), bg));
    if bold {
        style.combine(Effect::Bold)
    } else {
        style
    }
}

/// Renders a user prompt as a gray box.
pub fn user_message(content: &str, colors: &ThemeColors) -> LinearLayout {
    LinearLayout::vertical().child(TextView::new(content.to_string()).style(user_style(colors)))
}

/// Renders a full assistant message with markdown styling.
pub fn assistant_message(content: &str, colors: &ThemeColors) -> LinearLayout {
    LinearLayout::vertical().child(TextView::new(render_markdown(content, colors)))
}

/// Renders a muted system / status line.
pub fn system_message(content: &str, colors: &ThemeColors) -> LinearLayout {
    LinearLayout::vertical().child(TextView::new(content.to_string()).style(system_style(colors)))
}

/// Renders an error message in the error color.
pub fn error_message(content: &str, colors: &ThemeColors) -> LinearLayout {
    LinearLayout::vertical().child(
        TextView::new(format!("Error: {content}"))
            .style(Style::from(ColorStyle::front(colors.error))),
    )
}

/// Creates a new tool-call box with a pending spinner title.
///
/// The title, command and content TextViews are given predictable names
/// (`tool_{id}_title`, `tool_{id}_command`, `tool_{id}_content`) so they can
/// be updated in place when the tool finishes.
pub fn tool_call_box(
    id: usize,
    tool_name: &str,
    command: Option<&str>,
    colors: &ThemeColors,
) -> LinearLayout {
    let mut layout = LinearLayout::vertical();

    let title = TextView::new(format!("⟳ {tool_name}"))
        .style(pending_style(colors))
        .with_name(format!("tool_{id}_title"));
    layout.add_child(title);

    if let Some(command) = command {
        let cmd = TextView::new(format!("$ {command}"))
            .style(pending_content_style(colors))
            .with_name(format!("tool_{id}_command"));
        layout.add_child(cmd);
    }

    let content = TextView::new("")
        .style(pending_content_style(colors))
        .with_name(format!("tool_{id}_content"));
    layout.add_child(content);

    layout
}

/// Updates the given tool-call box with its result, recoloring it green or
/// red depending on `success`.
pub fn update_tool_box(
    siv: &mut Cursive,
    id: usize,
    output: &str,
    success: bool,
    colors: &ThemeColors,
) {
    let bg = if success {
        colors.success
    } else {
        colors.error
    };
    let prefix = if success { "✓ " } else { "✗ " };

    let title_name = format!("tool_{id}_title");
    siv.call_on_name(&title_name, |title: &mut TextView| {
        let current = title.get_content().source().to_string();
        let name = current.strip_prefix("⟳ ").unwrap_or(&current);
        title.set_content(format!("{prefix}{name}"));
        title.set_style(result_style(bg, true));
    });

    let content_name = format!("tool_{id}_content");
    siv.call_on_name(&content_name, |content_view: &mut TextView| {
        content_view.set_content(output.to_string());
        content_view.set_style(result_style(bg, false));
    });
}
