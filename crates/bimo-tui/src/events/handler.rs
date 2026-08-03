use bimo_core::AgentEvent;
use cursive::Cursive;
use cursive::style::{ColorStyle, Style};
use cursive::utils::markup::StyledString;
use cursive::view::Nameable;
use cursive::views::TextView;

use crate::app::AppState;
use crate::output;
use crate::theme::ThemeColors;

/// Maps agent events to UI updates.
pub fn handle_agent_event(siv: &mut Cursive, event: AgentEvent) {
    match event {
        AgentEvent::TextDelta(delta) => stream_text(siv, &delta),
        AgentEvent::ReasoningDelta(delta) => stream_reasoning(siv, &delta),
        AgentEvent::ToolCallStart { tool_name, args } => start_tool_call(siv, &tool_name, &args),
        AgentEvent::ToolCallEnd { tool_name, result } => {
            let (output, success) = match result {
                Ok(output) => (output, true),
                Err(error) => (error, false),
            };
            finish_tool_call(siv, &tool_name, &output, success);
        }
        AgentEvent::Steering(guidance) => {
            let message = format!("[steering] {guidance}");
            let view = output::message_view::user_message(&message, &colors(siv));
            output::scroll::add_child(siv, view);
        }
        AgentEvent::Retrying { attempt, error } => {
            let message = format!("Retrying (attempt {attempt}): {error}");
            let view = output::message_view::system_message(&message, &colors(siv));
            output::scroll::add_child(siv, view);
        }
        AgentEvent::Error(error) => {
            let view = output::message_view::error_message(&error, &colors(siv));
            output::scroll::add_child(siv, view);
        }
        AgentEvent::Done => {}
    }
}

/// Appends a text delta to the current assistant message, creating a new
/// (markdown-rendered) message view if there is none yet.
fn stream_text(siv: &mut Cursive, delta: &str) {
    if let Some((name, is_new)) = current_or_new(siv, "assistant", |_id, colors| {
        TextView::new(output::markdown::render_markdown(delta, colors))
    }) {
        if !is_new {
            siv.call_on_name(&name, |view: &mut TextView| {
                view.append(delta);
            });
        }
    }
}

/// Appends a reasoning delta to the current reasoning message, in a dimmed
/// style.
fn stream_reasoning(siv: &mut Cursive, delta: &str) {
    if let Some((name, is_new)) = current_or_new(siv, "reasoning", |_id, colors| {
        TextView::new(StyledString::styled(
            format!("[think] {delta}"),
            Style::from(ColorStyle::front(colors.muted)),
        ))
    }) {
        if !is_new {
            let muted = siv
                .user_data::<AppState>()
                .map(|state| state.colors.muted)
                .unwrap_or_default();
            siv.call_on_name(&name, |view: &mut TextView| {
                view.append(StyledString::styled(
                    delta,
                    Style::from(ColorStyle::front(muted)),
                ));
            });
        }
    }
}

fn start_tool_call(siv: &mut Cursive, tool_name: &str, args: &serde_json::Value) {
    let command = match (tool_name, args) {
        ("run_command", args) => args
            .get("command")
            .and_then(|value| value.as_str())
            .map(String::from),
        (_, args) if args.as_object().is_some_and(|map| !map.is_empty()) => {
            Some(describe_args(args).trim().to_string())
        }
        _ => None,
    };

    let (id, colors) = {
        let Some(state) = siv.user_data::<AppState>() else {
            return;
        };
        let id = state.next_id;
        state.next_id += 1;
        state.current_tool = Some(id);
        (id, state.colors.clone())
    };

    let view = output::message_view::tool_call_box(id, tool_name, command.as_deref(), &colors);
    output::scroll::add_child(siv, view);
}

fn finish_tool_call(siv: &mut Cursive, _tool_name: &str, output: &str, success: bool) {
    let (id, colors) = {
        let Some(state) = siv.user_data::<AppState>() else {
            return;
        };
        let Some(id) = state.current_tool else {
            return;
        };
        (id, state.colors.clone())
    };
    output::message_view::update_tool_box(siv, id, output, success, &colors);
}

/// Returns the name of the streaming message of the given kind (`"assistant"`
/// or `"reasoning"`), creating a new one if needed.
///
/// The second element of the tuple is `true` when the view was just created.
fn current_or_new(
    siv: &mut Cursive,
    kind: &str,
    make: impl FnOnce(usize, &ThemeColors) -> TextView,
) -> Option<(String, bool)> {
    let existing = {
        let state = siv.user_data::<AppState>()?;
        match kind {
            "assistant" => state.current_assistant.clone(),
            "reasoning" => state.current_reasoning.clone(),
            _ => None,
        }
    };

    if let Some(name) = existing
        && siv.call_on_name(&name, |_: &mut TextView| {}).is_some()
    {
        return Some((name, false));
    }

    let (id, colors) = {
        let state = siv.user_data::<AppState>()?;
        let id = state.next_id;
        state.next_id += 1;
        (id, state.colors.clone())
    };

    let name = format!("{kind}_{id}");
    {
        let state = siv.user_data::<AppState>()?;
        match kind {
            "assistant" => state.current_assistant = Some(name.clone()),
            "reasoning" => state.current_reasoning = Some(name.clone()),
            _ => {}
        }
    }

    let view = make(id, &colors).with_name(name.clone());
    output::scroll::add_child(siv, view);
    Some((name, true))
}

fn colors(siv: &mut Cursive) -> ThemeColors {
    siv.user_data::<AppState>()
        .map(|state| state.colors.clone())
        .unwrap_or_default()
}

fn describe_args(args: &serde_json::Value) -> String {
    if let serde_json::Value::Object(map) = args
        && !map.is_empty()
    {
        let parts: Vec<String> = map
            .iter()
            .map(|(k, v)| match v {
                serde_json::Value::String(s) => format!("{k}={s}"),
                serde_json::Value::Number(n) => format!("{k}={n}"),
                other => format!("{k}={other}"),
            })
            .collect();
        format!(" ({})", parts.join(", "))
    } else {
        String::new()
    }
}
