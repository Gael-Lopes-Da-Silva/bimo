use std::path::Path;

use bimo_core::config::SettingsConfig;
use bimo_core::session::SessionManager;
use bimo_core::skill;
use bimo_core::tools;

use crate::cli::{CleanupArgs, SkillsCommand, ToolsCommand};
use crate::output;

pub async fn tools_run(json: bool, sub: &ToolsCommand) -> crate::Result<()> {
    match sub {
        ToolsCommand::List => tools_list(json),
    }
}

fn tools_list(json: bool) -> crate::Result<()> {
    if json {
        return output::emit_json_array(&tools::tool_names());
    }
    let desc = tools::describe_tools(&std::collections::BTreeSet::new());
    println!("Built-in tools:");
    for line in desc.lines() {
        println!("  {line}");
    }
    let names = tools::tool_names();
    println!("\n{} tools total", names.len());
    Ok(())
}

pub async fn skills_run(json: bool, sub: &SkillsCommand) -> crate::Result<()> {
    match sub {
        SkillsCommand::List { project_dir } => skills_list(json, project_dir.as_deref()).await,
    }
}

async fn skills_list(json: bool, project_dir: Option<&Path>) -> crate::Result<()> {
    let dirs = skill::default_skill_dirs(project_dir);
    let skills = skill::load_skills(&dirs);
    if json {
        let entries: Vec<serde_json::Value> = skills
            .iter()
            .map(|s| {
                serde_json::json!({
                    "id": s.id,
                    "name": s.name,
                    "description": s.description,
                    "enabled": s.enabled,
                    "path": s.path,
                })
            })
            .collect();
        return output::emit_json_array(&entries);
    }
    if skills.is_empty() {
        println!("No skills found.");
        return Ok(());
    }
    for s in &skills {
        let state = if s.enabled { "enabled" } else { "disabled" };
        println!(
            "{:<28} {:<32} {:<10} {}",
            s.id,
            output::truncate(&s.name, 32),
            state,
            s.path.display(),
        );
    }
    println!("{} skills", skills.len());
    Ok(())
}

pub async fn cleanup_run(args: &CleanupArgs) -> crate::Result<()> {
    let mut settings = SettingsConfig::load()?;
    if let Some(ttl) = args.ttl {
        settings.session_ttl_hours = ttl;
    }
    if let Some(max) = args.max {
        settings.max_sessions = max;
    }
    let manager = SessionManager::new(settings).await?;
    manager.run_cleanup_now().await;
    let remaining = manager.list().await;
    println!("Cleanup complete: {} sessions remain", remaining.len());
    Ok(())
}

pub fn config_path() -> crate::Result<()> {
    println!("{}", bimo_core::paths::config_dir().display());
    Ok(())
}
