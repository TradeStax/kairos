//! Secrets Management
//!
//! Provides secure storage for API keys using OS keyring with fallback
//! to file-based storage and environment variables.
//!
//! ## Storage Priority (checked in order)
//! 1. OS Keyring (most secure, UI-configured via `keyring` crate)
//! 2. File storage (base64-encoded, NOT encrypted - use only when keyring
//!    unavailable)
//! 3. Environment variables (backward compatibility)
//! 4. Not configured

use crate::config::secrets::{ApiKeyStatus, ApiProvider, SecretsError};
use std::path::PathBuf;

/// Manager for secure API key storage
///
/// Uses OS keyring for persistent storage with fallback to file storage
/// and environment variables.
#[derive(Debug, Clone, Default)]
pub struct SecretsManager;

impl SecretsManager {
    /// Create a new secrets manager
    pub fn new() -> Self {
        Self
    }

    /// Get the secrets file path for a provider
    fn secrets_file_path(provider: ApiProvider) -> Option<PathBuf> {
        let data_dir = crate::infra::platform::data_path(None);
        let secrets_dir = data_dir.join("secrets");
        Some(secrets_dir.join(format!("{}.key", provider.keyring_user())))
    }

    /// Get the API key for a provider
    ///
    /// Checks in order:
    /// 1. OS Keyring (preferred, UI-configured)
    /// 2. File storage (fallback when keyring unavailable)
    /// 3. Environment variable (backward compatibility)
    /// 4. Returns NotConfigured
    pub fn get_api_key(&self, provider: ApiProvider) -> ApiKeyStatus {
        // First, try keyring
        if let Some(key) = self.get_from_keyring(provider)
            && !key.is_empty()
        {
            return ApiKeyStatus::FromKeyring(key);
        }

        // Try file-based storage (fallback when keyring is unavailable).
        // Returns FromFile — key is base64-encoded on disk, NOT encrypted by the OS.
        if let Some(key) = self.get_from_file(provider)
            && !key.is_empty()
        {
            log::debug!(
                "Found {} key in file storage (keyring fallback)",
                provider.display_name()
            );
            return ApiKeyStatus::FromFile(key);
        }

        // Fall back to environment variable
        if let Ok(key) = std::env::var(provider.env_var())
            && !key.is_empty()
        {
            return ApiKeyStatus::FromEnv(key);
        }

        ApiKeyStatus::NotConfigured
    }

    /// Store an API key in the OS keyring (with file fallback)
    pub fn set_api_key(&self, provider: ApiProvider, key: &str) -> Result<(), SecretsError> {
        // Basic validation
        if key.is_empty() {
            return Err(SecretsError::InvalidKey(
                "API key cannot be empty".to_string(),
            ));
        }

        if key.len() < 10 {
            return Err(SecretsError::InvalidKey(
                "API key appears too short".to_string(),
            ));
        }

        log::debug!(
            "Attempting to store {} API key (len={}) in keyring \
             service='{}' user='{}'",
            provider.display_name(),
            key.len(),
            provider.keyring_service(),
            provider.keyring_user()
        );

        // Try keyring first
        let keyring_result = self.save_to_keyring(provider, key);

        // Always save to file as backup
        let file_result = self.save_to_file(provider, key);

        // If both failed, return error
        if let (Err(e), Err(_)) = (&keyring_result, &file_result) {
            return Err(e.clone());
        }

        if keyring_result.is_ok() {
            log::info!("Stored {} API key in OS keyring", provider.display_name());
        } else {
            log::info!(
                "Stored {} API key in file storage (keyring unavailable)",
                provider.display_name()
            );
        }

        Ok(())
    }

    /// Check if an API key is configured (from any source)
    pub fn has_api_key(&self, provider: ApiProvider) -> bool {
        self.get_api_key(provider).is_available()
    }

    // ── Per-feed password storage ────────────────────────────────────────

    const FEED_KEYRING_SERVICE: &'static str = "kairos-feed";

    fn feed_key_file_path(feed_id: &str) -> Option<std::path::PathBuf> {
        let data_dir = crate::infra::platform::data_path(None);
        Some(
            data_dir
                .join("secrets")
                .join(format!("feed-{}.key", feed_id)),
        )
    }

