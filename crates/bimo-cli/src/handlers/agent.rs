use std::io::Write;

use bimo_core::config::{ProvidersConfig, SettingsConfig};
use bimo_core::error::CustomError;
use bimo_core::providers::CloudProviderRegistry;
use bimo_core::session::Session;
use bimo_core::{Agent, AgentEvent, Provider};

use crate::cli::{AgentArgs, RunArgs};
use crate::output;

pub async fn run(json: bool, args: &RunArgs) -> crate::Result<()> {
    let session = match &args.session {
        Some(id) => Session::load(id)
            .map_err(|e| CustomError::Session(format!("Cannot resume session {id}: {e}")))?,
        None => Session::new(),
    };
    let mut session = session;
    if let Some(name) = &args.name {
        set_metadata(&mut session, "name", name);
        session.save()?;
    }
    execute_prompt(json, &session, &args.prompt, &args.agent).await
}

pub async fn send(id: &str, message: &str, agent_args: &AgentArgs) -> crate::Result<()> {
    let session =
        Session::load(id).map_err(|e| CustomError::Session(format!("Session {id}: {e}")))?;
    execute_prompt(false, &session, message, agent_args).await
}

pub async fn title(json: bool, id: &str, agent_args: &AgentArgs) -> crate::Result<()> {
    let session =
        Session::load(id).map_err(|e| CustomError::Session(format!("Session {id}: {e}")))?;
    let settings = SettingsConfig::load()?;
    let provider = resolve_provider(agent_args.provider.as_deref()).await?;
    let model = resolve_model(agent_args.model.as_deref(), &provider, &settings).await;

    let mut agent = Agent::builder()
        .with_settings(settings)
        .with_provider(provider)
        .with_model(model)
        .with_session(session)
        .with_user_prompt(String::new())
        .build()?;

    let name = agent.generate_session_name().await?;
    let mut session = agent.session;
    set_metadata(&mut session, "name", &name);
    session.save()?;

    if json {
        return output::emit_json(&serde_json::json!({ "session": session.id, "name": name }));
    }
    println!("{name}");
    Ok(())
}

async fn execute_prompt(
    json: bool,
    session: &Session,
    prompt: &str,
    agent_args: &AgentArgs,
) -> crate::Result<()> {
    let mut settings = SettingsConfig::load()?;
    if let Some(s) = agent_args.snapshots {
        settings.snapshots = s;
    }
    let provider = resolve_provider(agent_args.provider.as_deref()).await?;
    let model = resolve_model(agent_args.model.as_deref(), &provider, &settings).await;
    let project_dir = agent_args.project_dir.clone().unwrap_or_else(|| {
        std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
    });

    let mut builder = Agent::builder()
        .with_settings(settings)
        .with_provider(provider.clone())
        .with_model(model.clone())
        .with_session(session.clone())
        .with_project_dir(project_dir)
        .with_user_prompt(prompt.to_string());
    if let Some(t) = agent_args.temperature {
        builder = builder.with_temperature(t);
    }
    if let Some(t) = agent_args.max_tokens {
        builder = builder.with_max_tokens(t);
    }
    if let Some(s) = agent_args.max_steps {
        builder = builder.with_max_steps(s);
    }
    if let Some(r) = agent_args.reasoning_effort {
        builder = builder.with_reasoning_effort(r.into());
    }
    if let Some(a) = agent_args.retry_attempts {
        builder = builder.with_retry_attempts(a);
    }
    if let Some(t) = agent_args.retry_timeout {
        builder = builder.with_retry_timeout(t);
    }

    let mut agent = builder.build()?;
    let mut rx = agent.run().await?;
    stream_events(&mut rx).await?;

    let final_session = Session::load(&agent.session.id).unwrap_or_else(|_| agent.session.clone());
    if json {
        return output::emit_json(&serde_json::json!({
            "session": final_session.id,
            "provider": provider.name,
            "model": model,
        }));
    }
    println!(
        "[session] {} ({} / {})",
        final_session.id, provider.name, model
    );
    Ok(())
}

async fn stream_events(rx: &mut tokio::sync::broadcast::Receiver<AgentEvent>) -> crate::Result<()> {
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    while let Ok(event) = rx.recv().await {
        match event {
            AgentEvent::TextDelta(t) => {
                let _ = out.write_all(t.as_bytes());
                let _ = out.flush();
            }
            AgentEvent::ReasoningDelta(r) => {
                eprintln!("\x1b[2m{r}\x1b[0m");
            }
            AgentEvent::ToolCallStart { tool_name, args } => {
                eprintln!("\n[tool] {tool_name} {args}");
            }
            AgentEvent::ToolCallEnd { tool_name, result } => match result {
                Ok(_) => eprintln!("[ok] {tool_name}"),
                Err(e) => eprintln!("[failed] {tool_name}: {e}"),
            },
            AgentEvent::Steering(t) => eprintln!("\n[steer] {t}"),
            AgentEvent::Retrying { attempt, error } => {
                eprintln!("[retry {attempt}] {error}");
            }
            AgentEvent::Error(e) => {
                eprintln!("[error] {e}");
                return Err(CustomError::Agent(e));
            }
            AgentEvent::Done => break,
        }
    }
    Ok(())
}

async fn resolve_provider(id: Option<&str>) -> crate::Result<Provider> {
    let providers_config = ProvidersConfig::load()?;
    let settings = SettingsConfig::load()?;
    let registry = CloudProviderRegistry::new();
    let _ = registry.load().await;

    let selected = id
        .map(str::to_string)
        .or_else(|| settings.default_provider.clone())
        .or_else(|| providers_config.default.clone());

    let Some(selected) = selected else {
        return Err(CustomError::Config(
            "No provider selected. Pass --provider, or configure a default via \
             `bimo provider set-default <id>` or `bimo settings set default_provider <id>`"
                .to_string(),
        ));
    };

    let mut provider = registry
        .resolve_provider(&selected, &providers_config.providers)
        .await
        .ok_or_else(|| CustomError::Config(format!("Provider '{selected}' not found")))?;
    registry
        .resolve_base_urls(std::slice::from_mut(&mut provider))
        .await;
    Ok(provider)
}

async fn resolve_model(id: Option<&str>, provider: &Provider, settings: &SettingsConfig) -> String {
    id.map(str::to_string)
        .or_else(|| settings.default_model.clone())
        .unwrap_or_else(|| provider.id.clone())
}

pub(crate) fn set_metadata(session: &mut Session, key: &str, value: &str) {
    if !session.metadata.is_object() {
        session.metadata = serde_json::json!({});
    }
    session.metadata[key] = serde_json::json!(value);
}
