use crate::config::encryption;
use base64::{Engine as _, engine::general_purpose};
use keyring::{Entry, Error as KeyringError};
#[cfg(test)]
use std::sync::Mutex;
use thiserror::Error;

const SERVICE_NAME: &str = "com.terrance.config";
const KEY_NAME: &str = "encryption-key";

#[derive(Error, Debug)]
pub enum KeychainError {
    #[error("Keychain access failed: {0}")]
    AccessFailed(String),

    #[error("Key not found in keychain")]
    KeyNotFound,

    #[error("Failed to store key: {0}")]
    StoreFailed(String),
}

pub trait EncryptionKeyStore: Send + Sync {
    fn store_key(&self, key: &[u8; 32]) -> Result<(), KeychainError>;
    fn retrieve_key(&self) -> Result<[u8; 32], KeychainError>;
    fn delete_key(&self) -> Result<(), KeychainError>;

    fn key_exists(&self) -> bool {
        self.retrieve_key().is_ok()
    }

    fn get_or_create_key(&self) -> Result<[u8; 32], KeychainError> {
        match self.retrieve_key() {
            Ok(key) => Ok(key),
            Err(KeychainError::KeyNotFound) => {
                let key = encryption::generate_key();
                self.store_key(&key)?;
                Ok(key)
            }
            Err(e) => Err(e),
        }
    }
}

pub struct KeychainManager;

impl EncryptionKeyStore for KeychainManager {
    fn store_key(&self, key: &[u8; 32]) -> Result<(), KeychainError> {
        let entry = get_entry()?;
        let key_b64 = general_purpose::STANDARD.encode(key);
        entry
            .set_password(&key_b64)
            .map_err(|e| KeychainError::StoreFailed(e.to_string()))?;
        Ok(())
    }

    fn retrieve_key(&self) -> Result<[u8; 32], KeychainError> {
        let entry = get_entry()?;

        let key_b64 = entry.get_password().map_err(|e| match e {
            KeyringError::NoEntry => KeychainError::KeyNotFound,
            _ => KeychainError::AccessFailed(e.to_string()),
        })?;

        let key_bytes = general_purpose::STANDARD
            .decode(&key_b64)
            .map_err(|e| KeychainError::AccessFailed(format!("Invalid key format: {}", e)))?;

        if key_bytes.len() != 32 {
            return Err(KeychainError::AccessFailed(
                "Invalid key length".to_string(),
            ));
        }

        let mut key = [0u8; 32];
        key.copy_from_slice(&key_bytes);

        Ok(key)
    }

    fn delete_key(&self) -> Result<(), KeychainError> {
        let entry = get_entry()?;
        entry
            .delete_password()
            .map_err(|e| KeychainError::AccessFailed(e.to_string()))?;
        Ok(())
    }
}

fn get_entry() -> Result<Entry, KeychainError> {
    Entry::new(SERVICE_NAME, KEY_NAME).map_err(|e| KeychainError::AccessFailed(e.to_string()))
}

/// In-memory key storage for tests (no OS keychain).
#[cfg(test)]
#[derive(Default)]
pub(crate) struct MemoryKeyStore {
    key: Mutex<Option<[u8; 32]>>,
}

#[cfg(test)]
impl MemoryKeyStore {
    pub fn new() -> Self {
        Self {
            key: Mutex::new(None),
        }
    }
}

#[cfg(test)]
impl EncryptionKeyStore for MemoryKeyStore {
    fn store_key(&self, key: &[u8; 32]) -> Result<(), KeychainError> {
        *self.key.lock().expect("key lock") = Some(*key);
        Ok(())
    }

    fn retrieve_key(&self) -> Result<[u8; 32], KeychainError> {
        self.key
            .lock()
            .expect("key lock")
            .ok_or(KeychainError::KeyNotFound)
    }

    fn delete_key(&self) -> Result<(), KeychainError> {
        *self.key.lock().expect("key lock") = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_store_retrieve_key() {
        let store = MemoryKeyStore::new();
        let key = encryption::generate_key();

        store.store_key(&key).unwrap();
        let retrieved = store.retrieve_key().unwrap();

        assert_eq!(key, retrieved);

        store.delete_key().unwrap();
    }

    #[test]
    fn test_memory_get_or_create_key() {
        let store = MemoryKeyStore::new();

        let key1 = store.get_or_create_key().unwrap();
        let key2 = store.get_or_create_key().unwrap();

        assert_eq!(key1, key2);

        store.delete_key().unwrap();
    }

    #[test]
    fn test_memory_key_exists() {
        let store = MemoryKeyStore::new();

        assert!(!store.key_exists());

        let key = encryption::generate_key();
        store.store_key(&key).unwrap();

        assert!(store.key_exists());

        store.delete_key().unwrap();
    }
}
