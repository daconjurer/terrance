use super::encryption::{Encryptor, secure_zero};
#[cfg(test)]
use super::keychain::MemoryKeyStore;
use super::keychain::{EncryptionKeyStore, KeychainManager};
use super::types::Config;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Clone)]
pub struct ConfigManager {
    config_dir: PathBuf,
    config_file: PathBuf,
    keys: Arc<dyn EncryptionKeyStore>,
}

impl ConfigManager {
    pub fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let config_dir = Self::default_config_dir()?;
        let config_file = config_dir.join("config.enc");

        Ok(Self {
            config_dir,
            config_file,
            keys: Arc::new(KeychainManager),
        })
    }

    pub fn config_dir_path(&self) -> &Path {
        self.config_dir.as_path()
    }

    fn default_config_dir() -> Result<PathBuf, Box<dyn std::error::Error + Send + Sync>> {
        let home = dirs::home_dir().ok_or("Could not find home directory")?;
        Ok(home.join(".terry"))
    }

    pub fn init_config_dir(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if !self.config_dir.exists() {
            fs::create_dir_all(&self.config_dir)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let permissions = fs::Permissions::from_mode(0o700);
                fs::set_permissions(&self.config_dir, permissions)?;
            }
        }
        Ok(())
    }

    pub fn config_exists(&self) -> bool {
        self.config_file.exists()
    }

    pub fn get_config_path(&self) -> &PathBuf {
        &self.config_file
    }

    pub fn remove_config_file(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.config_file.exists() {
            fs::remove_file(&self.config_file)?;
        }
        Ok(())
    }

    /// Writes configuration as JSON to `config.enc`. Encryption may replace this in a later phase.
    pub fn write_config_json(
        &self,
        config: &Config,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.init_config_dir()?;
        let json = serde_json::to_string_pretty(config)?;
        fs::write(&self.config_file, format!("{}\n", json))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let permissions = fs::Permissions::from_mode(0o600);
            fs::set_permissions(&self.config_file, permissions)?;
        }
        Ok(())
    }

    pub fn save_config(
        &self,
        config: &Config,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.init_config_dir()?;

        let key = self.keys.get_or_create_key()?;

        let json = serde_json::to_string_pretty(config)?;
        let plaintext = json.as_bytes();

        let encryptor = Encryptor::new(&key)?;
        let encrypted = encryptor.encrypt(plaintext)?;

        fs::write(&self.config_file, encrypted)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let permissions = fs::Permissions::from_mode(0o600);
            fs::set_permissions(&self.config_file, permissions)?;
        }

        Ok(())
    }

    pub fn load_config(&self) -> Result<Config, Box<dyn std::error::Error + Send + Sync>> {
        if !self.config_exists() {
            return Err("Config file not found. Run 'terry config sync' first.".into());
        }

        let encrypted = fs::read(&self.config_file)?;

        let key = self.keys.retrieve_key()?;

        let encryptor = Encryptor::new(&key)?;
        let mut plaintext = encryptor.decrypt(&encrypted)?;

        let config: Config = serde_json::from_slice(&plaintext)?;

        secure_zero(&mut plaintext);

        Ok(config)
    }

    #[cfg(test)]
    fn with_paths(config_dir: PathBuf, config_file: PathBuf) -> Self {
        Self::with_key_store(config_dir, config_file, Arc::new(MemoryKeyStore::new()))
    }

    #[cfg(test)]
    fn with_key_store(
        config_dir: PathBuf,
        config_file: PathBuf,
        keys: Arc<dyn EncryptionKeyStore>,
    ) -> Self {
        Self {
            config_dir,
            config_file,
            keys,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::types::{Config, ConfigMetadata, GitHubConfig};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_test_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!(
            "terry-config-test-{}-{}",
            std::process::id(),
            nanos
        ))
    }

    #[test]
    fn test_config_manager_creation() {
        let manager = ConfigManager::new();
        assert!(manager.is_ok());
    }

    #[test]
    fn test_config_dir_path_contains_terry() {
        let manager = ConfigManager::new().expect("manager");
        let path = manager.get_config_path();
        assert!(path.to_string_lossy().contains(".terry"));
    }

    #[test]
    fn test_init_config_dir_creates_with_restrictive_permissions() {
        let root = unique_test_dir();
        let config_dir = root.join(".terry");
        let config_file = config_dir.join("config.enc");
        let manager = ConfigManager::with_paths(config_dir.clone(), config_file);

        manager.init_config_dir().expect("init");
        assert!(config_dir.is_dir());
        assert!(!manager.config_exists());

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let meta = fs::metadata(&config_dir).expect("metadata");
            assert_eq!(meta.permissions().mode() & 0o777, 0o700);
        }

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn test_config_exists_after_touch() {
        let root = unique_test_dir();
        let config_dir = root.join(".terry");
        let config_file = config_dir.join("config.enc");
        let manager = ConfigManager::with_paths(config_dir.clone(), config_file.clone());

        manager.init_config_dir().expect("init");
        fs::write(&config_file, b"{}").expect("write");
        assert!(manager.config_exists());

        manager.remove_config_file().expect("remove");
        assert!(!manager.config_exists());

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn test_write_config_json_roundtrip() {
        let root = unique_test_dir();
        let config_dir = root.join(".terry");
        let config_file = config_dir.join("config.enc");
        let manager = ConfigManager::with_paths(config_dir.clone(), config_file.clone());

        let config = Config {
            github: GitHubConfig {
                token: "dummy-token".to_string(),
                token_write: Some("dummy-write-token".to_string()),
                username: "Nope".to_string(),
            },
            metadata: ConfigMetadata {
                synced_at: "2026-01-01T00:00:00Z".to_string(),
                vault_name: "Dev".to_string(),
            },
        };

        manager.write_config_json(&config).expect("write");
        assert!(manager.config_exists());

        let raw = fs::read_to_string(&config_file).expect("read");
        let parsed: Config = serde_json::from_str(raw.trim()).expect("parse");
        assert_eq!(parsed.github.token, "dummy-token");
        assert_eq!(
            parsed.github.token_write.as_deref(),
            Some("dummy-write-token")
        );
        assert_eq!(parsed.github.username, "Nope");
        assert_eq!(parsed.metadata.vault_name, "Dev".to_string());

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let meta = fs::metadata(&config_file).expect("metadata");
            assert_eq!(meta.permissions().mode() & 0o777, 0o600);
        }

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn test_save_and_load_config() {
        let root = unique_test_dir();
        let config_dir = root.join(".terry");
        let config_file = config_dir.join("config.enc");
        let manager = ConfigManager::with_paths(config_dir.clone(), config_file.clone());

        let config = Config {
            github: GitHubConfig {
                token: "ghp_secret_token_12345".to_string(),
                token_write: Some("ghp_write_secret_67890".to_string()),
                username: "testuser".to_string(),
            },
            metadata: ConfigMetadata {
                synced_at: "2026-05-01T22:00:00Z".to_string(),
                vault_name: "Development".to_string(),
            },
        };

        manager.save_config(&config).expect("save");
        assert!(manager.config_exists());

        let loaded = manager.load_config().expect("load");
        assert_eq!(loaded.github.token, "ghp_secret_token_12345");
        assert_eq!(
            loaded.github.token_write.as_deref(),
            Some("ghp_write_secret_67890")
        );
        assert_eq!(loaded.github.username, "testuser");
        assert_eq!(loaded.metadata.vault_name, "Development");

        let raw = fs::read(&config_file).expect("read");
        assert!(raw[0] == 1);
        assert!(!String::from_utf8_lossy(&raw).contains("ghp_secret"));

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let meta = fs::metadata(&config_file).expect("metadata");
            assert_eq!(meta.permissions().mode() & 0o777, 0o600);
        }

        fs::remove_dir_all(&root).ok();
    }
}
