use cursive::Cursive;
use cursive::view::{View, ViewWrapper};
use cursive::views::{LinearLayout, ResizedView, ScrollView};

pub struct MainLayout {
    layout: LinearLayout,
}

impl MainLayout {
    pub fn new() -> Self {
        let output_area = OutputArea::new();
        let input_area = InputAreaWrapper::new();

        let layout = LinearLayout::vertical()
            .child(ResizedView::with_full_screen(output_area.view()))
            .child(input_area.view());

        Self { layout }
    }
}

impl ViewWrapper for MainLayout {
    cursive::wrap_impl!(self.layout: LinearLayout);
}

#[derive(Clone)]
pub struct OutputArea {
    messages: LinearLayout,
}

impl OutputArea {
    pub fn new() -> Self {
        let messages = LinearLayout::vertical();
        Self { messages }
    }

    pub fn add_message(&mut self, view: impl View + 'static) {
        self.messages.add_child(view);
        self.messages
            .add_child(cursive::views::DummyView::new().max_height(1));
    }

    pub fn clear(&mut self) {
        while self.messages.get_child(0).is_some() {
            self.messages.remove_child(0);
        }
    }

    pub fn view(&self) -> ScrollView<LinearLayout> {
        ScrollView::new(self.messages.clone())
            .scroll_strategy(cursive::views::ScrollStrategy::StickToBottom)
    }

    pub fn inner(&self) -> &LinearLayout {
        &self.messages
    }

    pub fn inner_mut(&mut self) -> &mut LinearLayout {
        &mut self.messages
    }
}

impl Default for OutputArea {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
pub struct InputAreaWrapper {
    inner: ResizedView<LinearLayout>,
}

impl InputAreaWrapper {
    pub fn new() -> Self {
        let content = LinearLayout::vertical()
            .child(cursive::views::DummyView::new().max_height(1))
            .child(
                cursive::views::EditView::new()
                    .with_name("input")
                    .min_height(1)
                    .max_height(15),
            )
            .child(cursive::views::DummyView::new().max_height(1));

        let resized = ResizedView::with_fixed_height(3, content);

        Self { inner: resized }
    }

    pub fn set_height(&mut self, height: usize) {
        let height = height.clamp(3, 17);
        self.inner
            .set_height(cursive::view::SizeConstraint::Fixed(height));
    }

    pub fn height(&self) -> usize {
        self.inner.get_height()
    }

    pub fn view(&self) -> ResizedView<LinearLayout> {
        self.inner.clone()
    }
}

impl Default for InputAreaWrapper {
    fn default() -> Self {
        Self::new()
    }
}

pub fn create_main_layout(siv: &mut Cursive) -> MainLayout {
    let layout = MainLayout::new();
    siv.add_layer(layout.clone());
    layout
}
