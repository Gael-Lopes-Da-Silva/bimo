//! Agent orchestration — builder, executor, and instruction loading.

mod builder;
mod executor;
pub mod instructions;

pub use builder::AgentBuilder;
pub use executor::{Agent, AgentEvent};
