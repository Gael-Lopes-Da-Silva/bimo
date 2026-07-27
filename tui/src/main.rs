use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
};
use serde::Deserialize;
use std::io;
use tokio::sync::mpsc;

// ---------------------------------------------------------------------------
// API types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct ApiResponse {
    success: bool,
    data: Option<serde_json::Value>,
    error: Option<ApiError>,
}

#[derive(Debug, Deserialize)]
struct ApiError {
    code: String,
    message: String,
}

#[derive(Debug, Deserialize)]
struct StatusData {
    provider: Option<String>,
    model: Option<String>,
    #[allow(dead_code)]
    session_id: String,
    #[allow(dead_code)]
    message_count: usize,
    needs_configuration: bool,
}

#[derive(Debug, Deserialize)]
struct ChatData {
    content: String,
    #[allow(dead_code)]
    model: Option<String>,
    #[allow(dead_code)]
    session_id: String,
}

#[derive(Debug, Deserialize)]
struct CommandsData {
    commands: Vec<CommandInfo>,
}

#[derive(Debug, Deserialize, Clone)]
struct CommandInfo {
    name: String,
    description: String,
    subcommands: Vec<SubcommandInfo>,
}

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)]
struct SubcommandInfo {
    name: String,
    description: String,
    usage: String,
}

// ---------------------------------------------------------------------------
// Autocomplete state
// ---------------------------------------------------------------------------

struct Autocomplete {
    visible: bool,
    items: Vec<CommandInfo>,
    filtered: Vec<CommandInfo>,
    state: ListState,
    prefix: String,
}

impl Autocomplete {
    fn new() -> Self {
        Self {
            visible: false,
            items: Vec::new(),
            filtered: Vec::new(),
            state: ListState::default(),
            prefix: String::new(),
        }
    }

    fn show(&mut self, items: Vec<CommandInfo>, query: &str) {
        self.items = items;
        self.filter(query);
        self.visible = true;
    }

    fn hide(&mut self) {
        self.visible = false;
        self.state.select(None);
    }

    fn filter(&mut self, query: &str) {
        self.prefix = query.to_string();
        let q = query.trim_start_matches('/');
        self.filtered = self
            .items
            .iter()
            .filter(|c| q.is_empty() || c.name.starts_with(q))
            .cloned()
            .collect();
        if self.filtered.is_empty() {
            self.state.select(None);
        } else {
            self.state.select(Some(0));
        }
    }

    fn next(&mut self) {
        if self.filtered.is_empty() {
            return;
        }
        let i = self
            .state
            .selected()
            .map(|i| (i + 1) % self.filtered.len())
            .unwrap_or(0);
        self.state.select(Some(i));
    }

    fn prev(&mut self) {
        if self.filtered.is_empty() {
            return;
        }
        let i = self
            .state
            .selected()
            .map(|i| {
                if i == 0 {
                    self.filtered.len() - 1
                } else {
                    i - 1
                }
            })
            .unwrap_or(0);
        self.state.select(Some(i));
    }

    fn selected_command(&self) -> Option<&CommandInfo> {
        self.state.selected().and_then(|i| self.filtered.get(i))
    }
}

// ---------------------------------------------------------------------------
// Chat message (local display)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
enum ChatEntry {
    User(String),
    Assistant(String),
    System(String),
    Error(String),
}

// ---------------------------------------------------------------------------
// Application state
// ---------------------------------------------------------------------------

struct App {
    base_url: String,
    input: String,
    messages: Vec<ChatEntry>,
    scroll: usize,
    loading: bool,
    provider: Option<String>,
    model: Option<String>,
    status_msg: Option<String>,
    quit: bool,
    autocomplete: Autocomplete,
}

impl App {
    fn new(base_url: String) -> Self {
        Self {
            base_url,
            input: String::new(),
            messages: Vec::new(),
            scroll: 0,
            loading: false,
            provider: None,
            model: None,
            status_msg: None,
            quit: false,
            autocomplete: Autocomplete::new(),
        }
    }

    fn push_message(&mut self, entry: ChatEntry) {
        self.messages.push(entry);
        self.scroll = self.messages.len();
    }
}

