use bimo_core::{Agent, ApiFormat, Provider, Session};
use cursive::Cursive;
use cursive::CursiveExt;
use cursive::view::Nameable;
use cursive::views::{DummyView, EditView, LinearLayout, Panel, ScrollView, TextView};
use tokio::sync::broadcast;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::config::load_theme;
use crate::events::EventBridge;

pub struct App {
    siv: Cursive,
    session: Session,
}

impl App {
    pub fn new(session: Session) -> Result<Self, crate::error::Error> {
        let theme = load_theme(None)?;

        let mut siv = Cursive::new();
        siv.set_theme(theme.to_cursive_theme());

        let mut app = Self { siv, session };

        app.setup_ui();
        crate::input::keybindings::setup_global_keybindings(&mut app.siv);

        Ok(app)
    }

    fn setup_ui(&mut self) {
        let messages = LinearLayout::vertical().with_name("messages");
        let output_area = ScrollView::new(messages).with_name("output_area");

        let input_area = EditView::new()
            .on_submit(|siv, content| {
                let content = content.trim().to_string();
                if content.is_empty() {
                    return;
                }
                add_user_message(siv, &content);
                if let Some(tx) = siv.user_data::<UnboundedSender<String>>() {
                    let _ = tx.send(content);
                }
                siv.call_on_name("input", |input: &mut EditView| {
                    input.set_content("");
                });
            })
            .with_name("input");

        let layout = LinearLayout::vertical()
            .child(output_area)
            .child(input_area);

        self.siv.add_layer(layout);
    }

    pub fn run(mut self) -> Result<(), crate::error::Error> {
        let (prompt_tx, prompt_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        self.siv.set_user_data(prompt_tx);

        let rt = tokio::runtime::Runtime::new()?;
        let session = self.session.clone();
        let cb_sink = self.siv.cb_sink().clone();
        rt.spawn(Self::prompt_loop(prompt_rx, session, cb_sink));

        self.siv.run();
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
                    let msg = format!("Error starting agent: {e}");
                    let sink = cb_sink.clone();
                    sink.send(Box::new(move |siv: &mut Cursive| {
                        add_error_message(siv, &msg);
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

fn add_user_message(siv: &mut Cursive, content: &str) {
    siv.call_on_name("messages", |messages: &mut LinearLayout| {
        let panel = Panel::new(TextView::new(content.to_string())).title("You");
        messages.add_child(panel);
        messages.add_child(DummyView);
    });
}

fn add_error_message(siv: &mut Cursive, content: &str) {
    siv.call_on_name("messages", |messages: &mut LinearLayout| {
        let panel = Panel::new(TextView::new(content.to_string())).title("Error");
        messages.add_child(panel);
        messages.add_child(DummyView);
    });
}

pub fn run_tui(session: Session) -> Result<(), crate::error::Error> {
    let app = App::new(session)?;
    app.run()
}
