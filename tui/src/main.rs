use bimo_api::BimoApi;
use bimo_api::api::dto::{ApiResponse, ChatRequest, CommandRequest};
use crossterm::event::{Event, EventStream, KeyCode, KeyEventKind, KeyModifiers};
use futures::StreamExt;
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
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
        if let Ok(WorkerResult::Response(resp)) = rx.await {
            if let Some(data) = resp.data {
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
        let Event::Key(key) = event else { return };
        if key.kind != KeyEventKind::Press {
            return;
        }

        match key.code {
            KeyCode::Char('q') if key.modifiers == KeyModifiers::CONTROL => {
                self.should_quit = true;
            }
            KeyCode::Char('c') if key.modifiers == KeyModifiers::CONTROL => {
                self.should_quit = true;
            }
            KeyCode::Char('l') if key.modifiers == KeyModifiers::CONTROL => {
                self.messages.retain(|m| m.role == "system");
                self.auto_scroll = true;
            }
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
            }
            KeyCode::Backspace => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                    self.input.remove(self.cursor);
                }
            }
            KeyCode::Delete => {
                if self.cursor < self.input.len() {
                    self.input.remove(self.cursor);
                }
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
                } else if let Some(text) = output {
                    if !text.is_empty() {
                        self.add_msg("result", text);
                    }
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

    fn render(&self, f: &mut Frame) {
        let area = f.area();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(3)])
            .split(area);

        self.render_chat(f, chunks[0]);
        self.render_input(f, chunks[1]);
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
    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;

    let worker_tx = spawn_worker();
    let mut app = App::new(worker_tx);

    // Get initial state via a status command
    {
        let (tx, rx) = oneshot::channel();
        if app
            .worker_tx
            .send(WorkerMsg::Command {
                command: "/status".into(),
                tx,
            })
            .is_ok()
        {
            if let Ok(WorkerResult::Response(resp)) = rx.await {
                if let Some(data) = resp.data {
                    app.set_initial_state(&data);
                }
            }
        }
    }

    app.boot_message();
    if app.needs_config {
        app.add_msg(
            "info",
            "No provider configured. Use /provider to select one.",
        );
    }

    let result = app.run(&mut terminal).await;

    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(
        terminal.backend_mut(),
        crossterm::terminal::LeaveAlternateScreen
    )?;

    if let Err(e) = result {
        eprintln!("Error: {e}");
    }

    Ok(())
}
