//! Terry user configuration (paths, types, persistence helpers).

pub mod encryption;
pub mod keychain;
pub mod manager;
pub mod one_password;
pub mod types;

pub use encryption::{EncryptionError, Encryptor};
pub use keychain::{EncryptionKeyStore, KeychainError, KeychainManager};
pub use manager::ConfigManager;
pub use one_password::{ITEM_TERRY_GITHUB, OnePasswordClient, OpError};
pub use types::{Config, ConfigMetadata, GitHubConfig};
