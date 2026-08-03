use bimo_core::AgentEvent;
use cursive::CbSink;
use cursive::Cursive;
use cursive::view::Nameable;
use cursive::views::{DummyView, LinearLayout, Panel, TextView};
use tokio::sync::broadcast;

pub struct EventBridge {
    cb_sink: CbSink,
    rx: broadcast::Receiver<AgentEvent>,
}

impl EventBridge {
    pub fn new(cb_sink: CbSink, rx: broadcast::Receiver<AgentEvent>) -> Self {
        Self { cb_sink, rx }
    }

    pub fn spawn(self) {
        tokio::spawn(async move {
            let mut rx = self.rx;
            while let Ok(event) = rx.recv().await {
                let cb_sink = self.cb_sink.clone();
                cb_sink
                    .send(Box::new(move |siv: &mut Cursive| {
                        handle_agent_event(siv, event);
                    }))
                    .ok();
            }
        });
    }
}

pub fn create_event_bridge(siv: &mut Cursive, rx: broadcast::Receiver<AgentEvent>) -> EventBridge {
    let cb_sink = siv.cb_sink().clone();
    EventBridge::new(cb_sink, rx)
}

pub fn handle_agent_event(siv: &mut Cursive, event: AgentEvent) {
    match event {
        AgentEvent::TextDelta(delta) => {
            let found = siv.call_on_name("current_assistant", |panel: &mut Panel<TextView>| {
                panel.get_inner_mut().append(delta.clone());
            });
            if found.is_none() {
                siv.call_on_name("messages", |messages: &mut LinearLayout| {
                    let panel = Panel::new(TextView::new(delta)).title("Assistant");
                    messages.add_child(panel.with_name("current_assistant"));
                    messages.add_child(DummyView);
                });
            }
        }
        AgentEvent::ReasoningDelta(delta) => {
            let found = siv.call_on_name("current_reasoning", |panel: &mut Panel<TextView>| {
                panel.get_inner_mut().append(delta.clone());
            });
            if found.is_none() {
                siv.call_on_name("messages", |messages: &mut LinearLayout| {
                    let panel = Panel::new(TextView::new(delta)).title("Reasoning");
                    messages.add_child(panel.with_name("current_reasoning"));
                    messages.add_child(DummyView);
                });
            }
        }
        AgentEvent::ToolCallStart { tool_name, args } => {
            let desc = format!("Running: {}{}", tool_name, describe_args(&args));
            let found = siv.call_on_name("current_tool", |panel: &mut Panel<TextView>| {
                panel.set_title(tool_name.clone());
                panel.get_inner_mut().set_content(desc.clone());
            });
            if found.is_none() {
                siv.call_on_name("messages", |messages: &mut LinearLayout| {
                    let panel = Panel::new(TextView::new(desc)).title(tool_name);
                    messages.add_child(panel.with_name("current_tool"));
                    messages.add_child(DummyView);
                });
            }
        }
        AgentEvent::ToolCallEnd { tool_name, result } => {
            let (output, success) = match result {
                Ok(output) => (output, true),
                Err(error) => (error, false),
            };
            let title = if success {
                format!("OK {}", tool_name)
            } else {
                format!("FAILED {}", tool_name)
            };
            siv.call_on_name("current_tool", |panel: &mut Panel<TextView>| {
                panel.set_title(title);
                panel.get_inner_mut().set_content(output);
            });
        }
        AgentEvent::Steering(guidance) => {
            siv.call_on_name("messages", |messages: &mut LinearLayout| {
                let panel =
                    Panel::new(TextView::new(format!("[steering] {}", guidance))).title("Steering");
                messages.add_child(panel);
                messages.add_child(DummyView);
            });
        }
        AgentEvent::Retrying { attempt, error } => {
            siv.call_on_name("messages", |messages: &mut LinearLayout| {
                let panel = Panel::new(TextView::new(format!(
                    "Retrying (attempt {}): {}",
                    attempt, error
                )))
                .title("Retry");
                messages.add_child(panel);
                messages.add_child(DummyView);
            });
        }
        AgentEvent::Error(error) => {
            siv.call_on_name("messages", |messages: &mut LinearLayout| {
                let panel = Panel::new(TextView::new(format!("Error: {}", error))).title("Error");
                messages.add_child(panel);
                messages.add_child(DummyView);
            });
        }
        AgentEvent::Done => {
            siv.call_on_name("messages", |messages: &mut LinearLayout| {
                messages.add_child(TextView::new("-- run complete --"));
                messages.add_child(DummyView);
            });
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_describe_args_string() {
        assert_eq!(
            describe_args(&json!({ "command": "cargo build" })),
            " (command=cargo build)"
        );
    }

    #[test]
    fn test_describe_args_number_and_mixed() {
        assert_eq!(describe_args(&json!({ "a": 1, "b": "x" })), " (a=1, b=x)");
    }

    #[test]
    fn test_describe_args_empty() {
        assert_eq!(describe_args(&json!({})), "");
        assert_eq!(describe_args(&json!([1, 2, 3])), "");
    }
}
