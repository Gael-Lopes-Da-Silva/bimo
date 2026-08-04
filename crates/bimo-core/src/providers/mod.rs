mod cloud;
mod local;
mod types;

pub use cloud::CloudProviderRegistry;
pub use local::LocalProviderRegistry;
pub use types::{ProviderEntry, ProviderMap};
