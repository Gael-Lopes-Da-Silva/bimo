pub mod app;
pub mod components;
pub mod event;
pub mod layouts;
pub mod state;
pub mod theme;
pub mod widgets;

use anyhow::Result;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers, poll, read},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io;
use tokio::sync::{broadcast, mpsc};

use crate::app::App;
use crate::state::app_state::AppState;

pub async fn run() -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut app_state = AppState::new();

    // Initialize with default theme
    let styles = crate::theme::Styles::from_theme(&crate::theme::Theme::mocha());
    app_state.layout = app_state.layout.styles(styles);

    // Load sessions
    load_sessions(&mut app_state).await;

    // Create event channels
    let (event_tx, mut event_rx) = mpsc::channel::<crate::event::AppEvent>(1024);
    let (agent_event_tx, agent_event_rx) = broadcast::channel::<bimo_core::AgentEvent>(256);
    let (steer_tx, steer_rx) = mpsc::channel::<bimo_core::SteerCommand>(64);

    app_state.set_agent_channels(agent_event_tx, steer_tx);

    // Spawn event handler
    let event_tx_clone = event_tx.clone();
    let mut app_state_clone = app_state.clone();
    tokio::spawn(async move {
        let mut last_tick = std::time::Instant::now();
        loop {
            // Handle crossterm events
            if poll(std::time::Duration::from_millis(16)).unwrap_or(false) {
                if let Ok(event) = read() {
                    let app_event =
                        crate::event::EventHandler::new().handle_event(event, &mut app_state_clone);
                    if app_event != crate::event::AppEvent::None {
                        if event_tx_clone.send(app_event).await.is_err() {
                            break;
                        }
                    }
                }
            }

            // Tick for animations
            if last_tick.elapsed() >= std::time::Duration::from_millis(16) {
                if event_tx_clone
                    .send(crate::event::AppEvent::Tick)
                    .await
                    .is_err()
                {
                    break;
                }
                last_tick = std::time::Instant::now();
            }

            // Check for quit
            if app_state_clone.should_quit {
                break;
            }
        }
    });

    // Spawn agent event handler
    let event_tx_agent = event_tx.clone();
    tokio::spawn(async move {
        let mut rx = agent_event_rx;
        while let Ok(event) = rx.recv().await {
            if event_tx_agent
                .send(crate::event::AppEvent::AgentEvent(event))
                .await
                .is_err()
            {
                break;
            }
        }
    });

    // Main event loop
    let mut app = App::new(app_state, terminal);
    app.run(event_rx).await?;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        app.terminal().backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;

    Ok(())
}

async fn load_sessions(app_state: &mut AppState) {
    use bimo_core::config::SettingsConfig;
    use bimo_core::session::SessionManager;

    let settings = SettingsConfig::load().unwrap_or_default();
    if let Ok(manager) = SessionManager::new(settings).await {
        let sessions = manager.list().await;
        let session_items: Vec<crate::components::sidebar::SessionItem> = sessions
            .into_iter()
            .map(|s| crate::components::sidebar::SessionItem {
                id: s.id.clone(),
                name: s
                    .metadata
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unnamed")
                    .to_string(),
                model: s
                    .metadata
                    .get("model")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown")
                    .to_string(),
                updated: s.updated_at.format("%Y-%m-%d %H:%M").to_string(),
                message_count: s.messages.len(),
                is_active: false,
            })
            .collect();

        app_state.layout.sidebar_mut().set_sessions(session_items);
        app_state.session_manager = Some(std::sync::Arc::new(tokio::sync::RwLock::new(manager)));
    }
}
