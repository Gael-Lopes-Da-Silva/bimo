use cursive::Cursive;
use cursive::View;
use cursive::view::{Nameable, Resizable, ScrollStrategy};
use cursive::views::{DummyView, LinearLayout, NamedView, ScrollView};

/// Builds the scrollable output area.
///
/// The inner message list is named `"messages"` and the wrapping
/// `ScrollView` is named `"output_area"`. The view uses the
/// `StickToBottom` strategy so new content is followed automatically until
/// the user scrolls elsewhere.
pub fn create_output_area() -> NamedView<ScrollView<NamedView<LinearLayout>>> {
    let messages = LinearLayout::vertical().with_name("messages");
    ScrollView::new(messages)
        .scroll_strategy(ScrollStrategy::StickToBottom)
        .with_name("output_area")
}

/// Appends a message view to the output, followed by a small spacer.
pub fn add_child(siv: &mut Cursive, view: impl View + 'static) {
    siv.call_on_name("messages", |messages: &mut LinearLayout| {
        messages.add_child(view);
        messages.add_child(DummyView::new().max_height(1));
    });
}

/// Removes every message from the output.
pub fn clear(siv: &mut Cursive) {
    siv.call_on_name("messages", |messages: &mut LinearLayout| {
        while messages.get_child(0).is_some() {
            messages.remove_child(0);
        }
    });
}

/// Scrolls the output by the given number of lines.
///
/// Scrolling up stops following the bottom; scrolling back to the bottom
/// resumes auto-scrolling.
pub fn scroll_by(siv: &mut Cursive, lines: isize) {
    siv.call_on_name(
        "output_area",
        |area: &mut ScrollView<NamedView<LinearLayout>>| {
            area.set_scroll_strategy(resume_strategy(area, lines));
        },
    );
}

pub fn page_by(siv: &mut Cursive, lines: isize) {
    siv.call_on_name(
        "output_area",
        |area: &mut ScrollView<NamedView<LinearLayout>>| {
            area.set_scroll_strategy(resume_strategy(area, lines));
        },
    );
}

pub fn scroll_to(siv: &mut Cursive, top: bool) {
    siv.call_on_name(
        "output_area",
        |area: &mut ScrollView<NamedView<LinearLayout>>| {
            if top {
                area.scroll_to_top();
            } else {
                area.scroll_to_bottom();
            }
            area.set_scroll_strategy(resume_strategy(area, 0));
        },
    );
}

fn resume_strategy(area: &ScrollView<NamedView<LinearLayout>>, lines: isize) -> ScrollStrategy {
    if lines >= 0 && area.is_at_bottom() {
        ScrollStrategy::StickToBottom
    } else {
        ScrollStrategy::KeepRow
    }
}
