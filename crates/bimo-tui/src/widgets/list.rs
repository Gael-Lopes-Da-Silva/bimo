use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    widgets::{List as RatatuiList, ListItem, ListState, StatefulWidget, Widget},
};

pub struct SelectableList<'a, T> {
    items: &'a [T],
    state: &'a mut ListState,
    style: Style,
    selected_style: Style,
    format: Box<dyn Fn(&T) -> String + 'a>,
    show_scrollbar: bool,
}

impl<'a, T> SelectableList<'a, T> {
    pub fn new(items: &'a [T], state: &'a mut ListState) -> Self {
        Self {
            items,
            state,
            style: Style::default(),
            selected_style: Style::default(),
            format: Box::new(|_| String::new()),
            show_scrollbar: true,
        }
    }

    pub fn styles(mut self, style: Style, selected_style: Style) -> Self {
        self.style = style;
        self.selected_style = selected_style;
        self
    }

    pub fn format<F>(mut self, f: F) -> Self
    where
        F: Fn(&T) -> String + 'a,
    {
        self.format = Box::new(f);
        self
    }

    pub fn show_scrollbar(mut self, show: bool) -> Self {
        self.show_scrollbar = show;
        self
    }
}

impl<'a, T> Widget for SelectableList<'a, T> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let items: Vec<ListItem> = self
            .items
            .iter()
            .map(|item| {
                let text = (self.format)(item);
                ListItem::new(text).style(self.style)
            })
            .collect();

        let list = RatatuiList::new(items)
            .highlight_style(self.selected_style)
            .highlight_symbol("► ");

        StatefulWidget::render(list, area, buf, self.state);
    }
}
