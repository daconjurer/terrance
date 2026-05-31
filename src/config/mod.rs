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
    ITEM_TERRY_GITHUB, ITEM_TERRY_PROJECT_TEMPLATES, OnePasswordClient, OpError, OpItem,
    SECTION_TEMPLATE_AGENTIC, SECTION_TEMPLATE_GO, SECTION_TEMPLATE_PYTHON,
    SECTION_TEMPLATE_RUST, SECTION_TEMPLATE_TYPESCRIPT,
};
pub use types::{
    Config, ConfigMetadata, GitHubConfig, LanguageTemplates, TemplateLanguage, TemplateSource,
    TemplatesConfig,
};