// ---------------------------------------------------------------------------
// HTTP client
// ---------------------------------------------------------------------------

async fn api_get(base: &str, path: &str) -> Result<ApiResponse, reqwest::Error> {
    reqwest::get(format!("{base}{path}")).await?.json().await
}

async fn api_post(
    base: &str,
    path: &str,
    body: &impl serde::Serialize,
) -> Result<ApiResponse, reqwest::Error> {
    reqwest::Client::new()
        .post(format!("{base}{path}"))
        .json(body)
        .send()
        .await?
        .json()
        .await
}

async fn fetch_status(base: &str) -> Option<StatusData> {
    let resp = api_get(base, "/api/status").await.ok()?;
    if resp.success {
        resp.data.and_then(|v| serde_json::from_value(v).ok())
    } else {
        None
    }
}

async fn fetch_commands(base: &str) -> Vec<CommandInfo> {
    let resp = match api_get(base, "/api/commands").await {
        Ok(r) => r,
        Err(_) => return vec![],
    };
    if resp.success {
        if let Some(data) = resp.data {
            if let Ok(cmds) = serde_json::from_value::<CommandsData>(data) {
                return cmds.commands;
            }
        }
    }
    vec![]
}

async fn send_chat(base: &str, message: &str) -> Result<String, String> {
    let resp = api_post(
        base,
        "/api/chat",
        &serde_json::json!({ "message": message }),
    )
    .await
    .map_err(|e| format!("request failed: {e}"))?;

    if resp.success {
        if let Some(data) = resp.data {
            if let Ok(chat) = serde_json::from_value::<ChatData>(data) {
                return Ok(chat.content);
            }
        }
        Err("empty response from server".into())
    } else {
        let err = resp.error.unwrap_or(ApiError {
            code: "UNKNOWN".into(),
            message: "unknown error".into(),
        });
        Err(format!("[{}] {}", err.code, err.message))
    }
}

async fn send_command(base: &str, command: &str) -> Result<String, String> {
    let resp = api_post(
        base,
        "/api/command",
        &serde_json::json!({ "command": command }),
    )
    .await
    .map_err(|e| format!("request failed: {e}"))?;

    if resp.success {
        if let Some(data) = resp.data {
            if let Some(output) = data.get("output").and_then(|o| o.as_str()) {
                return Ok(output.to_string());
            }
        }
        Ok("done".into())
    } else {
        let err = resp.error.unwrap_or(ApiError {
            code: "UNKNOWN".into(),
            message: "unknown error".into(),
        });
        Err(format!("[{}] {}", err.code, err.message))
    }
}

// ---------------------------------------------------------------------------
// UI rendering
// ---------------------------------------------------------------------------

fn draw(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(frame.area());

    render_messages(frame, app, chunks[0]);
    render_input(frame, app, chunks[1]);
    render_statusbar(frame, app, chunks[2]);

    if app.autocomplete.visible {
        render_autocomplete(frame, app);
    }
}

/// Split text into lines, preserving empty lines from consecutive newlines.
fn split_lines(text: &str) -> Vec<String> {
    text.split('\n').map(|s| s.to_string()).collect()
}

