//! Provider resolution — built-in locals, cloud registries, model discovery.

mod cloud;
mod entry;
mod local;

pub use cloud::CloudProviderRegistry;
pub use entry::{ProviderEntry, ProviderMap};
pub use local::LocalProviderRegistry;
