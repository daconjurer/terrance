//! Terry user configuration (paths, types, persistence helpers).

pub mod manager;
pub mod one_password;
pub mod types;

pub use manager::ConfigManager;
pub use one_password::{ITEM_TERRY_GITHUB, OnePasswordClient, OpError};
pub use types::{Config, ConfigMetadata, GitHubConfig};
