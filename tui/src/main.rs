use bimo_api::BimoApi;
use bimo_api::api::dto::{ApiResponse, ChatRequest, CommandRequest};
use crossterm::event::{
    Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEventKind,
};
use futures::StreamExt;
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
};
use tokio::sync::oneshot;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Debug, Clone)]
struct DisplayMessage {
    role: String,
    content: String,
}

enum Status {
    Ready,
    Busy,
}

enum WorkerMsg {
    Chat {
        message: String,
        tx: oneshot::Sender<WorkerResult>,
    },
    Command {
        command: String,
        tx: oneshot::Sender<WorkerResult>,
    },
    ListCommands {
        tx: oneshot::Sender<WorkerResult>,
    },
}

enum WorkerResult {
    Response(ApiResponse),
}

struct App {
    worker_tx: tokio::sync::mpsc::UnboundedSender<WorkerMsg>,
    messages: Vec<DisplayMessage>,
    input: String,
    cursor: usize,
    status: Status,
    pending_rx: Option<oneshot::Receiver<WorkerResult>>,
    scroll: usize,
    auto_scroll: bool,
    should_quit: bool,
    provider: Option<String>,
    model: Option<String>,
    session_id: String,
    message_count: usize,
    needs_config: bool,
    commands: Vec<(String, String)>,
    completion_visible: bool,
    completion_selected: usize,
    completion_offset: usize,
    completion_popup_area: Option<Rect>,
}

impl App {
    fn new(worker_tx: tokio::sync::mpsc::UnboundedSender<WorkerMsg>) -> Self {
        Self {
            worker_tx,
            messages: Vec::new(),
            input: String::new(),
            cursor: 0,
            status: Status::Ready,
            pending_rx: None,
            scroll: 0,
            auto_scroll: true,
            should_quit: false,
            provider: None,
            model: None,
            session_id: String::new(),
            message_count: 0,
            needs_config: true,
            commands: Vec::new(),
            completion_visible: false,
            completion_selected: 0,
            completion_offset: 0,
            completion_popup_area: None,
        }
    }

    fn boot_message(&mut self) {
        self.messages.push(DisplayMessage {
            role: "system".into(),
            content:
                "Welcome to Bimo TUI. Type a message to start chatting, or /help for commands."
                    .into(),
        });
    }

    async fn refresh_state(&mut self) {
        let (tx, rx) = oneshot::channel();
        if self
            .worker_tx
            .send(WorkerMsg::Command {
                command: "/status".into(),
                tx,
            })
            .is_err()
        {
            return;
        }
        if let Ok(WorkerResult::Response(resp)) = rx.await
            && let Some(data) = resp.data
        {
            self.provider = data["provider"].as_str().map(String::from);
            self.model = data["model"].as_str().map(String::from);
            self.session_id = data["session_id"]
                .as_str()
                .map(String::from)
                .unwrap_or_default();
            self.message_count = data["message_count"].as_u64().unwrap_or(0) as usize;
            self.needs_config = data["needs_configuration"].as_bool().unwrap_or(true);
        }
    }

    fn set_initial_state(&mut self, data: &serde_json::Value) {
        self.provider = data["provider"].as_str().map(String::from);
        self.model = data["model"].as_str().map(String::from);
        self.session_id = data["session_id"]
            .as_str()
            .map(String::from)
            .unwrap_or_default();
        self.message_count = data["message_count"].as_u64().unwrap_or(0) as usize;
        self.needs_config = data["needs_configuration"].as_bool().unwrap_or(true);
    }

