use bimo_core::error::CustomError;
use bimo_core::models::{ModelEntry, ModelRegistry};
use bimo_core::providers::CloudProviderRegistry;

use crate::cli::ModelCommand;
use crate::output;

pub async fn run(json: bool, sub: &ModelCommand) -> crate::Result<()> {
    match sub {
        ModelCommand::List { provider, refresh } => list(json, provider.as_deref(), *refresh).await,
        ModelCommand::Show { model_id } => show(json, model_id).await,
    }
}

async fn load_registry(refresh: bool) -> crate::Result<CloudProviderRegistry> {
    let registry = CloudProviderRegistry::new();
    if refresh {
        registry.refresh().await?;
    } else {
        registry.load().await?;
    }
    Ok(registry)
}

async fn list(json: bool, provider: Option<&str>, refresh: bool) -> crate::Result<()> {
    let registry = load_registry(refresh).await?;
    let model_registry = ModelRegistry::from_registry(&registry);

    let mut all: Vec<(String, ModelEntry)> = Vec::new();
    match provider {
        Some(pid) => {
            for m in model_registry.list_models(pid).await {
                all.push((pid.to_string(), m));
            }
        }
        None => {
            for p in registry.list_providers().await {
                for m in model_registry.list_models(&p.id).await {
                    all.push((p.id.clone(), m));
                }
            }
        }
    }

    if json {
        let entries: Vec<serde_json::Value> = all
            .iter()
            .map(|(pid, m)| serde_json::json!({ "provider": pid, "model": m }))
            .collect();
        return output::emit_json_array(&entries);
    }

    if all.is_empty() {
        println!("(no models found)");
        return Ok(());
    }
    for (pid, m) in &all {
        let context = m
            .limit
            .as_ref()
            .and_then(|l| l.context)
            .map(|c| c.to_string())
            .unwrap_or_else(|| "-".to_string());
        println!(
            "{:<24} {:<48} ctx={}",
            pid,
            output::truncate(&m.id, 48),
            context
        );
    }
    println!("Total: {} models", all.len());
    Ok(())
}

async fn show(json: bool, model_id: &str) -> crate::Result<()> {
    let registry = load_registry(false).await?;
    let model_registry = ModelRegistry::from_registry(&registry);
    let (pid, model) = model_registry
        .find_model(model_id)
        .await
        .ok_or_else(|| CustomError::Msg(format!("Model '{model_id}' not found")))?;

    if json {
        return output::emit_json(&serde_json::json!({ "provider": pid, "model": model }));
    }

    println!("model: {}", model.id);
    println!("provider: {pid}");
    println!("name: {}", model.name);
    if let Some(d) = &model.description {
        println!("description: {d}");
    }
    if let Some(f) = &model.family {
        println!("family: {f}");
    }
    if let Some(l) = &model.limit {
        println!(
            "context: {}",
            l.context
                .map(|c| c.to_string())
                .unwrap_or_else(|| "-".to_string())
        );
        println!(
            "output: {}",
            l.output
                .map(|c| c.to_string())
                .unwrap_or_else(|| "-".to_string())
        );
    }
    if let Some(c) = &model.cost {
        println!(
            "cost ($/MTok): in={} out={} cache_read={} cache_write={}",
            fmt_opt(c.input),
            fmt_opt(c.output),
            fmt_opt(c.cache_read),
            fmt_opt(c.cache_write),
        );
    }
    println!(
        "modalities: input=[{}] output=[{}]",
        model
            .modalities
            .as_ref()
            .map(|m| m.input.join(","))
            .unwrap_or_else(|| "-".to_string()),
        model
            .modalities
            .as_ref()
            .map(|m| m.output.join(","))
            .unwrap_or_else(|| "-".to_string()),
    );
    if let Some(t) = model.tool_call {
        println!("tool_call: {t}");
    }
    if let Some(s) = model.structured_output {
        println!("structured_output: {s}");
    }
    if let Some(r) = model.reasoning {
        println!("reasoning: {r}");
    }
    if let Some(t) = model.temperature {
        println!("temperature: {t}");
    }
    if let Some(a) = model.attachment {
        println!("attachment: {a}");
    }
    if let Some(o) = model.open_weights {
        println!("open_weights: {o}");
    }
    if let Some(k) = &model.knowledge {
        println!("knowledge: {k}");
    }
    if let Some(r) = &model.release_date {
        println!("release_date: {r}");
    }
    Ok(())
}

fn fmt_opt(v: Option<f64>) -> String {
    v.map(|x| format!("{x}")).unwrap_or_else(|| "-".to_string())
}
