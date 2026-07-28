pub mod agent;
pub mod api;
pub mod command;
pub mod config;
pub mod error;
pub mod model;
pub mod prompts;
pub mod provider;
pub mod session;
pub mod tools;

// Re-export the main API types for convenience.
pub use api::BimoApi;
pub use error::{BimoError, Result};
