//! Model metadata types and registry for querying models from known providers.

mod registry;
pub mod types;

pub use registry::ModelRegistry;
pub use types::{ModelCost, ModelEntry, ModelLimit, ModelMap, ModelModalities, ReasoningOption};
