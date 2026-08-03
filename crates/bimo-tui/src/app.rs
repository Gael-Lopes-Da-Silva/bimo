use bimo_core::{Agent, ApiFormat, Provider, Session};
use cursive::view::Resizable;
use cursive::views::{DummyView, LinearLayout};
use cursive::{Cursive, CursiveExt};
use tokio::sync::broadcast;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::config::load_theme;
use crate::events::EventBridge;
use crate::input::input_area::INPUT_MIN_HEIGHT;
use crate::output;
use crate::theme::{BimoTheme, ThemeColors};

/// Application-wide state stored in the `Cursive` user data.
///
/// The streaming event handler and the input area both read and update this
/// state while the agent runs.
pub struct AppState {
    /// Channel to send user prompts to the agent loop.
    pub prompt_tx: UnboundedSender<String>,
    /// Current theme colors, used to style new message views.
    pub colors: ThemeColors,
    /// Name of the assistant message currently being streamed.
    pub current_assistant: Option<String>,
    /// Name of the reasoning message currently being streamed.
    pub current_reasoning: Option<String>,
    /// Id of the tool-call box waiting for its result.
    pub current_tool: Option<usize>,
    /// Counter used to generate unique view names.
    pub next_id: usize,
    /// Current height (in lines) of the input area.
    pub input_height: usize,
}

pub struct App {
    siv: Cursive,
    session: Session,
    theme: BimoTheme,
}

impl App {
    pub fn new(session: Session, theme_name: Option<&str>) -> Result<Self, crate::error::Error> {
        let theme = load_theme(theme_name)?;

        let mut siv = Cursive::new();
        siv.set_theme(theme.to_cursive_theme());

        let mut app = Self {
            siv,
            session,
            theme,
        };

        app.setup_ui();
        crate::input::keybindings::setup_global_keybindings(&mut app.siv);

        Ok(app)
    }

    fn setup_ui(&mut self) {
        let layout = crate::layout::create_main_layout();
        self.siv.add_layer(layout);
        self.render_session_history();
    }

    fn render_session_history(&mut self) {
        let colors = self.theme.colors.clone();
        let history = self.session.messages.clone();
        self.siv
            .call_on_name("messages", |messages: &mut LinearLayout| {
                for message in &history {
                    let view = match message.role.as_str() {
                        "user" => output::message_view::user_message(&message.content, &colors),
                        "assistant" => {
                            output::message_view::assistant_message(&message.content, &colors)
                        }
                        "tool" => output::message_view::system_message(&message.content, &colors),
                        _ => output::message_view::system_message(&message.content, &colors),
                    };
                    messages.add_child(view);
                    messages.add_child(DummyView::new().max_height(1));
                }
            });
    }

    pub fn run(mut self) -> Result<(), crate::error::Error> {
        let (prompt_tx, prompt_rx) = tokio::sync::mpsc::unbounded_channel::<String>();

        let state = AppState {
            colors: self.theme.colors.clone(),
            current_assistant: None,
            current_reasoning: None,
            current_tool: None,
            next_id: 0,
            input_height: INPUT_MIN_HEIGHT,
            prompt_tx,
        };
        self.siv.set_user_data(state);

        let rt = tokio::runtime::Runtime::new()?;
        let session = self.session.clone();
        let cb_sink = self.siv.cb_sink().clone();
        rt.spawn(Self::prompt_loop(prompt_rx, session, cb_sink));

        self.siv.run();
        rt.shutdown_background();
        Ok(())
    }

    async fn prompt_loop(
        mut prompt_rx: UnboundedReceiver<String>,
        session: Session,
        cb_sink: cursive::CbSink,
    ) {
        while let Some(prompt) = prompt_rx.recv().await {
            match run_agent_once(&session, &prompt).await {
                Ok(rx) => {
                    EventBridge::new(cb_sink.clone(), rx).spawn();
                }
                Err(e) => {
                    let message = format!("Error starting agent: {e}");
                    let sink = cb_sink.clone();
                    sink.send(Box::new(move |siv: &mut Cursive| {
                        let view = output::message_view::error_message(&message, &colors_from(siv));
                        output::scroll::add_child(siv, view);
                    }))
                    .ok();
                }
            }
        }
    }

    pub fn session(&self) -> &Session {
        &self.session
    }
}

fn colors_from(siv: &mut Cursive) -> ThemeColors {
    siv.user_data::<AppState>()
        .map(|state| state.colors.clone())
        .unwrap_or_default()
}

async fn run_agent_once(
    session: &Session,
    prompt: &str,
) -> Result<broadcast::Receiver<bimo_core::AgentEvent>, crate::error::Error> {
    let api_key = std::env::var("ANTHROPIC_API_KEY").ok();
    let mut provider = Provider::cloud(
        "anthropic",
        "Anthropic",
        "https://api.anthropic.com",
        ApiFormat::Anthropic,
    );
    provider.api_key = api_key;

    let model =
        std::env::var("BIMO_MODEL").unwrap_or_else(|_| "claude-sonnet-4-20250514".to_string());

    let mut agent = Agent::builder()
        .with_session(session.clone())
        .with_provider(provider)
        .with_model(model)
        .with_user_prompt(prompt.to_string())
        .build()?;

    let (rx, steer_tx) = agent.run_steerable().await?;
    drop(steer_tx);
    Ok(rx)
}

pub fn run_tui(session: Session, theme: Option<&str>) -> Result<(), crate::error::Error> {
    let app = App::new(session, theme)?;
    app.run()
}
