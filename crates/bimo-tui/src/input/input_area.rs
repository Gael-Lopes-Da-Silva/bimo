use cursive::Cursive;
use cursive::event::{Event, EventResult, Key};
use cursive::view::{Nameable, Resizable, SizeConstraint};
use cursive::views::{DummyView, LinearLayout, NamedView, OnEventView, ResizedView, TextArea};

use crate::app::AppState;
use crate::output;

/// Base height of the input area: 1 padding line, 1 content line, 1 padding line.
pub const INPUT_MIN_HEIGHT: usize = 3;
/// Maximum height: 15 content lines plus the two padding lines.
pub const INPUT_MAX_HEIGHT: usize = 17;

/// Builds the expandable multi-line input area.
///
/// * `Enter` submits the prompt.
/// * `Ctrl+J` inserts a newline and grows the box by one line (up to
///   `INPUT_MAX_HEIGHT`).
///
/// The `ResizedView` is named `"input_area"` and the inner text area is named
/// `"input"` so both can be mutated from callbacks.
pub fn create_input_area() -> NamedView<ResizedView<LinearLayout>> {
    let text_area = OnEventView::new(TextArea::new())
        .on_pre_event(Event::Key(Key::Enter), |siv| submit(siv))
        .on_pre_event_inner(Event::CtrlChar('j'), |textarea, _event| {
            let mut content = textarea.get_content().to_string();
            content.push('\n');
            textarea.set_content(content);
            textarea.set_cursor(textarea.get_content().len());
            Some(EventResult::Consumed(None))
        })
        .on_pre_event(Event::CtrlChar('j'), |siv| grow(siv))
        .with_name("input");

    let content = LinearLayout::vertical()
        .child(DummyView::new().max_height(1))
        .child(text_area)
        .child(DummyView::new().max_height(1));

    ResizedView::with_fixed_height(INPUT_MIN_HEIGHT, content).with_name("input_area")
}

fn submit(siv: &mut Cursive) {
    let content = siv
        .call_on_name("input", |input: &mut OnEventView<TextArea>| {
            input.get_inner().get_content().to_string()
        })
        .unwrap_or_default();
    let content = content.trim().to_string();
    if content.is_empty() {
        return;
    }

    let (colors, sent) = {
        let Some(state) = siv.user_data::<AppState>() else {
            return;
        };
        state.current_assistant = None;
        state.current_reasoning = None;
        state.current_tool = None;
        (
            state.colors.clone(),
            state.prompt_tx.send(content.clone()).is_ok(),
        )
    };
    if !sent {
        return;
    }

    let view = output::message_view::user_message(&content, &colors);
    output::scroll::add_child(siv, view);

    clear_input(siv);
    reset_height(siv);
}

fn grow(siv: &mut Cursive) {
    let height = {
        let Some(state) = siv.user_data::<AppState>() else {
            return;
        };
        state.input_height = (state.input_height + 1).min(INPUT_MAX_HEIGHT);
        state.input_height
    };
    set_height(siv, height);
}

fn reset_height(siv: &mut Cursive) {
    if let Some(state) = siv.user_data::<AppState>() {
        state.input_height = INPUT_MIN_HEIGHT;
    }
    set_height(siv, INPUT_MIN_HEIGHT);
}

fn clear_input(siv: &mut Cursive) {
    siv.call_on_name("input", |input: &mut OnEventView<TextArea>| {
        input.get_inner_mut().set_content("");
    });
}

fn set_height(siv: &mut Cursive, height: usize) {
    siv.call_on_name("input_area", |area: &mut ResizedView<LinearLayout>| {
        area.set_height(SizeConstraint::Fixed(height));
    });
}
