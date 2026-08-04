use std::path::Path;

use bimo_core::config::SettingsConfig;
use bimo_core::error::CustomError;
use bimo_core::session::{Session, SessionManager};
use serde::Serialize;

use crate::cli::{ExportFormat, SessionCommand};
use crate::output;

pub async fn run(json: bool, sub: &SessionCommand) -> crate::Result<()> {
    match sub {
        SessionCommand::New { name } => new(json, name.as_deref()).await,
        SessionCommand::List => list(json).await,
        SessionCommand::Show { id, full } => show(json, id, *full).await,
        SessionCommand::Delete { id } => delete(id).await,
        SessionCommand::Fork { id } => fork(json, id).await,
        SessionCommand::Clear { id } => clear(id).await,
        SessionCommand::Export { id, format, output } => {
            export(id, *format, output.as_deref()).await
        }
        SessionCommand::Rename { id, name } => rename(id, name).await,
        SessionCommand::Title { id, agent } => crate::handlers::agent::title(json, id, agent).await,
        SessionCommand::Undo { id, message_id } => undo(id, message_id.as_deref()).await,
        SessionCommand::Redo { id } => redo(id).await,
        SessionCommand::Restore { id, batch } => restore(id, *batch).await,
    }
}

async fn session_manager() -> crate::Result<SessionManager> {
    let settings = SettingsConfig::load()?;
    SessionManager::new(settings).await
}

fn load(id: &str) -> crate::Result<Session> {
    Session::load(id).map_err(|e| CustomError::Session(format!("Session {id}: {e}")))
}

#[derive(Serialize)]
struct SessionSummary {
    id: String,
    name: Option<String>,
    created_at: String,
    updated_at: String,
    messages: usize,
    provider: Option<String>,
    model: Option<String>,
}

fn summarize(session: &Session) -> SessionSummary {
    SessionSummary {
        id: session.id.clone(),
        name: session
            .metadata
            .get("name")
            .and_then(|v| v.as_str())
            .map(String::from),
        created_at: session.created_at.to_string(),
        updated_at: session.updated_at.to_string(),
        messages: session.messages.len(),
        provider: session
            .metadata
            .get("provider")
            .and_then(|v| v.as_str())
            .map(String::from),
        model: session
            .metadata
            .get("model")
            .and_then(|v| v.as_str())
            .map(String::from),
    }
}

async fn new(json: bool, name: Option<&str>) -> crate::Result<()> {
    let manager = session_manager().await?;
    let mut session = manager.create().await?;
    if let Some(name) = name {
        crate::handlers::agent::set_metadata(&mut session, "name", name);
        manager.update(&session).await?;
    }
    if json {
        return output::emit_json(&session);
    }
    println!("{}", session.id);
    Ok(())
}

async fn list(json: bool) -> crate::Result<()> {
    let manager = session_manager().await?;
    let sessions = manager.list().await;
    if json {
        let entries: Vec<SessionSummary> = sessions.iter().map(summarize).collect();
        return output::emit_json_array(&entries);
    }
    if sessions.is_empty() {
        println!("No sessions.");
        return Ok(());
    }
    for s in &sessions {
        let name = s
            .metadata
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("-");
        println!(
            "{:<38} {:<24} {:<6} msgs  {}",
            s.id,
            output::truncate(name, 24),
            s.messages.len(),
            s.updated_at,
        );
    }
    Ok(())
}

