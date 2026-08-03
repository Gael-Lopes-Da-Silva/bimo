use cursive::views::LinearLayout;

use crate::input::input_area::create_input_area;
use crate::output::scroll::create_output_area;

/// Builds the main layout: a scrollable output area on top and a fixed
/// multi-line input area at the bottom.
pub fn create_main_layout() -> LinearLayout {
    LinearLayout::vertical()
        .child(create_output_area())
        .child(create_input_area())
}
