use bimo_core::config::{ApiFormat, Provider, ProviderType, ProvidersConfig, SettingsConfig};
use bimo_core::models::ModelRegistry;
use bimo_core::providers::{CloudProviderRegistry, LocalProviderRegistry};

#[derive(Debug, Clone)]
pub struct ConfigState {
    pub settings: SettingsConfig,
    pub providers: ProvidersConfig,
    pub cloud_registry: Option<CloudProviderRegistry>,
    pub local_registry: LocalProviderRegistry,
    pub model_registry: Option<ModelRegistry>,
    pub available_models: Vec<String>,
    pub selected_provider: Option<String>,
    pub selected_model: Option<String>,
    pub discovering_models: bool,
    pub refreshing_catalogue: bool,
}

impl ConfigState {
    pub fn new() -> Self {
        let settings = SettingsConfig::load().unwrap_or_default();
        let providers = ProvidersConfig::load().unwrap_or(ProvidersConfig {
            providers: Vec::new(),
            default: None,
        });
        let local_registry = LocalProviderRegistry::new();

        Self {
            settings,
            providers,
            cloud_registry: None,
            local_registry,
            model_registry: None,
            available_models: Vec::new(),
            selected_provider: None,
            selected_model: None,
            discovering_models: false,
            refreshing_catalogue: false,
        }
    }

    pub async fn load_registries(&mut self) {
        let cloud = CloudProviderRegistry::new();
        let _ = cloud.load().await;
        self.cloud_registry = Some(cloud.clone());
        self.model_registry = Some(bimo_core::models::ModelRegistry::from_registry(&cloud));
    }

    pub fn save_settings(&self) -> Result<(), bimo_core::error::CustomError> {
        self.settings.save()
    }

    pub fn save_providers(&self) -> Result<(), bimo_core::error::CustomError> {
        self.providers.save()
    }

    pub fn add_provider(&mut self, provider: Provider) {
        self.providers.providers.push(provider);
    }

    pub fn remove_provider(&mut self, id: &str) {
        self.providers.providers.retain(|p| p.id != id);
        if self.providers.default.as_deref() == Some(id) {
            self.providers.default = None;
        }
    }

    pub fn set_default_provider(&mut self, id: &str) -> bool {
        if self.providers.find(id).is_some() {
            self.providers.default = Some(id.to_string());
            true
        } else {
            false
        }
    }

    pub fn get_provider(&self, id: &str) -> Option<&Provider> {
        self.providers.find(id)
    }

    pub fn local_providers(&self) -> Vec<&Provider> {
        self.providers.local_providers()
    }

    pub fn cloud_providers(&self) -> Vec<&Provider> {
        self.providers.cloud_providers()
    }

    pub async fn discover_models(
        &mut self,
        provider_id: &str,
    ) -> Result<Vec<String>, bimo_core::error::CustomError> {
        self.discovering_models = true;

        let result = if let Some(provider) = self.providers.find(provider_id) {
            if provider.is_local() {
                let mut p = provider.clone();
                self.local_registry.auto_discover_models(&mut p).await;
                Ok(p.models)
            } else if let Some(cloud) = &self.cloud_registry {
                cloud.refresh_provider(provider_id).await?;
                if let Some(reg) = &self.model_registry {
                    let models = reg.list_models(provider_id).await;
                    Ok(models.into_iter().map(|m| m.name).collect())
                } else {
                    Ok(Vec::new())
                }
            } else {
                Ok(Vec::new())
            }
        } else {
            Ok(Vec::new())
        };

        self.discovering_models = false;
        result
    }

    pub async fn refresh_catalogue(&mut self) -> Result<usize, bimo_core::error::CustomError> {
        self.refreshing_catalogue = true;

        let result = if let Some(cloud) = &self.cloud_registry {
            cloud.refresh().await?;
            let count = cloud.provider_count().await;
            self.model_registry = Some(bimo_core::models::ModelRegistry::from_registry(cloud));
            Ok(count)
        } else {
            Ok(0)
        };

        self.refreshing_catalogue = false;
        result
    }
}

impl Default for ConfigState {
    fn default() -> Self {
        Self::new()
    }
}