    async fn run(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    ) -> Result<()> {
        let mut reader = EventStream::new();

        loop {
            if self.should_quit {
                break;
            }

            terminal.draw(|f| self.render(f))?;

            match &mut self.pending_rx {
                Some(rx) => {
                    tokio::select! {
                        event = rx => {
                            match event {
                                Ok(WorkerResult::Response(resp)) => self.handle_response(resp).await,
                                Err(_) => {
                                    self.status = Status::Ready;
                                    self.pending_rx = None;
                                    self.add_msg("error", "Operation cancelled.");
                                }
                            }
                        }
                        Some(Ok(event)) = reader.next() => {
                            self.handle_event(event);
                        }
                    }
                }
                None => {
                    tokio::select! {
                        Some(Ok(event)) = reader.next() => {
                            self.handle_event(event);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn handle_event(&mut self, event: Event) {
        match event {
            Event::Mouse(m) => self.handle_mouse(m),
            Event::Key(key) => {
                if key.kind != KeyEventKind::Press {
                    return;
                }
                self.handle_key(key);
            }
            _ => {}
        }
    }

    fn handle_mouse(&mut self, m: crossterm::event::MouseEvent) {
        let hit = |app: &App| -> Option<Rect> {
            let popup = app.completion_popup_area?;
            if m.row > popup.y
                && m.row < popup.y + popup.height - 1
                && m.column > popup.x
                && m.column < popup.x + popup.width - 1
            {
                Some(popup)
            } else {
                None
            }
        };

        match m.kind {
            MouseEventKind::Moved => {
                if self.completion_visible
                    && let Some(popup) = hit(self)
                {
                    let filtered = self.filtered_completions();
                    let item_idx = self.completion_offset + (m.row - popup.y - 1) as usize;
                    if item_idx < filtered.len() {
                        self.completion_selected = item_idx;
                    }
                }
            }
            MouseEventKind::Down(_) => {
                if self.completion_visible
                    && let Some(popup) = hit(self)
                {
                    let filtered = self.filtered_completions();
                    let item_idx = self.completion_offset + (m.row - popup.y - 1) as usize;
                    if item_idx < filtered.len() {
                        self.completion_selected = item_idx;
                        let (name, _) = &filtered[item_idx];
                        self.completion_visible = false;
                        self.completion_offset = 0;
                        self.input.clear();
                        self.cursor = 0;
                        self.exec_cmd(format!("/{name}"));
                    }
                }
            }
            MouseEventKind::ScrollUp => {
                if self.completion_visible && hit(self).is_some() {
                    let count = self.filtered_completions().len();
                    if count > 0 {
                        self.completion_selected = self.completion_selected.saturating_sub(1);
                        self.sync_completion_scroll(count);
                    }
                } else {
                    self.auto_scroll = false;
                    self.scroll = self.scroll.saturating_add(3);
                }
            }
            MouseEventKind::ScrollDown => {
                if self.completion_visible && hit(self).is_some() {
                    let count = self.filtered_completions().len();
                    if count > 0 {
                        self.completion_selected =
                            (self.completion_selected + 1).min(count.saturating_sub(1));
                        self.sync_completion_scroll(count);
                    }
                } else {
                    self.scroll = self.scroll.saturating_sub(3);
                    if self.scroll == 0 || self.scroll > 10000 {
                        self.auto_scroll = true;
                    }
                }
            }
            _ => {}
        }
    }

    fn handle_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') if key.modifiers == KeyModifiers::CONTROL => {
                self.should_quit = true;
                return;
            }
            KeyCode::Char('c') if key.modifiers == KeyModifiers::CONTROL => {
                self.should_quit = true;
                return;
            }
            KeyCode::Char('l') if key.modifiers == KeyModifiers::CONTROL => {
                self.messages.retain(|m| m.role == "system");
                self.auto_scroll = true;
                return;
            }
            _ => {}
        }

        if self.completion_visible {
            self.handle_completion_key(key);
        } else {
            self.handle_normal_key(key);
        }
    }

    fn sync_completion_scroll(&mut self, total: usize) {
        let visible = 10;
        if total <= visible {
            self.completion_offset = 0;
        } else if self.completion_selected < self.completion_offset {
            self.completion_offset = self.completion_selected;
        } else if self.completion_selected >= self.completion_offset + visible {
            self.completion_offset = self
                .completion_selected
                .saturating_add(1)
                .saturating_sub(visible);
        }
    }

    fn handle_completion_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up => {
                let count = self.filtered_completions().len();
                self.completion_selected = if self.completion_selected == 0 {
                    count.saturating_sub(1)
                } else {
                    self.completion_selected - 1
                };
                self.sync_completion_scroll(count);
            }
            KeyCode::Down => {
                let count = self.filtered_completions().len();
                self.completion_selected = (self.completion_selected + 1) % count;
                self.sync_completion_scroll(count);
            }
            KeyCode::Tab | KeyCode::Enter => {
                let filtered = self.filtered_completions();
                if let Some((name, _)) = filtered.get(self.completion_selected) {
                    self.input.clear();
                    self.cursor = 0;
                    self.completion_offset = 0;
                    if matches!(key.code, KeyCode::Enter) {
                        self.completion_visible = false;
                        self.exec_cmd(format!("/{name}"));
                    } else {
                        self.input = format!("/{name} ");
                        self.cursor = self.input.len();
                        self.completion_visible = false;
                    }
                }
            }
            KeyCode::Esc => {
                self.completion_visible = false;
                self.completion_offset = 0;
            }
            KeyCode::Backspace => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                    self.input.remove(self.cursor);
                }
                self.update_completion();
            }
            KeyCode::Delete => {
                if self.cursor < self.input.len() {
                    self.input.remove(self.cursor);
                }
                self.update_completion();
            }
            KeyCode::Left => {
                self.completion_visible = false;
                self.completion_offset = 0;
                self.cursor = self.cursor.saturating_sub(1);
            }
            KeyCode::Right => {
                self.completion_visible = false;
                self.completion_offset = 0;
                if self.cursor < self.input.len() {
                    self.cursor += 1;
                }
            }
            KeyCode::Home => {
                self.completion_visible = false;
                self.completion_offset = 0;
                self.cursor = 0;
            }
            KeyCode::End => {
                self.completion_visible = false;
                self.completion_offset = 0;
                self.cursor = self.input.len();
            }
            KeyCode::Char(c) => {
                self.input.insert(self.cursor, c);
                self.cursor += 1;
                self.update_completion();
            }
            _ => {}
        }
    }

    fn handle_normal_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter => {
                let input = self.input.trim().to_string();
                if !input.is_empty() && self.pending_rx.is_none() {
                    self.input.clear();
                    self.cursor = 0;
                    if input.starts_with('/') {
                        self.exec_cmd(input);
                    } else {
                        self.send_chat(input);
                    }
                }
            }
            KeyCode::Char(c) => {
                self.input.insert(self.cursor, c);
                self.cursor += 1;
                self.update_completion();
            }
            KeyCode::Backspace => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                    self.input.remove(self.cursor);
                }
                self.update_completion();
            }
            KeyCode::Delete => {
                if self.cursor < self.input.len() {
                    self.input.remove(self.cursor);
                }
                self.update_completion();
            }
            KeyCode::Left => self.cursor = self.cursor.saturating_sub(1),
            KeyCode::Right => {
                if self.cursor < self.input.len() {
                    self.cursor += 1;
                }
            }
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = self.input.len(),
            KeyCode::Up => {
                if self.auto_scroll {
                    self.auto_scroll = false;
                    self.scroll = 1;
                } else {
                    self.scroll = self.scroll.saturating_add(1);
                }
            }
            KeyCode::Down => {
                if self.scroll > 0 {
                    self.scroll = self.scroll.saturating_sub(1);
                    if self.scroll == 0 {
                        self.auto_scroll = true;
                    }
                }
            }
            KeyCode::PageUp => {
                self.auto_scroll = false;
                self.scroll = self.scroll.saturating_add(10);
            }
            KeyCode::PageDown => {
                self.scroll = self.scroll.saturating_sub(10);
                if self.scroll == 0 || self.scroll > 10000 {
                    self.auto_scroll = true;
                }
            }
            _ => {}
        }
    }

    fn filtered_completions(&self) -> Vec<(String, String)> {
        let filter = self.input.strip_prefix('/').unwrap_or("");
        if filter.is_empty() {
            return self.commands.clone();
        }
        let mut results: Vec<_> = self
            .commands
            .iter()
            .filter(|(name, _)| name.starts_with(filter))
            .cloned()
            .collect();
        if "quit".starts_with(filter) || "exit".starts_with(filter) {
            let exit = ("exit".into(), "Exit the application".into());
            if !results.contains(&exit) {
                results.push(exit);
            }
        }
        results
    }

    fn update_completion(&mut self) {
        if self.input.starts_with('/') {
            let filtered = self.filtered_completions();
            if !filtered.is_empty() {
                self.completion_visible = true;
                if self.completion_selected >= filtered.len() {
                    self.completion_selected = 0;
                    self.completion_offset = 0;
                }
                self.sync_completion_scroll(filtered.len());
                return;
            }
        }
        self.completion_visible = false;
        self.completion_selected = 0;
        self.completion_offset = 0;
    }

    fn send_chat(&mut self, msg: String) {
        self.add_msg("user", &msg);
        let (tx, rx) = oneshot::channel();
        if self
            .worker_tx
            .send(WorkerMsg::Chat { message: msg, tx })
            .is_err()
        {
            self.add_msg("error", "Failed to send message to worker.");
            return;
        }
        self.pending_rx = Some(rx);
        self.status = Status::Busy;
    }

    fn exec_cmd(&mut self, cmd: String) {
        if matches!(cmd.trim(), "/exit" | "/quit") {
            self.should_quit = true;
            return;
        }
        self.add_msg("command", &cmd);
        let (tx, rx) = oneshot::channel();
        if self
            .worker_tx
            .send(WorkerMsg::Command { command: cmd, tx })
            .is_err()
        {
            self.add_msg("error", "Failed to send command to worker.");
            return;
        }
        self.pending_rx = Some(rx);
        self.status = Status::Busy;
    }

    async fn handle_response(&mut self, resp: ApiResponse) {
        self.status = Status::Ready;
        self.pending_rx = None;
        self.auto_scroll = true;

        if resp.success {
            if let Some(data) = resp.data {
                let content = data["content"].as_str();
                let output = data["output"].as_str();

                if let Some(text) = content {
                    self.add_msg("assistant", text);
                    if let Some(model) = data["model"].as_str() {
                        self.model = Some(model.to_string());
                    }
                } else if let Some(text) = output
                    && !text.is_empty()
                {
                    self.add_msg("result", text);
                }
            }
        } else {
            let msg = resp
                .error
                .map(|e| e.message)
                .unwrap_or_else(|| "Unknown error".into());
            self.add_msg("error", &msg);
        }

        self.refresh_state().await;

        if self.needs_config && self.messages.iter().all(|m| m.role != "info") {
            self.add_msg(
                "info",
                "No provider configured. Use /provider to select one.",
            );
        }
    }

    fn add_msg(&mut self, role: &str, content: &str) {
        self.messages.push(DisplayMessage {
            role: role.into(),
            content: content.into(),
        });
    }

    fn render(&mut self, f: &mut Frame) {
        let area = f.area();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(3)])
            .split(area);

        self.render_chat(f, chunks[0]);
        self.render_input(f, chunks[1]);
        self.render_completion_popup(f, chunks[0], chunks[1]);
    }

    fn render_chat(&self, f: &mut Frame, area: Rect) {
        if area.width < 3 || area.height < 3 {
            return;
        }

        let lines: Vec<Line> = self
            .messages
            .iter()
            .flat_map(|msg| {
                let style = match msg.role.as_str() {
                    "system" => Style::default().fg(Color::Cyan).dim(),
                    "user" => Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                    "assistant" => Style::default().fg(Color::White),
                    "command" => Style::default()
                        .fg(Color::Magenta)
                        .add_modifier(Modifier::BOLD),
                    "result" => Style::default().fg(Color::Yellow),
                    "error" => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    "info" => Style::default().fg(Color::Blue).italic(),
                    _ => Style::default().fg(Color::White),
                };

                let lines: Vec<Line> = msg
                    .content
                    .lines()
                    .map(|line| {
                        Line::from(Span::styled(
                            if line.is_empty() {
                                String::new()
                            } else {
                                format!(" {}", line)
                            },
                            style,
                        ))
                    })
                    .collect();

                if lines.is_empty() {
                    vec![Line::from("")]
                } else {
                    lines
                }
            })
            .collect();

        let total = lines.len();
        let visible = area.height as usize - 2;
        let max_offset = total.saturating_sub(visible);
        let offset = if self.auto_scroll || total <= visible {
            max_offset
        } else {
            max_offset.saturating_sub(self.scroll)
        };

        let p = Paragraph::new(Text::from(lines))
            .block(Block::default().borders(Borders::ALL).title(" Chat "))
            .scroll((offset as u16, 0))
            .wrap(Wrap { trim: false });

        f.render_widget(p, area);
    }

    fn render_input(&self, f: &mut Frame, area: Rect) {
        let is_busy = matches!(self.status, Status::Busy);
        let content = if is_busy {
            String::new()
        } else {
            self.input.clone()
        };
        let style = if is_busy {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default()
        };

        let p = Paragraph::new(content.as_str())
            .style(style)
            .block(Block::default().borders(Borders::ALL).title(" Input "));

        f.render_widget(p, area);

        if !is_busy {
            f.set_cursor_position((area.x + 1 + self.cursor as u16, area.y + 1));
        }
    }

    fn render_completion_popup(&mut self, f: &mut Frame, chat_area: Rect, input_area: Rect) {
        if !self.completion_visible {
            return;
        }

        let filtered = self.filtered_completions();
        if filtered.is_empty() {
            return;
        }

        let max_visible = 10.min(filtered.len());
        let popup_height = max_visible as u16 + 2;
        let popup_width = (chat_area.width.saturating_sub(4)).min(80);

        let popup_x = input_area.x;
        let popup_y = input_area.y.saturating_sub(popup_height);

        let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);
        self.completion_popup_area = Some(popup_area);

        // content_width excludes borders (2 chars)
        let content_width = popup_width.saturating_sub(2) as usize;
        let name_col_width = 24.min(content_width.saturating_sub(4));

        let end = (self.completion_offset + max_visible).min(filtered.len());
        let visible_slice = &filtered[self.completion_offset..end];

        let items: Vec<ListItem> = visible_slice
            .iter()
            .enumerate()
            .map(|(i, (name, desc))| {
                let abs_idx = self.completion_offset + i;
                let selected = abs_idx == self.completion_selected;
                let cmd = format!("/{}", name);
                let padded_cmd = format!(" {cmd:<name_col_width$}");

                let desc_avail = content_width.saturating_sub(name_col_width + 3);
                let desc_text = if desc_avail >= 4 && desc.len() > desc_avail {
                    format!("{}…", &desc[..desc_avail.saturating_sub(1)])
                } else if desc_avail >= 4 {
                    desc.to_string()
                } else {
                    String::new()
                };

                let base = Style::default();
                let (cmd_style, desc_style) = if selected {
                    (
                        base.bg(Color::Rgb(30, 60, 120))
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                        base.bg(Color::Rgb(30, 60, 120)).fg(Color::White),
                    )
                } else {
                    (
                        base.fg(Color::Cyan).add_modifier(Modifier::BOLD),
                        base.fg(Color::White),
                    )
                };

                ListItem::new(Line::from(vec![
                    Span::styled(padded_cmd, cmd_style),
                    Span::styled(desc_text, desc_style),
                ]))
            })
            .collect();

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Commands ")
            .border_style(Style::default().fg(Color::Cyan));

        let list = List::new(items).block(block);

        f.render_widget(Clear, popup_area);
        f.render_widget(list, popup_area);
    }
}

