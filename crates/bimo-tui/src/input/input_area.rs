use cursive::event::{Event, Key};
use cursive::view::{Nameable, Resizable};
use cursive::views::{EditView, LinearLayout, ResizedView};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputAreaEvent {
    Submit(String),
    NewLine,
    Expand,
    Contract,
    AutocompleteTrigger(char),
    Cancel,
}

pub struct InputArea {
    view: ResizedView<LinearLayout>,
    max_height: usize,
    min_height: usize,
    on_event: Option<Box<dyn Fn(InputAreaEvent) + Send + Sync>>,
}

impl InputArea {
    pub fn new() -> Self {
        let edit_view = EditView::new().min_height(1).max_height(15);

        let content = LinearLayout::vertical()
            .child(cursive::views::DummyView::new().max_height(1))
            .child(edit_view.with_name("input"))
            .child(cursive::views::DummyView::new().max_height(1));

        let view = ResizedView::with_fixed_height(3, content);

        Self {
            view,
            max_height: 15,
            min_height: 3,
            on_event: None,
        }
    }

    pub fn set_on_event<F>(&mut self, f: F)
    where
        F: Fn(InputAreaEvent) + Send + Sync + 'static,
    {
        self.on_event = Some(Box::new(f));
    }

    pub fn get_content(&self) -> String {
        self.view
            .get_inner()
            .get_child(1)
            .and_then(|v| v.as_any().downcast_ref::<EditView>())
            .map(|e| e.get_content().to_string())
            .unwrap_or_default()
    }

    pub fn set_content(&mut self, content: &str) {
        if let Some(edit) = self
            .view
            .get_inner_mut()
            .get_child_mut(1)
            .and_then(|v| v.as_any_mut().downcast_mut::<EditView>())
        {
            edit.set_content(content);
        }
    }

    pub fn clear(&mut self) {
        self.set_content("");
        self.reset_height();
    }

    pub fn height(&self) -> usize {
        self.view.get_height()
    }

    pub fn set_height(&mut self, height: usize) {
        let height = height.clamp(self.min_height, self.max_height + 2);
        self.view
            .set_height(cursive::view::SizeConstraint::Fixed(height));
    }

    pub fn reset_height(&mut self) {
        self.view
            .set_height(cursive::view::SizeConstraint::Fixed(self.min_height));
    }

    pub fn expand(&mut self) {
        let current = self.height();
        if current < self.max_height + 2 {
            self.set_height(current + 1);
        }
    }

    pub fn handle_event(&mut self, event: &Event) -> bool {
        match event {
            Event::Key(Key::Enter) => {
                let content = self.get_content();
                if !content.trim().is_empty() {
                    if let Some(ref cb) = self.on_event {
                        cb(InputAreaEvent::Submit(content));
                    }
                    self.clear();
                }
                true
            }
            Event::CtrlChar('j') => {
                self.expand();
                if let Some(ref cb) = self.on_event {
                    cb(InputAreaEvent::NewLine);
                }
                true
            }
            Event::Key(Key::Esc) => {
                if let Some(ref cb) = self.on_event {
                    cb(InputAreaEvent::Cancel);
                }
                true
            }
            _ => false,
        }
    }

    pub fn view(&self) -> &ResizedView<LinearLayout> {
        &self.view
    }

    pub fn view_mut(&mut self) -> &mut ResizedView<LinearLayout> {
        &mut self.view
    }
}

impl Default for InputArea {
    fn default() -> Self {
        Self::new()
    }
}

pub fn create_input_area<F>(on_submit: F) -> InputArea
where
    F: Fn(String) + Send + Sync + 'static,
{
    let mut area = InputArea::new();
    area.set_on_event(move |event| {
        if let InputAreaEvent::Submit(content) = event {
            on_submit(content);
        }
    });
    area
}