fn render_messages(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let lines: Vec<Line> = app
        .messages
        .iter()
        .flat_map(|entry| match entry {
            ChatEntry::User(text) => split_lines(text)
                .into_iter()
                .enumerate()
                .map(|(i, l)| {
                    if i == 0 {
                        Line::from(vec![
                            Span::styled(
                                "you  ",
                                Style::default()
                                    .fg(Color::Cyan)
                                    .add_modifier(Modifier::BOLD),
                            ),
                            Span::raw(l),
                        ])
                    } else {
                        Line::from(vec![Span::raw("      ".to_string() + &l)])
                    }
                })
                .collect::<Vec<_>>(),
            ChatEntry::Assistant(text) => split_lines(text)
                .into_iter()
                .enumerate()
                .map(|(i, l)| {
                    if i == 0 {
                        Line::from(vec![
                            Span::styled(
                                "bimo ",
                                Style::default()
                                    .fg(Color::Green)
                                    .add_modifier(Modifier::BOLD),
                            ),
                            Span::raw(l),
                        ])
                    } else {
                        Line::from(vec![Span::raw("      ".to_string() + &l)])
                    }
                })
                .collect::<Vec<_>>(),
            ChatEntry::System(text) => split_lines(text)
                .into_iter()
                .enumerate()
                .map(|(i, l)| {
                    if i == 0 {
                        Line::from(vec![Span::styled(
                            format!("sys  {l}"),
                            Style::default().fg(Color::DarkGray),
                        )])
                    } else {
                        Line::from(vec![Span::styled(
                            format!("      {l}"),
                            Style::default().fg(Color::DarkGray),
                        )])
                    }
                })
                .collect::<Vec<_>>(),
            ChatEntry::Error(text) => split_lines(text)
                .into_iter()
                .enumerate()
                .map(|(i, l)| {
                    if i == 0 {
                        Line::from(vec![Span::styled(
                            format!("err  {l}"),
                            Style::default().fg(Color::Red),
                        )])
                    } else {
                        Line::from(vec![Span::styled(
                            format!("      {l}"),
                            Style::default().fg(Color::Red),
                        )])
                    }
                })
                .collect::<Vec<_>>(),
        })
        .collect();

    let total_lines = lines.len().max(1);
    let visible = area.height.saturating_sub(2) as usize;
    let max_scroll = total_lines.saturating_sub(visible);
    let scroll = app.scroll.min(max_scroll);

    let paragraph = Paragraph::new(lines)
        .block(block)
        .scroll((scroll as u16, 0));

    frame.render_widget(paragraph, area);
}

fn render_input(frame: &mut Frame, app: &App, area: Rect) {
    let border_color = if app.loading {
        Color::Yellow
    } else {
        Color::DarkGray
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let (display, cursor_style) = if app.loading {
        (
            "waiting for response...",
            Style::default().fg(Color::DarkGray),
        )
    } else {
        (&app.input[..], Style::default())
    };

    let text = format!("{display}|");
    let paragraph = Paragraph::new(text).block(block).style(cursor_style);
    frame.render_widget(paragraph, area);
}

fn render_statusbar(frame: &mut Frame, app: &App, area: Rect) {
    let provider = app.provider.as_deref().unwrap_or("none");
    let model = app.model.as_deref().unwrap_or("none");

    let status_color = if app.provider.is_some() {
        Color::Green
    } else {
        Color::Yellow
    };

    let left = format!(" provider: {provider}  |  model: {model}");
    let right = if let Some(ref msg) = app.status_msg {
        msg.clone()
    } else if app.autocomplete.visible {
        "Tab/Up/Down: navigate  |  Enter: select  |  Esc: cancel".into()
    } else {
        "Esc: quit  |  Enter: send  |  Tab: autocomplete".into()
    };

    let width = area.width as usize;
    let gap = width.saturating_sub(left.len() + right.len() + 1);
    let padding = " ".repeat(gap);

    let line = Line::from(vec![
        Span::styled(left, Style::default().fg(status_color)),
        Span::raw(padding),
        Span::styled(right, Style::default().fg(Color::DarkGray)),
    ]);

    let paragraph = Paragraph::new(line).style(Style::default().bg(Color::Black).fg(Color::White));
    frame.render_widget(paragraph, area);
}

fn render_autocomplete(frame: &mut Frame, app: &App) {
    let area = frame.area();

    let popup_height = (app.autocomplete.filtered.len() as u16 + 2).min(12);
    let popup_width = area.width;
    let x = 0;
    let y = area.height - 4 - popup_height;

    let area = Rect {
        x,
        y,
        width: popup_width,
        height: popup_height,
    };

    frame.render_widget(Clear, area);

    let items: Vec<ListItem> = app
        .autocomplete
        .filtered
        .iter()
        .map(|cmd| {
            let name = format!("/{}", cmd.name);
            let desc = if cmd.subcommands.is_empty() {
                cmd.description.clone()
            } else {
                let subs: Vec<&str> = cmd.subcommands.iter().map(|s| s.name.as_str()).collect();
                format!("{} [{}]", cmd.description, subs.join("|"))
            };
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("{name:<14}"),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(desc, Style::default().fg(Color::DarkGray)),
            ]))
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");

    let mut state = app.autocomplete.state.clone();
    frame.render_stateful_widget(list, area, &mut state);
}

