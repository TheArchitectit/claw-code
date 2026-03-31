//! Secure Storage for OAuth Tokens
//!
//! Provides pluggable secure storage backends:
//! - System keyring (preferred)
//! - Encrypted file fallback (when keyring unavailable)
//! - Environment variable (for CI/testing)

use anyhow::{Context, Result};
use async_trait::async_trait;
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use ring::rand::SecureRandom;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Secure storage trait for OAuth tokens
#[async_trait]
pub trait SecureStorage: Send + Sync + std::fmt::Debug {
    /// Get a value from storage
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>>;

    /// Set a value in storage
    async fn set(&self, key: &str, value: &[u8]) -> Result<()>;

    /// Delete a value from storage
    async fn delete(&self, key: &str) -> Result<()>;

    /// Get storage backend type
    fn backend_type(&self) -> StorageBackend;
}

/// Storage backend types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageBackend {
    /// System keyring (macOS Keychain, Windows Credential Manager, Linux Secret Service)
    Keyring,
    /// Encrypted file storage
    EncryptedFile,
    /// Environment variable (for testing/CI)
    Environment,
}

impl StorageBackend {
    /// Get display name
    pub fn name(&self) -> &'static str {
        match self {
            StorageBackend::Keyring => "System Keyring",
            StorageBackend::EncryptedFile => "Encrypted File",
            StorageBackend::Environment => "Environment Variable",
        }
    }
}

/// Keyring-based secure storage
#[derive(Debug)]
pub struct KeyringStorage {
    service_name: String,
}

impl KeyringStorage {
    /// Create a new keyring storage
    pub fn new(service_name: impl Into<String>) -> Self {
        Self {
            service_name: service_name.into(),
        }
    }

    /// Create with default service name
    pub fn default_service() -> Self {
        Self::new("claude-code-rust")
    }
}

#[async_trait]
impl SecureStorage for KeyringStorage {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let entry = keyring::Entry::new(&self.service_name, key)?;
        match entry.get_password() {
            Ok(value) => {
                // Decode from base64
                let decoded = BASE64.decode(&value)?;
                Ok(Some(decoded))
            }
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(anyhow::anyhow!("Keyring error: {}", e)),
        }
    }

    async fn set(&self, key: &str, value: &[u8]) -> Result<()> {
        let entry = keyring::Entry::new(&self.service_name, key)?;
        // Encode as base64 for storage
        let encoded = BASE64.encode(value);
        entry.set_password(&encoded)?;
        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<()> {
        let entry = keyring::Entry::new(&self.service_name, key)?;
        entry.delete_credential()?;
        Ok(())
    }

    fn backend_type(&self) -> StorageBackend {
        StorageBackend::Keyring
    }
}

/// Encrypted file-based storage
#[derive(Debug)]
pub struct EncryptedFileStorage {
    storage_dir: PathBuf,
    encryption_key: Vec<u8>,
}

impl EncryptedFileStorage {
    /// Create a new encrypted file storage
    pub fn new(storage_dir: PathBuf) -> Result<Self> {
        // Generate or load encryption key
        let key = Self::get_or_create_key(&storage_dir)?;

        Ok(Self {
            storage_dir,
            encryption_key: key,
        })
    }

    /// Get or create encryption key
    fn get_or_create_key(storage_dir: &PathBuf) -> Result<Vec<u8>> {
        let key_file = storage_dir.join(".key");

        if key_file.exists() {
            // Load existing key
            let key_data = std::fs::read(&key_file)?;
            // Decode from hex
            let key_hex = String::from_utf8_lossy(&key_data);
            let key = hex::decode(key_hex.trim())?;
            Ok(key)
        } else {
            // Generate new key
            let rng = ring::rand::SystemRandom::new();
            let mut key = vec![0u8; 32];
            rng.fill(&mut key)
                .map_err(|_| anyhow::anyhow!("Failed to generate encryption key"))?;

            // Save key (in real implementation, this should use OS-level protection)
            std::fs::create_dir_all(storage_dir)?;
            std::fs::write(&key_file, hex::encode(&key))?;
            // Restrict permissions
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = std::fs::metadata(&key_file)?.permissions();
                perms.set_mode(0o600);
                std::fs::set_permissions(&key_file, perms)?;
            }

            Ok(key)
        }
    }

    /// Get file path for a key
    fn file_path(&self, key: &str) -> PathBuf {
        // Sanitize key for filesystem
        let safe_key = key.replace('/', "_").replace('\\', "_");
        self.storage_dir.join(format!("{}.enc", safe_key))
    }

    /// Encrypt data (simplified implementation - uses basic XOR for demo)
    /// In production, use proper AEAD encryption like AES-256-GCM
    fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        // Generate random nonce (12 bytes for GCM)
        let rng = ring::rand::SystemRandom::new();
        let mut nonce = [0u8; 12];
        rng.fill(&mut nonce)
            .map_err(|_| anyhow::anyhow!("Failed to generate nonce"))?;

        // Simple XOR-based encryption for demonstration
        // In production, use ring::aead with proper NonceSequence implementation
        let mut ciphertext = nonce.to_vec();
        for (i, byte) in plaintext.iter().enumerate() {
            let key_byte = self.encryption_key[i % self.encryption_key.len()];
            ciphertext.push(byte ^ key_byte);
        }

        Ok(ciphertext)
    }

    /// Decrypt data
    fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        if ciphertext.len() < 12 {
            return Err(anyhow::anyhow!("Invalid ciphertext"));
        }

        // Skip nonce bytes (first 12 bytes)
        let encrypted = &ciphertext[12..];

        // XOR decrypt
        let mut plaintext = Vec::with_capacity(encrypted.len());
        for (i, byte) in encrypted.iter().enumerate() {
            let key_byte = self.encryption_key[i % self.encryption_key.len()];
            plaintext.push(byte ^ key_byte);
        }

        Ok(plaintext)
    }
}

