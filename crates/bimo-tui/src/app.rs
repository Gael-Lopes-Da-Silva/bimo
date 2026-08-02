//! Standalone TUI model for bimo.
//!
//! Owns the widget tree and all interaction state. It is intentionally not
//! wired to `bimo-core` yet; that link will be added later.

use std::any::Any;

use tuie::prelude::*;

const SESSION_NAMES: &[&str] = &[
    "refactor parser",
    "add cli flags",
    "write integration tests",
    "benchmark the agent loop",
];

/// Editor bindings that reject the chords the model wants to handle itself,
/// so they bubble up to [`BimoTui::override_on_input`].
struct AppBindings<T: TextDocument> {
    inner: Box<dyn InputBindings<T>>,
}

impl<T: TextDocument + 'static> AppBindings<T> {
    fn new() -> Self {
        Self {
            inner: DefaultBindings::new(),
        }
    }
}

impl<T: TextDocument + 'static> InputBindings<T> for AppBindings<T> {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn configure_state(&self, state: &mut EditorState<T>) {
        self.inner.configure_state(state);
    }

    fn on_input(
        &mut self,
        state: &mut EditorState<T>,
        text: &mut T,
        queue: &mut InputQueue,
    ) -> InputResult {
        let Some(event) = queue.peek() else {
            return InputResult::Rejected;
        };
        match event.chord {
            chord!(Enter) | chord!(Up) | chord!(Down) => InputResult::Rejected,
            _ => self.inner.on_input(state, text, queue),
        }
    }
}

fn app_bindings() -> Box<dyn InputBindings<Text>> {
    Box::new(AppBindings::<Text>::new())
}

struct SessionListContext {
    sessions: Vec<String>,
    selected: Option<usize>,
}

fn render_session(ctx: &mut SessionListContext, index: usize) -> Option<Box<dyn Widget>> {
    let name = ctx.sessions.get(index)?;
    let active = ctx.selected == Some(index);
    let pointer = if active { ">" } else { " " };
    let mut content = StyledString::from(format!("{pointer} {name}"));
    if active {
        content.style_range(0..content.as_str().len(), |s| {
            *s = s.apply(Style::new().fg(Color::BRIGHT_GREEN).bold());
        });
    }
    Some(Text::new().content(content).ellipsis())
}

/// The bimo TUI model: a session list, a transcript, and a prompt input.
pub struct App {
    root: Box<Pane>,
    sessions_id: WidgetId<List>,
    output_id: WidgetId<Text>,
    input_id: WidgetId<Input>,
    selected_session: Option<usize>,
}

impl App {
    pub fn new() -> Box<Self> {
        tuie::widget::widgets::input::config::update(|cfg| cfg.bindings = app_bindings);

        let mut sessions_id = WidgetId::EMPTY;
        let mut output_id = WidgetId::EMPTY;
        let mut input_id = WidgetId::EMPTY;

        let sessions: Vec<String> = SESSION_NAMES.iter().map(|s| s.to_string()).collect();
        let mut sessions_list = List::new();
        sessions_list.set_renderer(
            SessionListContext {
                sessions,
                selected: None,
            },
            render_session,
        );
        sessions_list.set_item_count(SESSION_NAMES.len());
        let sessions_list = sessions_list.id(&mut sessions_id).flex(1);

        let sessions_pane = Pane::new()
            .child(sessions_list)
            .border(Border::SINGLE)
            .title("sessions")
            .flex(1)
            .min_width(24)
            .preferred_width(32);
        let sessions_child = SplitPaneChild::from(sessions_pane).borderless();

        let output = Text::new()
            .content("ready. select a session or type a message.")
            .id(&mut output_id);
        let output_pane = Pane::new()
            .child(output)
            .border(Border::SINGLE)
            .title("output")
            .flex(3);
        let output_child = SplitPaneChild::from(output_pane).borderless();

        let main_split = Split::new(SplitPane::new().horizontal())
            .children([sessions_child, output_child])
            .flex(1);

        let input = Input::new()
            .placeholder(Text::new().content("message... (enter sends, ctrl+q quits)"))
            .id(&mut input_id);
        let input_pane = Pane::new().child(input).border(Border::SINGLE).height(3);
        let input_child = SplitPaneChild::from(input_pane).borderless();

        let root_split = Split::new(SplitPane::new().vertical())
            .children([SplitPaneChild::from(main_split).borderless(), input_child])
            .flex(1);

        let root = Pane::new().child(root_split);

        Box::new(Self {
            root,
            sessions_id,
            output_id,
            input_id,
            selected_session: None,
        })
    }

    pub fn into_root(self) -> Box<dyn Widget> {
        Box::new(self)
    }

    fn append_output(&mut self, line: &str) {
        if let Some(output) = self.root.get_widget_mut(self.output_id) {
            let current = output.get_string();
            let mut content = String::with_capacity(current.len() + line.len() + 1);
            if !current.is_empty() {
                content.push_str(&current);
                content.push('\n');
            }
            content.push_str(line);
            output.set_content(content);
        }
    }

    fn select_session(&mut self, index: usize) {
        self.selected_session = Some(index);
        if let Some(list) = self.root.get_widget_mut(self.sessions_id) {
            if let Some(ctx) = list.get_context_mut::<SessionListContext>() {
                ctx.selected = Some(index);
            }
            list.invalidate_all();
        }
        self.append_output(&format!("selected session: {}", SESSION_NAMES[index]));
    }

    fn move_selection(&mut self, direction: Sign) {
        let len = SESSION_NAMES.len();
        if len == 0 {
            return;
        }
        let next = match self.selected_session {
            Some(i) => match direction {
                Sign::Positive => i.saturating_add(1).min(len - 1),
                Sign::Negative => i.saturating_sub(1),
            },
            None => 0,
        };
        self.select_session(next);
    }

    fn submit_prompt(&mut self) {
        let message = self.root.get_widget(self.input_id).map(Input::get_string);
        let Some(message) = message else {
            return;
        };
        if message.trim().is_empty() {
            return;
        }
        if let Some(input) = self.root.get_widget_mut(self.input_id) {
            input.set_content("");
        }
        self.append_output(&format!("> {message}"));
    }
}

impl DelegateWidget for App {
    tuie::delegate_widget!(root);

    fn override_on_input(&mut self, queue: &mut InputQueue) -> InputResult {
        let Some(event) = queue.peek() else {
            return InputResult::Rejected;
        };
        let chord = event.chord.clone();
        match chord {
            chord!(Esc) | chord!(Ctrl + q) => {
                queue.next();
                tuie::quit(0);
            }
            chord!(Enter) => {
                queue.next();
                self.submit_prompt();
            }
            chord!(Up) => {
                queue.next();
                self.move_selection(Sign::Negative);
            }
            chord!(Down) => {
                queue.next();
                self.move_selection(Sign::Positive);
            }
            _ => return InputResult::Rejected,
        }
        InputResult::Handled
    }
}