// ---------------------------------------------------------------------------
// Background tasks
// ---------------------------------------------------------------------------

enum AppEvent {
    ChatResponse(String),
    CommandResponse(String),
    ErrorResponse(String),
    StatusUpdate(StatusData),
    CommandsLoaded(Vec<CommandInfo>),
}

fn spawn_chat_task(base: String, message: String, tx: mpsc::UnboundedSender<AppEvent>) {
    tokio::spawn(async move {
        match send_chat(&base, &message).await {
            Ok(content) => {
                let _ = tx.send(AppEvent::ChatResponse(content));
            }
            Err(e) => {
                let _ = tx.send(AppEvent::ErrorResponse(e));
            }
        }
    });
}

fn spawn_command_task(base: String, command: String, tx: mpsc::UnboundedSender<AppEvent>) {
    tokio::spawn(async move {
        match send_command(&base, &command).await {
            Ok(output) => {
                let _ = tx.send(AppEvent::CommandResponse(output));
            }
            Err(e) => {
                let _ = tx.send(AppEvent::ErrorResponse(e));
            }
        }
    });
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let base_url = std::env::var("BIMO_URL").unwrap_or_else(|_| "http://localhost:3847".into());

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(base_url.clone());
    app.push_message(ChatEntry::System(
        "Bimo TUI — type a message or /help for commands".into(),
    ));

    // Fetch initial status
    if let Some(status) = fetch_status(&base_url).await {
        app.provider = status.provider.clone();
        app.model = status.model.clone();
        if status.needs_configuration {
            app.push_message(ChatEntry::System(
                "No provider selected. Use the API to configure one first.".into(),
            ));
        }
    } else {
        app.push_message(ChatEntry::Error(
            "Cannot reach Bimo API. Is the server running?".into(),
        ));
    }

    let (tx, mut rx) = mpsc::unbounded_channel::<AppEvent>();

    // Fetch commands for autocomplete
    {
        let cmd_tx = tx.clone();
        let cmd_base = base_url.clone();
        tokio::spawn(async move {
            let cmds = fetch_commands(&cmd_base).await;
            let _ = cmd_tx.send(AppEvent::CommandsLoaded(cmds));
        });
    }

    // Spawn periodic status refresh
    {
        let status_tx = tx.clone();
        let status_base = base_url.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                if let Some(status) = fetch_status(&status_base).await {
                    let _ = status_tx.send(AppEvent::StatusUpdate(status));
                }
            }
        });
    }

    // Main event loop
    loop {
        terminal.draw(|frame| draw(frame, &app))?;

        if app.quit {
            break;
        }

        // Poll crossterm events with a short timeout so we can process API responses
        if event::poll(std::time::Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                // --- Autocomplete mode ---
                if app.autocomplete.visible {
                    match key.code {
                        KeyCode::Esc => {
                            app.autocomplete.hide();
                        }
                        KeyCode::Up => {
                            app.autocomplete.prev();
                        }
                        KeyCode::Down | KeyCode::Tab => {
                            app.autocomplete.next();
                        }
                        KeyCode::Enter => {
                            let cmd_name = app
                                .autocomplete
                                .selected_command()
                                .map(|c| format!("/{}", c.name));
                            app.autocomplete.hide();
                            if let Some(input) = cmd_name {
                                app.input.clear();
                                app.status_msg = None;

                                if input == "/clear" {
                                    app.messages.clear();
                                    app.scroll = 0;
                                    app.status_msg = Some("session cleared".into());
                                } else if input == "/exit" || input == "/quit" {
                                    app.quit = true;
                                } else {
                                    app.push_message(ChatEntry::System(format!(">>> {input}")));
                                    app.loading = true;
                                    spawn_command_task(app.base_url.clone(), input, tx.clone());
                                }
                            }
                        }
                        KeyCode::Char(c) => {
                            app.input.push(c);
                            app.autocomplete.filter(&app.input);
                            if app.autocomplete.filtered.is_empty() {
                                app.autocomplete.hide();
                            }
                        }
                        KeyCode::Backspace => {
                            app.input.pop();
                            if app.input.is_empty() || !app.input.starts_with('/') {
                                app.autocomplete.hide();
                            } else {
                                app.autocomplete.filter(&app.input);
                            }
                        }
                        _ => {}
                    }
                    continue;
                }

                // --- Normal mode ---
                match key.code {
                    KeyCode::Esc => {
                        app.quit = true;
                    }
                    KeyCode::Enter => {
                        let input = app.input.trim().to_string();
                        if input.is_empty() {
                            continue;
                        }
                        app.input.clear();
                        app.status_msg = None;

                        if input.starts_with('/') {
                            if input == "/clear" {
                                app.messages.clear();
                                app.scroll = 0;
                                app.status_msg = Some("session cleared".into());
                                continue;
                            }
                            if input == "/exit" || input == "/quit" {
                                app.quit = true;
                                continue;
                            }
                            app.push_message(ChatEntry::System(format!(">>> {input}")));
                            app.loading = true;
                            spawn_command_task(app.base_url.clone(), input, tx.clone());
                        } else {
                            app.push_message(ChatEntry::User(input.clone()));
                            app.loading = true;
                            spawn_chat_task(app.base_url.clone(), input, tx.clone());
                        }
                    }
                    KeyCode::Tab => {
                        // Open autocomplete with all commands if input is empty or just `/`
                        if app.input.is_empty() || app.input == "/" {
                            app.input = "/".to_string();
                            let cmds = app.autocomplete.items.clone();
                            if !cmds.is_empty() {
                                app.autocomplete.show(cmds, &app.input);
                            }
                        } else if app.input.starts_with('/') {
                            // Filter existing commands
                            let cmds = app.autocomplete.items.clone();
                            if !cmds.is_empty() {
                                app.autocomplete.show(cmds, &app.input);
                            }
                        }
                    }
                    KeyCode::Char(c) => {
                        app.input.push(c);
                        // Auto-open autocomplete when user types `/`
                        if app.input == "/" {
                            let cmds = app.autocomplete.items.clone();
                            if !cmds.is_empty() {
                                app.autocomplete.show(cmds, &app.input);
                            }
                        }
                    }
                    KeyCode::Backspace => {
                        app.input.pop();
                    }
                    KeyCode::Up => {
                        app.scroll = app.scroll.saturating_sub(1);
                    }
                    KeyCode::Down => {
                        app.scroll = app.scroll.saturating_add(1);
                    }
                    KeyCode::PageUp => {
                        app.scroll = app.scroll.saturating_sub(10);
                    }
                    KeyCode::PageDown => {
                        app.scroll = app.scroll.saturating_add(10);
                    }
                    KeyCode::Home => {
                        app.scroll = 0;
                    }
                    KeyCode::End => {
                        app.scroll = app.messages.len();
                    }
                    _ => {}
                }
            }
        }

        // Process any pending API responses (non-blocking)
        while let Ok(event) = rx.try_recv() {
            match event {
                AppEvent::ChatResponse(content) => {
                    app.loading = false;
                    app.push_message(ChatEntry::Assistant(content));
                }
                AppEvent::CommandResponse(output) => {
                    app.loading = false;
                    app.push_message(ChatEntry::System(output));
                }
                AppEvent::ErrorResponse(e) => {
                    app.loading = false;
                    app.push_message(ChatEntry::Error(e));
                }
                AppEvent::StatusUpdate(status) => {
                    app.provider = status.provider;
                    app.model = status.model;
                }
                AppEvent::CommandsLoaded(cmds) => {
                    let mut items = cmds;
                    items.push(CommandInfo {
                        name: "exit".into(),
                        description: "quit the TUI".into(),
                        subcommands: vec![],
                    });
                    items.push(CommandInfo {
                        name: "quit".into(),
                        description: "alias for /exit".into(),
                        subcommands: vec![],
                    });
                    items.sort_by(|a, b| a.name.cmp(&b.name));
                    app.autocomplete.items = items;
                }
            }
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}
