use cursive::event::{Event, Key};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyBinding {
    Submit,
    NewLine,
    OpenPalette,
    ClosePalette,
    Cancel,
    ScrollUp,
    ScrollDown,
    ScrollPageUp,
    ScrollPageDown,
    ScrollTop,
    ScrollBottom,
    NextMessage,
    PrevMessage,
    Undo,
    Redo,
    Quit,
}

impl KeyBinding {
    pub fn default_event(&self) -> Event {
        match self {
            KeyBinding::Submit => Event::Key(Key::Enter),
            KeyBinding::NewLine => Event::CtrlChar('j'),
            KeyBinding::OpenPalette => Event::CtrlChar('p'),
            KeyBinding::ClosePalette => Event::Key(Key::Esc),
            KeyBinding::Cancel => Event::Key(Key::Esc),
            KeyBinding::ScrollUp => Event::Key(Key::Up),
            KeyBinding::ScrollDown => Event::Key(Key::Down),
            KeyBinding::ScrollPageUp => Event::Key(Key::PageUp),
            KeyBinding::ScrollPageDown => Event::Key(Key::PageDown),
            KeyBinding::ScrollTop => Event::Key(Key::Home),
            KeyBinding::ScrollBottom => Event::Key(Key::End),
            KeyBinding::NextMessage => Event::CtrlChar('n'),
            KeyBinding::PrevMessage => Event::CtrlChar('p'),
            KeyBinding::Undo => Event::CtrlChar('z'),
            KeyBinding::Redo => Event::CtrlChar('y'),
            KeyBinding::Quit => Event::CtrlChar('q'),
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            KeyBinding::Submit => "Submit prompt",
            KeyBinding::NewLine => "Add new line",
            KeyBinding::OpenPalette => "Open command palette",
            KeyBinding::ClosePalette => "Close palette/dialog",
            KeyBinding::Cancel => "Cancel current action",
            KeyBinding::ScrollUp => "Scroll up",
            KeyBinding::ScrollDown => "Scroll down",
            KeyBinding::ScrollPageUp => "Scroll page up",
            KeyBinding::ScrollPageDown => "Scroll page down",
            KeyBinding::ScrollTop => "Scroll to top",
            KeyBinding::ScrollBottom => "Scroll to bottom",
            KeyBinding::NextMessage => "Next message",
            KeyBinding::PrevMessage => "Previous message",
            KeyBinding::Undo => "Undo",
            KeyBinding::Redo => "Redo",
            KeyBinding::Quit => "Quit application",
        }
    }

    pub fn all() -> Vec<Self> {
        vec![
            KeyBinding::Submit,
            KeyBinding::NewLine,
            KeyBinding::OpenPalette,
            KeyBinding::ClosePalette,
            KeyBinding::Cancel,
            KeyBinding::ScrollUp,
            KeyBinding::ScrollDown,
            KeyBinding::ScrollPageUp,
            KeyBinding::ScrollPageDown,
            KeyBinding::ScrollTop,
            KeyBinding::ScrollBottom,
            KeyBinding::NextMessage,
            KeyBinding::PrevMessage,
            KeyBinding::Undo,
            KeyBinding::Redo,
            KeyBinding::Quit,
        ]
    }
}

pub struct KeyBindings {
    bindings: std::collections::HashMap<KeyBinding, Event>,
}

impl KeyBindings {
    pub fn new() -> Self {
        let mut bindings = std::collections::HashMap::new();
        for kb in KeyBinding::all() {
            bindings.insert(kb, kb.default_event());
        }
        Self { bindings }
    }

    pub fn get(&self, binding: KeyBinding) -> Event {
        self.bindings
            .get(&binding)
            .cloned()
            .unwrap_or_else(|| binding.default_event())
    }

    pub fn set(&mut self, binding: KeyBinding, event: Event) {
        self.bindings.insert(binding, event);
    }
}

impl Default for KeyBindings {
    fn default() -> Self {
        Self::new()
    }
}

pub fn setup_global_keybindings(siv: &mut cursive::Cursive) {
    let bindings = KeyBindings::new();

    siv.add_global_callback(bindings.get(KeyBinding::OpenPalette), toggle_palette);
    siv.add_global_callback(bindings.get(KeyBinding::NewLine), insert_newline);
    siv.add_global_callback(bindings.get(KeyBinding::ClosePalette), close_top_layer);
    siv.add_global_callback(bindings.get(KeyBinding::Quit), |s| {
        s.quit();
    });

    siv.add_global_callback(Event::CtrlChar('c'), |s| {
        s.quit();
    });
}

fn toggle_palette(siv: &mut cursive::Cursive) {
    let is_open = siv
        .call_on_name("command_palette", |_: &mut cursive::views::Dialog| {})
        .is_some();
    if is_open {
        siv.pop_layer();
    } else {
        siv.add_layer(crate::palette::create_command_palette_layer(
            crate::palette::CommandRegistry::default(),
        ));
    }
}

fn insert_newline(siv: &mut cursive::Cursive) {
    siv.call_on_name("input", |input: &mut cursive::views::EditView| {
        let content = input.get_content().to_string();
        input.set_content(format!("{}\n", content));
    });
}

fn close_top_layer(siv: &mut cursive::Cursive) {
    if siv.screen().len() > 1 {
        siv.pop_layer();
    }
}