fn spawn_worker() -> tokio::sync::mpsc::UnboundedSender<WorkerMsg> {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<WorkerMsg>();

    tokio::spawn(async move {
        let mut api = BimoApi::new();
        while let Some(msg) = rx.recv().await {
            match msg {
                WorkerMsg::Chat { message, tx } => {
                    let resp = api
                        .chat(ChatRequest {
                            message,
                            session_id: None,
                        })
                        .await;
                    let _ = tx.send(WorkerResult::Response(resp));
                }
                WorkerMsg::Command { command, tx } => {
                    let resp = api.execute_command(CommandRequest { command }).await;
                    let _ = tx.send(WorkerResult::Response(resp));
                }
                WorkerMsg::ListCommands { tx } => {
                    let resp = api.help();
                    let _ = tx.send(WorkerResult::Response(resp));
                }
            }
        }
    });

    tx
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("bimo_tui=info")
        .with_target(false)
        .init();

    crossterm::terminal::enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    crossterm::execute!(
        stdout,
        crossterm::terminal::EnterAlternateScreen,
        crossterm::event::EnableMouseCapture
    )?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;

    let worker_tx = spawn_worker();
    let mut app = App::new(worker_tx);

    // Get initial state and commands
    {
        let (tx, rx) = oneshot::channel();
        if app
            .worker_tx
            .send(WorkerMsg::Command {
                command: "/status".into(),
                tx,
            })
            .is_ok()
            && let Ok(WorkerResult::Response(resp)) = rx.await
            && let Some(data) = resp.data
        {
            app.set_initial_state(&data);
        }
    }
    {
        let (tx, rx) = oneshot::channel();
        if app.worker_tx.send(WorkerMsg::ListCommands { tx }).is_ok()
            && let Ok(WorkerResult::Response(resp)) = rx.await
            && let Some(data) = resp.data
            && let Some(cmds) = data["commands"].as_array()
        {
            app.commands = cmds
                .iter()
                .filter_map(|c| {
                    Some((
                        c["name"].as_str()?.to_string(),
                        c["description"].as_str()?.to_string(),
                    ))
                })
                .collect();
        }
        app.commands
            .push(("exit".into(), "Exit the application".into()));
    }

    app.boot_message();
    if app.needs_config {
        app.add_msg(
            "info",
            "No provider configured. Use /provider to select one.",
        );
    }

    let result = app.run(&mut terminal).await;

    crossterm::execute!(
        terminal.backend_mut(),
        crossterm::terminal::LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture
    )?;
    crossterm::terminal::disable_raw_mode()?;

    if let Err(e) = result {
        eprintln!("Error: {e}");
    }

    Ok(())
}
