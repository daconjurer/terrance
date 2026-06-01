//! Terry user configuration (paths, types, persistence helpers).

pub mod encryption;
pub mod keychain;
pub mod manager;
pub mod one_password;
pub mod types;

pub use encryption::{EncryptionError, Encryptor};
pub use keychain::{EncryptionKeyStore, KeychainError, KeychainManager};
pub use manager::ConfigManager;
pub use one_password::{
    OnePasswordClient, OpError, OpItem, OpItemName, OpTemplateSection,
};
pub use types::{
    Config, ConfigMetadata, GitHubConfig, LanguageTemplates, TemplateLanguage, TemplateSource,
    TemplatesConfig,
};
