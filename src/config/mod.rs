//! Terry user configuration (paths, types, persistence helpers).

pub mod manager;
pub mod types;

pub use manager::ConfigManager;
pub use types::{Config, ConfigMetadata, GitConfig, GitHubConfig};
