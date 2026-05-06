use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit, OsRng},
};
use rand::RngCore;
use thiserror::Error;

const NONCE_SIZE: usize = 12;
const VERSION: u8 = 1;

#[derive(Error, Debug)]
pub enum EncryptionError {
    #[error("Encryption failed: {0}")]
    EncryptionFailed(String),

    #[error("Decryption failed: {0}")]
    DecryptionFailed(String),

    #[error("Invalid encrypted data format")]
    InvalidFormat,

    #[error("Unsupported version: {0}")]
    UnsupportedVersion(u8),
}

pub struct Encryptor {
    cipher: Aes256Gcm,
}

impl Encryptor {
    pub fn new(key: &[u8; 32]) -> Result<Self, EncryptionError> {
        let cipher = Aes256Gcm::new(key.into());
        Ok(Self { cipher })
    }

    pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>, EncryptionError> {
        let mut nonce_bytes = [0u8; NONCE_SIZE];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = self
            .cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| EncryptionError::EncryptionFailed(e.to_string()))?;

        let mut result = Vec::with_capacity(1 + NONCE_SIZE + ciphertext.len());
        result.push(VERSION);
        result.extend_from_slice(&nonce_bytes);
        result.extend_from_slice(&ciphertext);

        Ok(result)
    }

    pub fn decrypt(&self, encrypted: &[u8]) -> Result<Vec<u8>, EncryptionError> {
        if encrypted.len() < 1 + NONCE_SIZE + 16 {
            return Err(EncryptionError::InvalidFormat);
        }

        let version = encrypted[0];
        if version != VERSION {
            return Err(EncryptionError::UnsupportedVersion(version));
        }

        let nonce_bytes = &encrypted[1..1 + NONCE_SIZE];
        let ciphertext = &encrypted[1 + NONCE_SIZE..];

        let nonce = Nonce::from_slice(nonce_bytes);

        self.cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| EncryptionError::DecryptionFailed(e.to_string()))
    }
}

pub fn generate_key() -> [u8; 32] {
    let mut key = [0u8; 32];
    OsRng.fill_bytes(&mut key);
    key
}

pub fn secure_zero(data: &mut [u8]) {
    use std::sync::atomic::{Ordering, compiler_fence};
    for byte in data.iter_mut() {
        unsafe { std::ptr::write_volatile(byte, 0) };
    }
    compiler_fence(Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt() {
        let key = generate_key();
        let encryptor = Encryptor::new(&key).unwrap();

        let plaintext = b"secret config data";
        let encrypted = encryptor.encrypt(plaintext).unwrap();
        let decrypted = encryptor.decrypt(&encrypted).unwrap();

        assert_eq!(plaintext, &decrypted[..]);
        assert_ne!(&encrypted[1 + NONCE_SIZE..], plaintext);
    }

    #[test]
    fn test_different_nonces() {
        let key = generate_key();
        let encryptor = Encryptor::new(&key).unwrap();

        let plaintext = b"same data";
        let encrypted1 = encryptor.encrypt(plaintext).unwrap();
        let encrypted2 = encryptor.encrypt(plaintext).unwrap();

        assert_ne!(encrypted1, encrypted2);
    }

    #[test]
    fn test_invalid_format() {
        let key = generate_key();
        let encryptor = Encryptor::new(&key).unwrap();

        let result = encryptor.decrypt(&[1, 2, 3]);
        assert!(matches!(result, Err(EncryptionError::InvalidFormat)));
    }

    #[test]
    fn test_wrong_key() {
        let key1 = generate_key();
        let key2 = generate_key();

        let encryptor1 = Encryptor::new(&key1).unwrap();
        let encryptor2 = Encryptor::new(&key2).unwrap();

        let plaintext = b"secret";
        let encrypted = encryptor1.encrypt(plaintext).unwrap();
        let result = encryptor2.decrypt(&encrypted);

        assert!(result.is_err());
    }

    #[test]
    fn test_tampered_ciphertext_fails() {
        let key = generate_key();
        let encryptor = Encryptor::new(&key).unwrap();

        let mut encrypted = encryptor.encrypt(b"secret").unwrap();

        encrypted[15] ^= 0x01;

        let result = encryptor.decrypt(&encrypted);
        assert!(result.is_err());
    }

    #[test]
    fn test_unsupported_version() {
        let key = generate_key();
        let encryptor = Encryptor::new(&key).unwrap();

        let mut encrypted = encryptor.encrypt(b"secret").unwrap();
        encrypted[0] = 99;

        let result = encryptor.decrypt(&encrypted);
        assert!(matches!(
            result,
            Err(EncryptionError::UnsupportedVersion(99))
        ));
    }
}
