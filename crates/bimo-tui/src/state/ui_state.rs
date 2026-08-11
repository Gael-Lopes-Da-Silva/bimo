use std::time::Instant;

#[derive(Debug, Clone, PartialEq)]
pub enum UITheme {
    System,
    Light,
    Dark,
    CatppuccinMocha,
    CatppuccinLatte,
    CatppuccinFrappe,
    CatppuccinMacchiato,
}

#[derive(Debug, Clone)]
pub struct UIState {
    pub theme: UITheme,
    pub reduced_motion: bool,
    pub show_line_numbers: bool,
    pub word_wrap: bool,
    pub font_size: u16,
    pub sidebar_width: u16,
    pub sidebar_collapsed: bool,
    pub last_click: Option<Instant>,
    pub click_count: u32,
    pub hovered_element: Option<HoveredElement>,
    pub drag_state: Option<DragState>,
    pub scroll_positions: std::collections::HashMap<String, u16>,
    pub panel_sizes: std::collections::HashMap<String, (u16, u16)>, // (min, max)
    pub animations_enabled: bool,
    pub fps_target: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum HoveredElement {
    SidebarTab(usize),
    SessionItem(usize),
    ProviderItem(usize),
    ToolCall(usize, usize), // message_index, tool_index
    Button(String),
    Tab(String),
    Scrollbar,
    Divider,
    None,
}

#[derive(Debug, Clone)]
pub struct DragState {
    pub element: DragElement,
    pub start_pos: (u16, u16),
    pub current_pos: (u16, u16),
    pub initial_value: u16,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DragElement {
    SidebarDivider,
    PanelDivider(String),
    ScrollbarThumb,
    None,
}

impl UIState {
    pub fn new() -> Self {
        Self {
            theme: UITheme::CatppuccinMocha,
            reduced_motion: false,
            show_line_numbers: false,
            word_wrap: true,
            font_size: 14,
            sidebar_width: 40,
            sidebar_collapsed: false,
            last_click: None,
            click_count: 0,
            hovered_element: None,
            drag_state: None,
            scroll_positions: std::collections::HashMap::new(),
            panel_sizes: std::collections::HashMap::new(),
            animations_enabled: true,
            fps_target: 60,
        }
    }

    pub fn set_theme(&mut self, theme: UITheme) {
        self.theme = theme;
    }

    pub fn toggle_reduced_motion(&mut self) {
        self.reduced_motion = !self.reduced_motion;
        self.animations_enabled = !self.reduced_motion;
    }

    pub fn set_sidebar_width(&mut self, width: u16) {
        self.sidebar_width = width.clamp(20, 80);
    }

    pub fn toggle_sidebar(&mut self) {
        self.sidebar_collapsed = !self.sidebar_collapsed;
    }

    pub fn record_click(&mut self, x: u16, y: u16) {
        let now = Instant::now();
        if let Some(last) = self.last_click {
            if now.duration_since(last).as_millis() < 300 {
                self.click_count += 1;
            } else {
                self.click_count = 1;
            }
        } else {
            self.click_count = 1;
        }
        self.last_click = Some(now);
    }

    pub fn is_double_click(&self) -> bool {
        self.click_count >= 2
    }

    pub fn set_hovered(&mut self, element: HoveredElement) {
        self.hovered_element = Some(element);
    }

    pub fn clear_hovered(&mut self) {
        self.hovered_element = None;
    }

    pub fn start_drag(&mut self, element: DragElement, x: u16, y: u16, initial_value: u16) {
        self.drag_state = Some(DragState {
            element,
            start_pos: (x, y),
            current_pos: (x, y),
            initial_value,
        });
    }

    pub fn update_drag(&mut self, x: u16, y: u16) -> Option<u16> {
        if let Some(drag) = &mut self.drag_state {
            drag.current_pos = (x, y);
            let delta = x as i16 - drag.start_pos.0 as i16;
            Some((drag.initial_value as i16 + delta).max(0) as u16)
        } else {
            None
        }
    }

    pub fn end_drag(&mut self) -> Option<(DragElement, u16)> {
        if let Some(drag) = self.drag_state.take() {
            let delta = drag.current_pos.0 as i16 - drag.start_pos.0 as i16;
            let final_value = (drag.initial_value as i16 + delta).max(0) as u16;
            Some((drag.element, final_value))
        } else {
            None
        }
    }

    pub fn save_scroll_position(&mut self, key: String, position: u16) {
        self.scroll_positions.insert(key, position);
    }

    pub fn get_scroll_position(&self, key: &str) -> u16 {
        self.scroll_positions.get(key).copied().unwrap_or(0)
    }

    pub fn set_panel_size(&mut self, panel: String, min: u16, max: u16) {
        self.panel_sizes.insert(panel, (min, max));
    }

    pub fn get_panel_size(&self, panel: &str) -> Option<(u16, u16)> {
        self.panel_sizes.get(panel).copied()
    }
}

impl Default for UIState {
    fn default() -> Self {
        Self::new()
    }
}