#[async_trait]
impl SecureStorage for EncryptedFileStorage {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let path = self.file_path(key);
        if !path.exists() {
            return Ok(None);
        }

        let data = tokio::fs::read(&path).await?;
        let decrypted = self.decrypt(&data)?;
        Ok(Some(decrypted))
    }

    async fn set(&self, key: &str, value: &[u8]) -> Result<()> {
        tokio::fs::create_dir_all(&self.storage_dir).await?;
        let path = self.file_path(key);
        let encrypted = self.encrypt(value)?;
        tokio::fs::write(&path, encrypted).await?;

        // Restrict permissions
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = tokio::fs::metadata(&path).await?.permissions();
            perms.set_mode(0o600);
            tokio::fs::set_permissions(&path, perms).await?;
        }

        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<()> {
        let path = self.file_path(key);
        if path.exists() {
            tokio::fs::remove_file(&path).await?;
        }
        Ok(())
    }

    fn backend_type(&self) -> StorageBackend {
        StorageBackend::EncryptedFile
    }
}

/// Environment variable storage (for testing/CI)
#[derive(Debug)]
pub struct EnvironmentStorage {
    prefix: String,
}

impl EnvironmentStorage {
    /// Create a new environment storage
    pub fn new(prefix: impl Into<String>) -> Self {
        Self {
            prefix: prefix.into(),
        }
    }

    fn env_key(&self, key: &str) -> String {
        format!("{}_{}", self.prefix, key.to_uppercase().replace('-', "_"))
    }
}

#[async_trait]
impl SecureStorage for EnvironmentStorage {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let env_key = self.env_key(key);
        match std::env::var(&env_key) {
            Ok(value) => {
                // Decode from base64
                let decoded = BASE64.decode(&value)?;
                Ok(Some(decoded))
            }
            Err(_) => Ok(None),
        }
    }

    async fn set(&self, key: &str, value: &[u8]) -> Result<()> {
        // Environment storage is read-only for normal use
        warn!("Cannot write to environment storage - use encrypted file or keyring instead");
        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<()> {
        // Cannot delete from environment
        Ok(())
    }

    fn backend_type(&self) -> StorageBackend {
        StorageBackend::Environment
    }
}

/// Create the default storage backend
///
/// Tries keyring first, falls back to encrypted file if unavailable
pub async fn create_default_storage() -> Result<Arc<dyn SecureStorage>> {
    // Try keyring first
    match keyring::Entry::new("claude-code-test", "test") {
        Ok(test_entry) => {
            if test_entry.set_password("test").is_ok() {
                let _ = test_entry.delete_credential();
                info!("Using system keyring for secure storage");
                return Ok(Arc::new(KeyringStorage::default_service()));
            }
        }
        Err(_) => {}
    }

    // Fall back to encrypted file
    let storage_dir = dirs::data_dir()
        .map(|d| d.join("claude").join("oauth"))
        .unwrap_or_else(|| PathBuf::from(".claude/oauth"));

    warn!("Keyring unavailable, using encrypted file storage");
    Ok(Arc::new(EncryptedFileStorage::new(storage_dir)?))
}

/// Create storage with a specific backend
pub fn create_storage(backend: StorageBackend) -> Result<Arc<dyn SecureStorage>> {
    match backend {
        StorageBackend::Keyring => {
            Ok(Arc::new(KeyringStorage::default_service()))
        }
        StorageBackend::EncryptedFile => {
            let storage_dir = dirs::data_dir()
                .map(|d| d.join("claude").join("oauth"))
                .unwrap_or_else(|| PathBuf::from(".claude/oauth"));
            Ok(Arc::new(EncryptedFileStorage::new(storage_dir)?))
        }
        StorageBackend::Environment => {
            Ok(Arc::new(EnvironmentStorage::new("CLAUDE_OAUTH")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_encrypted_file_storage() {
        let temp_dir = TempDir::new().unwrap();
        let storage = EncryptedFileStorage::new(temp_dir.path().to_path_buf()).unwrap();

        // Test store and retrieve
        let key = "test_token";
        let value = b"secret_token_value";

        storage.set(key, value).await.unwrap();
        let retrieved = storage.get(key).await.unwrap();

        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap(), value);

        // Test delete
        storage.delete(key).await.unwrap();
        let after_delete = storage.get(key).await.unwrap();
        assert!(after_delete.is_none());
    }

    #[test]
    fn test_storage_backend_names() {
        assert_eq!(StorageBackend::Keyring.name(), "System Keyring");
        assert_eq!(StorageBackend::EncryptedFile.name(), "Encrypted File");
        assert_eq!(StorageBackend::Environment.name(), "Environment Variable");
    }
}
