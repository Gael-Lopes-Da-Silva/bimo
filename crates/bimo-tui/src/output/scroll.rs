use crate::output::message_view::MessageView;
use cursive::View;
use cursive::views::{LinearLayout, ScrollView};

pub struct ScrollableOutput {
    scroll: ScrollView<LinearLayout>,
    messages: LinearLayout,
    auto_scroll: bool,
    user_scrolled: bool,
}

impl ScrollableOutput {
    pub fn new() -> Self {
        let messages = LinearLayout::vertical();
        let scroll = ScrollView::new(messages.clone());

        Self {
            scroll,
            messages,
            auto_scroll: true,
            user_scrolled: false,
        }
    }

    pub fn add_message(&mut self, view: MessageView) {
        self.messages.add_child(view);
        self.messages
            .add_child(cursive::views::DummyView::new().max_height(1));

        if self.auto_scroll && !self.user_scrolled {
            self.scroll_to_bottom();
        }
    }

    pub fn append_to_last_message(&mut self, delta: &str) {
        if let Some(last) = self
            .messages
            .get_child_mut(self.messages.len().saturating_sub(2))
        {
            if let Some(msg) = last.as_any_mut().downcast_mut::<MessageView>() {
                msg.append_content(delta);
            }
        }

        if self.auto_scroll && !self.user_scrolled {
            self.scroll_to_bottom();
        }
    }

    pub fn update_last_tool_call(&mut self, output: &str, _success: bool) {
        if let Some(last) = self
            .messages
            .get_child_mut(self.messages.len().saturating_sub(2))
        {
            if let Some(msg) = last.as_any_mut().downcast_mut::<MessageView>() {
                if let crate::output::message_view::MessageType::ToolCall { .. } =
                    msg.message_type()
                {
                    msg.append_content(output);
                }
            }
        }

        if self.auto_scroll && !self.user_scrolled {
            self.scroll_to_bottom();
        }
    }

    pub fn scroll_to_bottom(&mut self) {
        self.scroll.scroll_to_bottom();
    }

    pub fn scroll_to_top(&mut self) {
        self.scroll.scroll_to_top();
    }

    pub fn scroll_up(&mut self, lines: usize) {
        self.user_scrolled = true;
        for _ in 0..lines {
            self.scroll.scroll_up(1);
        }
    }

    pub fn scroll_down(&mut self, lines: usize) {
        for _ in 0..lines {
            self.scroll.scroll_down(1);
        }
        if self.scroll.is_at_bottom() {
            self.user_scrolled = false;
        }
    }

    pub fn page_up(&mut self) {
        self.user_scrolled = true;
        self.scroll.scroll_page_up();
    }

    pub fn page_down(&mut self) {
        self.scroll.scroll_page_down();
        if self.scroll.is_at_bottom() {
            self.user_scrolled = false;
        }
    }

    pub fn set_auto_scroll(&mut self, enabled: bool) {
        self.auto_scroll = enabled;
        if enabled {
            self.user_scrolled = false;
        }
    }

    pub fn clear(&mut self) {
        while self.messages.get_child(0).is_some() {
            self.messages.remove_child(0);
        }
    }

    pub fn view(&self) -> &ScrollView<LinearLayout> {
        &self.scroll
    }

    pub fn view_mut(&mut self) -> &mut ScrollView<LinearLayout> {
        &mut self.scroll
    }

    pub fn is_at_bottom(&self) -> bool {
        self.scroll.is_at_bottom()
    }

    pub fn message_count(&self) -> usize {
        self.messages.len() / 2
    }
}

impl Default for ScrollableOutput {
    fn default() -> Self {
        Self::new()
    }
}

impl View for ScrollableOutput {
    cursive::wrap_impl!(self.scroll: ScrollView<LinearLayout>);
}
