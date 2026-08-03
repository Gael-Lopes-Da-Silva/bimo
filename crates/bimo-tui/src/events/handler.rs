use bimo_core::agent::AgentEvent;
use cursive::Cursive;

pub fn handle_agent_event(siv: &mut Cursive, event: AgentEvent) {
    match event {
        AgentEvent::TextDelta(delta) => {
            siv.call_on_name(
                "output_area",
                |output: &mut crate::output::ScrollableOutput| {
                    output.append_to_last_message(&delta);
                },
            );
        }
        AgentEvent::ReasoningDelta(delta) => {
            siv.call_on_name(
                "output_area",
                |output: &mut crate::output::ScrollableOutput| {
                    output.append_to_last_message(&format!("[thinking] {delta}"));
                },
            );
        }
        AgentEvent::ToolCallStart { tool_name, args } => {
            let command = if tool_name == "run_command" {
                args.get("command")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            } else {
                None
            };

            siv.call_on_name(
                "output_area",
                |output: &mut crate::output::ScrollableOutput| {
                    let view =
                        crate::output::message_view::MessageView::tool_call(tool_name, command, "");
                    output.add_message(view);
                },
            );
        }
        AgentEvent::ToolCallEnd {
            tool_name: _tool_name,
            result,
        } => match result {
            Ok(output) => {
                siv.call_on_name(
                    "output_area",
                    |output_area: &mut crate::output::ScrollableOutput| {
                        output_area.update_last_tool_call(&output, true);
                    },
                );
            }
            Err(error) => {
                siv.call_on_name(
                    "output_area",
                    |output_area: &mut crate::output::ScrollableOutput| {
                        output_area.update_last_tool_call(&error, false);
                    },
                );
            }
        },
        AgentEvent::Steering(guidance) => {
            siv.call_on_name(
                "output_area",
                |output: &mut crate::output::ScrollableOutput| {
                    let view = crate::output::message_view::MessageView::user(format!(
                        "[steering] {guidance}"
                    ));
                    output.add_message(view);
                },
            );
        }
        AgentEvent::Retrying { attempt, error } => {
            siv.call_on_name(
                "output_area",
                |output: &mut crate::output::ScrollableOutput| {
                    let view = crate::output::message_view::MessageView::system(format!(
                        "Retrying (attempt {attempt}): {error}"
                    ));
                    output.add_message(view);
                },
            );
        }
        AgentEvent::Error(error) => {
            siv.call_on_name(
                "output_area",
                |output: &mut crate::output::ScrollableOutput| {
                    let view =
                        crate::output::message_view::MessageView::system(format!("Error: {error}"));
                    output.add_message(view);
                },
            );
        }
        AgentEvent::Done => {
            // Agent done, input is ready for next prompt
        }
    }
}

pub fn add_user_message(siv: &mut Cursive, content: &str) {
    siv.call_on_name(
        "output_area",
        |output: &mut crate::output::ScrollableOutput| {
            let view = crate::output::message_view::MessageView::user(content);
            output.add_message(view);
        },
    );
}

pub fn add_assistant_message(siv: &mut Cursive, content: &str) {
    siv.call_on_name(
        "output_area",
        |output: &mut crate::output::ScrollableOutput| {
            let view = crate::output::message_view::MessageView::assistant(content);
            output.add_message(view);
        },
    );
}

pub fn start_tool_call(siv: &mut Cursive, tool_name: &str, command: Option<String>) {
    siv.call_on_name(
        "output_area",
        |output: &mut crate::output::ScrollableOutput| {
            let view = crate::output::message_view::MessageView::tool_call(tool_name, command, "");
            output.add_message(view);
        },
    );
}

pub fn finish_tool_call(siv: &mut Cursive, output: &str, success: bool) {
    siv.call_on_name(
        "output_area",
        |output_area: &mut crate::output::ScrollableOutput| {
            output_area.update_last_tool_call(output, success);
        },
    );
}