    /// Store a password for a specific feed connection.
    pub fn set_feed_password(&self, feed_id: &str, password: &str) -> Result<(), SecretsError> {
        if password.is_empty() {
            return Err(SecretsError::InvalidKey(
                "Password cannot be empty".to_string(),
            ));
        }

        let keyring_result = keyring::Entry::new(Self::FEED_KEYRING_SERVICE, feed_id)
            .map_err(|e| SecretsError::KeyringAccess(e.to_string()))
            .and_then(|entry| {
                entry
                    .set_password(password)
                    .map_err(|e| SecretsError::StoreFailed(e.to_string()))
            });

        // File fallback
        let file_result = Self::feed_key_file_path(feed_id)
            .ok_or_else(|| SecretsError::StoreFailed("No data path".to_string()))
            .and_then(|path| {
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)
                        .map_err(|e| SecretsError::StoreFailed(e.to_string()))?;
                }
                std::fs::write(&path, base64_encode(password))
                    .map_err(|e| SecretsError::StoreFailed(e.to_string()))
            });

        if let (Err(e), Err(_)) = (&keyring_result, &file_result) {
            return Err(e.clone());
        }

        log::info!("Stored password for feed {}", feed_id);
        Ok(())
    }

    /// Retrieve the password for a specific feed connection.
    pub fn get_feed_password(&self, feed_id: &str) -> Option<String> {
        // Try keyring first
        if let Ok(entry) = keyring::Entry::new(Self::FEED_KEYRING_SERVICE, feed_id)
            && let Ok(pw) = entry.get_password()
            && !pw.is_empty()
        {
            return Some(pw);
        }

        // File fallback
        if let Some(path) = Self::feed_key_file_path(feed_id)
            && path.exists()
            && let Ok(encoded) = std::fs::read_to_string(&path)
            && let Some(pw) = base64_decode(&encoded)
            && !pw.is_empty()
        {
            return Some(pw);
        }

        None
    }

    /// Check whether a password has been stored for a specific feed.
    pub fn has_feed_password(&self, feed_id: &str) -> bool {
        self.get_feed_password(feed_id).is_some()
    }

    /// Save key to OS keyring
    fn save_to_keyring(&self, provider: ApiProvider, key: &str) -> Result<(), SecretsError> {
        let entry = keyring::Entry::new(provider.keyring_service(), provider.keyring_user())
            .map_err(|e| {
                log::warn!("Failed to create keyring entry: {}", e);
                SecretsError::KeyringAccess(e.to_string())
            })?;

        entry.set_password(key).map_err(|e| {
            log::warn!("Failed to set password in keyring: {}", e);
            SecretsError::StoreFailed(e.to_string())
        })?;

        Ok(())
    }

    /// Save key to file storage (fallback)
    fn save_to_file(&self, provider: ApiProvider, key: &str) -> Result<(), SecretsError> {
        let file_path = Self::secrets_file_path(provider).ok_or_else(|| {
            SecretsError::StoreFailed("Could not determine secrets directory".to_string())
        })?;

        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                SecretsError::StoreFailed(format!("Failed to create secrets dir: {}", e))
            })?;
        }

        let encoded = base64_encode(key);
        std::fs::write(&file_path, encoded)
            .map_err(|e| SecretsError::StoreFailed(format!("Failed to write key file: {}", e)))?;

        log::debug!(
            "Saved {} key to file: {:?}",
            provider.display_name(),
            file_path
        );
        Ok(())
    }

    /// Read key from file storage
    fn get_from_file(&self, provider: ApiProvider) -> Option<String> {
        let file_path = Self::secrets_file_path(provider)?;

        if !file_path.exists() {
            return None;
        }

        match std::fs::read_to_string(&file_path) {
            Ok(encoded) => base64_decode(&encoded),
            Err(e) => {
                log::warn!("Failed to read key file: {}", e);
                None
            }
        }
    }

    /// Get the key from keyring only
    fn get_from_keyring(&self, provider: ApiProvider) -> Option<String> {
        log::debug!(
            "Reading {} key from keyring service='{}' user='{}'",
            provider.display_name(),
            provider.keyring_service(),
            provider.keyring_user()
        );

        let entry = match keyring::Entry::new(provider.keyring_service(), provider.keyring_user()) {
            Ok(e) => e,
            Err(e) => {
                log::warn!("Failed to create keyring entry for read: {}", e);
                return None;
            }
        };

        match entry.get_password() {
            Ok(password) => {
                log::debug!(
                    "Found {} key in keyring (len={})",
                    provider.display_name(),
                    password.len()
                );
                Some(password)
            }
            Err(keyring::Error::NoEntry) => {
                log::debug!("No {} key found in keyring", provider.display_name());
                None
            }
            Err(e) => {
                log::warn!(
                    "Failed to read {} key from keyring: {}",
                    provider.display_name(),
                    e
                );
                None
            }
        }
    }
}

fn base64_encode(input: &str) -> String {
    use base64::{Engine, engine::general_purpose::STANDARD};
    STANDARD.encode(input.as_bytes())
}

fn base64_decode(input: &str) -> Option<String> {
    use base64::{Engine, engine::general_purpose::STANDARD};
    STANDARD
        .decode(input.trim())
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
}
