use bimo_core::config::{ProvidersConfig, SettingsConfig};
use bimo_core::error::CustomError;
use bimo_core::providers::CloudProviderRegistry;
use bimo_core::session::Session;
use bimo_core::{Agent, Provider};

use crate::cli::AgentArgs;
use crate::output;

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