async fn show(json: bool, id: &str, full: bool) -> crate::Result<()> {
    let session = load(id)?;
    if json {
        return output::emit_json(&session);
    }
    println!("id: {}", session.id);
    if let Some(name) = session.metadata.get("name").and_then(|v| v.as_str()) {
        println!("name: {name}");
    }
    println!("created_at: {}", session.created_at);
    println!("updated_at: {}", session.updated_at);
    if let Some(p) = session.metadata.get("provider").and_then(|v| v.as_str()) {
        println!("provider: {p}");
    }
    if let Some(m) = session.metadata.get("model").and_then(|v| v.as_str()) {
        println!("model: {m}");
    }

    println!("\nMessages ({}):", session.messages.len());
    for (i, m) in session.messages.iter().enumerate() {
        let content = if full {
            m.content.clone()
        } else {
            output::truncate(&m.content, 120)
        };
        println!("  [{:3}] {} {}", i, m.role, m.timestamp);
        for line in content.lines() {
            println!("        {line}");
        }
    }

    if !session.archived_messages.is_empty() {
        let total: usize = session.archived_messages.iter().map(|b| b.len()).sum();
        println!(
            "\nArchived messages: {total} in {} batches",
            session.archived_messages.len()
        );
    }
    if !session.todo_list.items.is_empty() {
        println!("\nTodos ({}):", session.todo_list.items.len());
        for t in &session.todo_list.items {
            println!(
                "  - [{:?}] {:?} {}",
                t.status,
                t.priority,
                output::truncate(&t.description, 80)
            );
        }
    }
    if !session.disabled_tools.is_empty() {
        println!(
            "\nDisabled tools: {}",
            session
                .disabled_tools
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    if !session.disabled_skills.is_empty() {
        println!(
            "\nDisabled skills: {}",
            session
                .disabled_skills
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    Ok(())
}

async fn delete(id: &str) -> crate::Result<()> {
    let manager = session_manager().await?;
    if manager.get(id).await.is_none() {
        return Err(CustomError::Session(format!("Session {id} not found")));
    }
    manager.delete(id).await?;
    println!("Deleted session {id}");
    Ok(())
}

async fn fork(json: bool, id: &str) -> crate::Result<()> {
    let manager = session_manager().await?;
    let fork = manager.fork(id).await?;
    if json {
        return output::emit_json(&fork);
    }
    println!("{}", fork.id);
    Ok(())
}

async fn clear(id: &str) -> crate::Result<()> {
    let mut session = load(id)?;
    session.clear_messages();
    session.save()?;
    println!("Cleared session {id}");
    Ok(())
}

async fn export(id: &str, format: ExportFormat, output_path: Option<&Path>) -> crate::Result<()> {
    let session = load(id)?;
    let content = match format {
        ExportFormat::Json => serde_json::to_string_pretty(&session)?,
        ExportFormat::Md => render_markdown(&session),
    };
    match output_path {
        Some(path) => std::fs::write(path, content)?,
        None => println!("{content}"),
    }
    Ok(())
}

fn render_markdown(session: &Session) -> String {
    let mut md = format!(
        "# Session {}\n\nCreated: {}\nUpdated: {}\n\n",
        session.id, session.created_at, session.updated_at
    );
    md.push_str("## Messages\n\n");
    for msg in &session.messages {
        md.push_str(&format!(
            "- **[{}] {}**: {}\n",
            msg.role, msg.timestamp, msg.content
        ));
    }
    if !session.archived_messages.is_empty() {
        md.push_str("\n## Archived messages\n\n");
        for batch in &session.archived_messages {
            for msg in batch {
                md.push_str(&format!(
                    "- **[{}] {}**: {}\n",
                    msg.role, msg.timestamp, msg.content
                ));
            }
            md.push('\n');
        }
    }
    md
}

async fn rename(id: &str, name: &str) -> crate::Result<()> {
    let mut session = load(id)?;
    crate::handlers::agent::set_metadata(&mut session, "name", name);
    session.save()?;
    println!("Session {id} renamed to '{name}'");
    Ok(())
}

async fn undo(id: &str, message_id: Option<&str>) -> crate::Result<()> {
    let mut session = load(id)?;
    session.undo(message_id)?;
    println!("Undid prompt in session {id}");
    Ok(())
}

async fn redo(id: &str) -> crate::Result<()> {
    let mut session = load(id)?;
    session.redo(None)?;
    println!("Redid prompt in session {id}");
    Ok(())
}

async fn restore(id: &str, batch: Option<usize>) -> crate::Result<()> {
    let mut session = load(id)?;
    let restored = match batch {
        Some(b) => session.restore_archived_batch(b).ok_or_else(|| {
            CustomError::Session(format!("No archived batch {b} in session {id}"))
        })?,
        None => session.restore_all_archived(),
    };
    session.save()?;
    println!("Restored {} messages in session {id}", restored.len());
    Ok(())
}
