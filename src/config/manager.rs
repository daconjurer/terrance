use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct ConfigManager {
    config_dir: PathBuf,
    config_file: PathBuf,
}

impl ConfigManager {
    pub fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let config_dir = Self::default_config_dir()?;
        let config_file = config_dir.join("config.enc");

        Ok(Self {
            config_dir,
            config_file,
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

    #[cfg(test)]
    fn with_paths(config_dir: PathBuf, config_file: PathBuf) -> Self {
        Self {
            config_dir,
            config_file,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_test_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("terry-config-test-{}-{}", std::process::id(), nanos))
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
}
