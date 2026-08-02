//! Provider command handlers — configure and inspect providers.

use bimo_core::config::{ApiFormat, Provider, ProviderType, ProvidersConfig};
use bimo_core::error::CustomError;
use bimo_core::models::ModelRegistry;
use bimo_core::providers::{CloudProviderRegistry, LocalProviderRegistry};
use serde::Serialize;

use crate::cli::{ProviderAddArgs, ProviderCommand, ProviderTypeArg};
use crate::output;

pub async fn run(json: bool, sub: &ProviderCommand) -> crate::Result<()> {
    match sub {
        ProviderCommand::List => list(json).await,
        ProviderCommand::Search { query } => search(json, query.as_deref()).await,
        ProviderCommand::Add(args) => add(args).await,
        ProviderCommand::Remove { id } => remove(id).await,
        ProviderCommand::Show { id } => show(json, id).await,
        ProviderCommand::SetDefault { id } => set_default(id).await,
        ProviderCommand::Models { id, refresh } => models(json, id, *refresh).await,
        ProviderCommand::Refresh => refresh_cache().await,
    }
}

async fn list(json: bool) -> crate::Result<()> {
    let config = ProvidersConfig::load()?;
    if json {
        return output::emit_json(&config);
    }
    if config.providers.is_empty() {
        println!("No providers configured.");
        println!(
            "Add one with `bimo provider add <id>`, or search the catalogue with `bimo provider search`."
        );
        return Ok(());
    }
    for p in &config.providers {
        let default = config.default.as_deref() == Some(p.id.as_str());
        let kind = if p.is_local() { "local" } else { "cloud" };
        let mark = if default { " *" } else { "" };
        println!(
            "{:<24} {:<20} {:<6} {:<20} {}{}",
            p.id,
            output::truncate(&p.name, 20),
            kind,
            api_format_label(&p.api_format),
            output::truncate(&p.base_url, 40),
            mark,
        );
    }
    if let Some(d) = &config.default {
        println!("Default: {d}");
    }
    Ok(())
}

#[derive(Serialize)]
struct ProviderSummary {
    id: String,
    name: String,
    base_url: String,
    api_format: String,
    kind: String,
    models: usize,
}

async fn search(json: bool, query: Option<&str>) -> crate::Result<()> {
    let registry = CloudProviderRegistry::new();
    if let Err(e) = registry.load().await {
        eprintln!("Note: models.dev catalogue unavailable: {e}");
    }
    let mut entries: Vec<ProviderSummary> = Vec::new();

    for p in LocalProviderRegistry::new().builtin() {
        if matches_query(&p.id, &p.name, query) {
            entries.push(ProviderSummary {
                id: p.id,
                name: p.name,
                base_url: p.base_url,
                api_format: api_format_label(&p.api_format),
                kind: "local".to_string(),
                models: 0,
            });
        }
    }
    for p in registry.list_providers().await {
        if matches_query(&p.id, &p.name, query) {
            entries.push(ProviderSummary {
                id: p.id.clone(),
                name: p.name.clone(),
                base_url: p.base_url().unwrap_or_default(),
                api_format: api_format_label(&p.api_format()),
                kind: "cloud".to_string(),
                models: p.models.len(),
            });
        }
    }
    entries.sort_by(|a, b| a.id.cmp(&b.id));

    if json {
        return output::emit_json_array(&entries);
    }
    if entries.is_empty() {
        println!("No providers match.");
        return Ok(());
    }
    for e in &entries {
        println!(
            "{:<24} {:<20} {:<6} {:<20} {}",
            e.id,
            output::truncate(&e.name, 20),
            e.kind,
            e.api_format,
            output::truncate(&e.base_url, 40),
        );
    }
    Ok(())
}

async fn add(args: &ProviderAddArgs) -> crate::Result<()> {
    let mut config = ProvidersConfig::load()?;
    if config.find(&args.id).is_some() {
        return Err(CustomError::Config(format!(
            "Provider '{}' already configured",
            args.id
        )));
    }

    let provider_type = match args.provider_type {
        ProviderTypeArg::Local => ProviderType::Local,
        ProviderTypeArg::Cloud => ProviderType::Cloud,
    };

    let mut provider = Provider {
        id: args.id.clone(),
        name: args.name.clone().unwrap_or_else(|| args.id.clone()),
        base_url: args.base_url.clone().unwrap_or_default(),
        api_key: args.api_key.clone(),
        provider_type,
        models: Vec::new(),
        api_format: ApiFormat::from(args.api_format),
    };

    if provider.is_cloud() && provider.base_url.is_empty() {
        let registry = CloudProviderRegistry::new();
        let _ = registry.load().await;
        if let Some(url) = registry.provider_base_url(&provider.id).await {
            provider.base_url = url;
        }
    }
    if args.discover && provider.is_local() {
        LocalProviderRegistry::new()
            .auto_discover_models(&mut provider)
            .await;
    }

    config.providers.push(provider.clone());
    config.save()?;

    println!("Added provider '{}'", provider.id);
    if provider.is_cloud() && provider.api_key.is_none() {
        println!(
            "Note: no API key set; add one with `bimo provider add {} --api-key <key>`",
            provider.id
        );
    }
    if !provider.models.is_empty() {
        println!("Discovered {} models", provider.models.len());
    }
    Ok(())
}

