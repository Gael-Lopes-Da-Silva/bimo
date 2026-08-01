use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::model::{App, AppMode};

pub fn draw(frame: &mut Frame, app: &App) {
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(3)])
        .split(frame.area());

    draw_output(frame, app, areas[0]);
    draw_input(frame, app, areas[1]);

    if app.is_palette_open() {
        draw_palette(frame, app);
    }
}

fn draw_output(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default().borders(Borders::TOP).title("Output");
    if app.output.is_empty() {
        let placeholder = Paragraph::new("No output yet. Type a prompt below and press Enter.")
            .block(block)
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(placeholder, area);
        return;
    }

    let inner = block.inner(area);
    let visible = inner.height as usize;
    let scroll = app.scroll.min(app.output.len().saturating_sub(1));
    let start = app.output.len().saturating_sub(visible + scroll);
    let items: Vec<ListItem> = app
        .output
        .iter()
        .skip(start)
        .map(|line| ListItem::new(Line::from(Span::raw(line.clone()))))
        .collect();

    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}

fn draw_input(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::TOP)
        .title(Line::from(vec![
            Span::raw("Prompt"),
            Span::styled(
                "  [Ctrl+Shift+P: palette]",
                Style::default().fg(Color::DarkGray),
            ),
        ]));
    let inner = block.inner(area);

    let (visible, cursor_col) = visible_input(app, inner.width as usize);
    let line = Line::from(vec![
        Span::styled("> ", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(visible),
    ]);
    frame.render_widget(Paragraph::new(line).block(block), area);

    if app.mode == AppMode::Normal {
        let x = inner.x + 2 + cursor_col as u16;
        frame.set_cursor_position((x, inner.y));
    }
}

fn draw_palette(frame: &mut Frame, app: &App) {
    let max_w = 60u16;
    let max_h = 15u16;
    let w = frame.area().width.min(max_w);
    let h = frame.area().height.min(max_h).max(3);
    let x = frame.area().x + frame.area().width.saturating_sub(w) / 2;
    let y = frame.area().y + frame.area().height.saturating_sub(h) / 3;
    let area = Rect::new(x, y, w, h);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title("Command Palette");
    let inner = block.inner(area);

    let prompt = format!("> {}", app.palette_filter);
    let filter_paragraph = Paragraph::new(prompt.as_str())
        .style(Style::default().fg(Color::Yellow))
        .block(Block::default().borders(Borders::BOTTOM));
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(1)])
        .split(inner);
    let (filter_area, list_area) = (areas[0], areas[1]);
    frame.render_widget(filter_paragraph, filter_area);

    let items = app.palette_items();
    if items.is_empty() {
        let empty = Paragraph::new("No matching commands")
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default())
            .alignment(Alignment::Center);
        frame.render_widget(empty, list_area);
    } else {
        let list_items: Vec<ListItem> = items
            .iter()
            .map(|item| ListItem::new(Line::from(item.to_string())))
            .collect();
        let mut state = ListState::default();
        state.select(Some(app.palette_selection.min(list_items.len() - 1)));
        let list = List::new(list_items)
            .block(Block::default())
            .highlight_style(
                Style::default()
                    .bg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("> ");
        frame.render_stateful_widget(list, list_area, &mut state);
    }

    frame.render_widget(block, area);

    let x = inner.x + 2 + app.palette_filter.width() as u16;
    frame.set_cursor_position((x, inner.y));
}

/// Computes the portion of the input to display and the cursor's column
/// within it, horizontally scrolling so the cursor stays visible.
fn visible_input(app: &App, max_width: usize) -> (String, usize) {
    let cursor_col = app.input[..app.input_cursor].width();
    let offset = cursor_col.saturating_sub(max_width.saturating_sub(1));
    let byte_offset = byte_offset_for_col(&app.input, offset);
    let visible = app.input[byte_offset..].to_string();
    (visible, cursor_col - offset)
}

fn byte_offset_for_col(text: &str, col: usize) -> usize {
    let mut acc = 0usize;
    for (i, c) in text.char_indices() {
        let w = c.width().unwrap_or(1);
        if acc + w > col {
            return i;
        }
        acc += w;
    }
    text.len()
}
