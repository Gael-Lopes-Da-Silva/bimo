//! Provider resolution — built-in locals, cloud registries, model discovery.

mod cloud;
mod local;
mod types;

pub use cloud::CloudProviderRegistry;
pub use local::LocalProviderRegistry;
pub use types::{ProviderEntry, ProviderMap};