async fn remove(id: &str) -> crate::Result<()> {
    let mut config = ProvidersConfig::load()?;
    let before = config.providers.len();
    config.providers.retain(|p| p.id != id && p.name != id);
    if config.providers.len() == before {
        return Err(CustomError::Config(format!("Provider '{id}' not found")));
    }
    if config.default.as_deref() == Some(id) {
        config.default = None;
    }
    config.save()?;
    println!("Removed provider '{id}'");
    Ok(())
}

async fn show(json: bool, id: &str) -> crate::Result<()> {
    if let Some(p) = ProvidersConfig::load()?.find(id) {
        if json {
            return output::emit_json(p);
        }
        return print_provider(p);
    }

    let registry = CloudProviderRegistry::new();
    let _ = registry.load().await;
    if let Some(entry) = registry.find_provider(id).await {
        if json {
            return output::emit_json(&entry);
        }
        println!("id: {}", entry.id);
        println!("name: {}", entry.name);
        println!("kind: cloud");
        println!("base_url: {}", entry.base_url().unwrap_or_default());
        println!("api_format: {}", api_format_label(&entry.api_format()));
        println!("env: {}", entry.env.join(", "));
        println!("doc: {}", entry.doc.as_deref().unwrap_or("-"));
        println!("models: {}", entry.models.len());
        return Ok(());
    }

    if let Some(p) = LocalProviderRegistry::new().find(id) {
        if json {
            return output::emit_json(&p);
        }
        return print_provider(&p);
    }

    Err(CustomError::Config(format!("Provider '{id}' not found")))
}

async fn set_default(id: &str) -> crate::Result<()> {
    let mut config = ProvidersConfig::load()?;
    if config.find(id).is_none() {
        return Err(CustomError::Config(format!(
            "Provider '{id}' not configured"
        )));
    }
    config.default = Some(id.to_string());
    config.save()?;
    println!("Default provider set to '{id}'");
    Ok(())
}

async fn models(json: bool, id: &str, refresh: bool) -> crate::Result<()> {
    let config = ProvidersConfig::load()?;
    let configured = config.find(id);

    let registry = CloudProviderRegistry::new();
    if refresh {
        if !configured.is_some_and(|p| p.is_local()) {
            registry.refresh_provider(id).await?;
        }
    } else {
        let _ = registry.load().await;
    }

    if let Some(p) = configured {
        let mut ids = p.models.clone();
        if ids.is_empty()
            && p.is_local()
            && let Ok(found) = LocalProviderRegistry::new().discover_models(p).await
        {
            ids = found;
        }
        if !ids.is_empty() {
            ids.sort();
            if json {
                return output::emit_json_array(&ids);
            }
            for m in &ids {
                println!("{m}");
            }
            return Ok(());
        }
    }

    let model_registry = ModelRegistry::from_registry(&registry);
    let models = model_registry.list_models(id).await;
    if !models.is_empty() {
        return output::print_models(json, &models);
    }

    Err(CustomError::Msg(format!(
        "No models found for provider '{id}'"
    )))
}

async fn refresh_cache() -> crate::Result<()> {
    let registry = CloudProviderRegistry::new();
    registry.refresh().await?;
    let count = registry.provider_count().await;
    println!("Refreshed models.dev catalogue: {count} providers cached");
    Ok(())
}

fn print_provider(p: &Provider) -> crate::Result<()> {
    println!("id: {}", p.id);
    println!("name: {}", p.name);
    println!("kind: {}", if p.is_local() { "local" } else { "cloud" });
    println!("base_url: {}", p.base_url);
    println!("api_format: {}", api_format_label(&p.api_format));
    if p.api_key.is_some() {
        println!("api_key: (set)");
    }
    if !p.models.is_empty() {
        println!("models: {}", p.models.join(", "));
    }
    Ok(())
}

fn api_format_label(fmt: &ApiFormat) -> String {
    match fmt {
        ApiFormat::OpenAICompatible => "openai_compatible".to_string(),
        ApiFormat::Anthropic => "anthropic".to_string(),
        ApiFormat::OpenAI => "openai".to_string(),
        ApiFormat::Google => "google".to_string(),
        ApiFormat::Other(s) => s.clone(),
    }
}

fn matches_query(id: &str, name: &str, query: Option<&str>) -> bool {
    match query {
        None => true,
        Some(q) => id.to_lowercase().contains(q) || name.to_lowercase().contains(q),
    }
}
