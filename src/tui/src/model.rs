use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::palette::{self, PaletteItem};

/// Where keyboard input is being directed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    /// The prompt line accepts text and Enter submits it.
    Normal,
    /// The command palette is open and accepts filtering text.
    Palette,
}

/// Side effects requested from the event loop after handling a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppAction {
    /// Nothing further to do.
    None,
    /// The application should exit.
    Quit,
}

/// The TUI model: pure application state plus key-driven transitions.
pub struct App {
    pub mode: AppMode,
    pub input: String,
    pub input_cursor: usize,
    pub output: Vec<String>,
    pub scroll: usize,
    pub palette_filter: String,
    pub palette_selection: usize,
}

impl App {
    pub fn new() -> Self {
        Self {
            mode: AppMode::Normal,
            input: String::new(),
            input_cursor: 0,
            output: Vec::new(),
            scroll: 0,
            palette_filter: String::new(),
            palette_selection: 0,
        }
    }

    /// Applies a key event to the model and reports the resulting action.
    pub fn handle_key(&mut self, key: KeyEvent) -> AppAction {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return AppAction::Quit;
        }
        if is_ctrl_shift_p(key) {
            self.toggle_palette();
            return AppAction::None;
        }

        match self.mode {
            AppMode::Palette => self.handle_palette_key(key),
            AppMode::Normal => self.handle_normal_key(key),
        }
    }

    /// Whether the palette should currently be rendered.
    pub fn is_palette_open(&self) -> bool {
        self.mode == AppMode::Palette
    }

    /// The palette commands matching the current filter.
    pub fn palette_items(&self) -> Vec<&'static PaletteItem> {
        palette::filter(palette::PALETTE_ITEMS, &self.palette_filter)
    }

    /// Moves the submitted prompt into the output log.
    pub fn submit_input(&mut self) {
        let prompt = self.input.trim();
        if prompt.is_empty() {
            return;
        }
        self.output.push(prompt.to_string());
        self.input.clear();
        self.input_cursor = 0;
        self.scroll = 0;
    }

    pub fn clear_output(&mut self) {
        self.output.clear();
        self.scroll = 0;
    }

    fn handle_normal_key(&mut self, key: KeyEvent) -> AppAction {
        match key.code {
            KeyCode::Enter => self.submit_input(),
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.insert_char(c);
            }
            KeyCode::Backspace => self.backspace(),
            KeyCode::Delete => self.delete(),
            KeyCode::Left => self.move_left(),
            KeyCode::Right => self.move_right(),
            KeyCode::Home => self.input_cursor = 0,
            KeyCode::End => self.input_cursor = self.input.len(),
            KeyCode::Up => self.scroll_up(),
            KeyCode::Down => self.scroll_down(),
            _ => {}
        }
        AppAction::None
    }

    fn handle_palette_key(&mut self, key: KeyEvent) -> AppAction {
        match key.code {
            KeyCode::Esc => self.close_palette(),
            KeyCode::Enter => return self.run_palette_selection(),
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.palette_filter.push(c);
                self.palette_selection = 0;
            }
            KeyCode::Backspace => {
                self.palette_filter.pop();
                self.palette_selection = 0;
            }
            KeyCode::Up => self.move_selection(-1),
            KeyCode::Down => self.move_selection(1),
            _ => {}
        }
        AppAction::None
    }

    fn run_palette_selection(&mut self) -> AppAction {
        let item = self
            .palette_items()
            .get(self.palette_selection)
            .cloned()
            .unwrap_or(&palette::PALETTE_ITEMS[0]);
        match item.label {
            "Quit" => AppAction::Quit,
            "Clear output" => {
                self.clear_output();
                self.close_palette();
                AppAction::None
            }
            _ => {
                self.close_palette();
                AppAction::None
            }
        }
    }

    fn toggle_palette(&mut self) {
        match self.mode {
            AppMode::Normal => {
                self.mode = AppMode::Palette;
                self.palette_filter.clear();
                self.palette_selection = 0;
            }
            AppMode::Palette => self.close_palette(),
        }
    }

    fn close_palette(&mut self) {
        self.mode = AppMode::Normal;
    }

    fn move_selection(&mut self, delta: isize) {
        let len = self.palette_items().len();
        if len == 0 {
            return;
        }
        let next = self.palette_selection as isize + delta;
        self.palette_selection = next.rem_euclid(len as isize) as usize;
    }

    fn insert_char(&mut self, c: char) {
        self.input.insert(self.input_cursor, c);
        self.input_cursor += c.len_utf8();
    }

    fn backspace(&mut self) {
        if self.input_cursor == 0 {
            return;
        }
        let Some((start, _)) = self.input[..self.input_cursor].char_indices().next_back() else {
            return;
        };
        self.input_cursor = start;
        self.input.remove(self.input_cursor);
    }

    fn delete(&mut self) {
        if self.input_cursor >= self.input.len() {
            return;
        }
        self.input.remove(self.input_cursor);
    }

    fn move_left(&mut self) {
        if self.input_cursor == 0 {
            return;
        }
        self.input_cursor = self.input[..self.input_cursor]
            .char_indices()
            .next_back()
            .map(|(i, _)| i)
            .unwrap_or(0);
    }

    fn move_right(&mut self) {
        if self.input_cursor >= self.input.len() {
            return;
        }
        if let Some((_, c)) = self.input[self.input_cursor..].char_indices().next() {
            self.input_cursor += c.len_utf8();
        }
    }

    fn scroll_up(&mut self) {
        self.scroll = self.scroll.saturating_add(1);
    }

    fn scroll_down(&mut self) {
        self.scroll = self.scroll.saturating_sub(1);
    }
}

fn is_ctrl_shift_p(key: KeyEvent) -> bool {
    matches!(key.code, KeyCode::Char('p' | 'P'))
        && key.modifiers == KeyModifiers::CONTROL | KeyModifiers::SHIFT
}
