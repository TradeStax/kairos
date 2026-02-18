//! Secrets Management Layer
//!
//! Provides secure storage for API keys using OS keyring with fallback to environment variables.
//!
//! ## Storage Priority (checked in order)
//! 1. OS Keyring (UI-configured via `keyring` crate)
//! 2. Environment variables (backward compatibility)
//! 3. Not configured
//!
//! ## Usage
//! ```rust
//! use flowsurface_data::secrets::{SecretsManager, ApiProvider, ApiKeyStatus};
//!
//! let manager = SecretsManager::new();
//! match manager.get_api_key(ApiProvider::Databento) {
//!     ApiKeyStatus::FromKeyring(key) => { /* use key */ }
//!     ApiKeyStatus::FromEnv(key) => { /* use key */ }
//!     ApiKeyStatus::NotConfigured => { /* prompt user */ }
//! }
//! ```

use std::path::PathBuf;
use thiserror::Error;

/// API providers that require authentication
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ApiProvider {
    /// Databento API for CME futures data
    Databento,
    /// Massive (Polygon) API for US options data
    Massive,
    /// Rithmic for realtime + historical futures data
    Rithmic,
}

impl ApiProvider {
    /// Get the keyring service name for this provider
    pub fn keyring_service(&self) -> &'static str {
        "flowsurface"
    }

    /// Get the keyring username (key identifier) for this provider
    pub fn keyring_user(&self) -> &'static str {
        match self {
            ApiProvider::Databento => "databento_api_key",
            ApiProvider::Massive => "massive_api_key",
            ApiProvider::Rithmic => "rithmic_password",
        }
    }

    /// Get the environment variable name for this provider
    pub fn env_var(&self) -> &'static str {
        match self {
            ApiProvider::Databento => "DATABENTO_API_KEY",
            ApiProvider::Massive => "MASSIVE_API_KEY",
            ApiProvider::Rithmic => "RITHMIC_PASSWORD",
        }
    }

    /// Get the display name for this provider
    pub fn display_name(&self) -> &'static str {
        match self {
            ApiProvider::Databento => "Databento",
            ApiProvider::Massive => "Massive (Polygon)",
            ApiProvider::Rithmic => "Rithmic",
        }
    }

    /// Get the URL to the provider's API key page
    pub fn api_key_url(&self) -> &'static str {
        match self {
            ApiProvider::Databento => "https://databento.com/portal/keys",
            ApiProvider::Massive => "https://polygon.io/dashboard/api-keys",
            ApiProvider::Rithmic => "https://rithmic.com",
        }
    }

    /// Get a brief description of what this API key is used for
    pub fn description(&self) -> &'static str {
        match self {
            ApiProvider::Databento => "Required for CME futures market data (ES, NQ, etc.)",
            ApiProvider::Massive => "Required for US options data and GEX analysis",
            ApiProvider::Rithmic => {
                "Required for Rithmic realtime + historical futures data"
            }
        }
    }

    /// List all providers
    pub fn all() -> &'static [ApiProvider] {
        &[ApiProvider::Databento, ApiProvider::Massive, ApiProvider::Rithmic]
    }
}

impl std::fmt::Display for ApiProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// Status of an API key
#[derive(Debug, Clone)]
pub enum ApiKeyStatus {
    /// Key was found in OS keyring (UI-configured)
    FromKeyring(String),
    /// Key was found in environment variable
    FromEnv(String),
    /// Key is not configured anywhere
    NotConfigured,
}

impl ApiKeyStatus {
    /// Check if a key is available (from any source)
    pub fn is_available(&self) -> bool {
        !matches!(self, ApiKeyStatus::NotConfigured)
    }

    /// Get the key value if available
    pub fn key(&self) -> Option<&str> {
        match self {
            ApiKeyStatus::FromKeyring(key) | ApiKeyStatus::FromEnv(key) => Some(key),
            ApiKeyStatus::NotConfigured => None,
        }
    }

    /// Get a description of the key source
    pub fn source_description(&self) -> &'static str {
        match self {
            ApiKeyStatus::FromKeyring(_) => "Configured in app",
            ApiKeyStatus::FromEnv(_) => "From environment variable",
            ApiKeyStatus::NotConfigured => "Not configured",
        }
    }
}

/// Errors that can occur when managing secrets
#[derive(Error, Debug, Clone)]
pub enum SecretsError {
    /// Failed to access keyring
    #[error("Keyring access failed: {0}")]
    KeyringAccess(String),

    /// Failed to store key in keyring
    #[error("Failed to store key: {0}")]
    StoreFailed(String),

    /// Failed to delete key from keyring
    #[error("Failed to delete key: {0}")]
    DeleteFailed(String),

    /// Key validation failed
    #[error("Invalid API key: {0}")]
    InvalidKey(String),
}

/// Manager for secure API key storage
///
/// Uses OS keyring for persistent storage with fallback to file storage and environment variables.
#[derive(Debug, Clone, Default)]
pub struct SecretsManager;

impl SecretsManager {
    /// Create a new secrets manager
    pub fn new() -> Self {
        Self
    }

    /// Get the secrets file path for a provider
    fn secrets_file_path(provider: ApiProvider) -> Option<PathBuf> {
        let data_dir = crate::data_path(None);
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
            && !key.is_empty() {
                return ApiKeyStatus::FromKeyring(key);
            }

        // Try file-based storage (fallback)
        if let Some(key) = self.get_from_file(provider)
            && !key.is_empty() {
                log::debug!("Found {} key in file storage", provider.display_name());
                return ApiKeyStatus::FromKeyring(key); // Report as "configured in app"
            }

        // Fall back to environment variable
        if let Ok(key) = std::env::var(provider.env_var())
            && !key.is_empty() {
                return ApiKeyStatus::FromEnv(key);
            }

        ApiKeyStatus::NotConfigured
    }

    /// Store an API key in the OS keyring (with file fallback)
    pub fn set_api_key(&self, provider: ApiProvider, key: &str) -> Result<(), SecretsError> {
        // Basic validation
        if key.is_empty() {
            return Err(SecretsError::InvalidKey("API key cannot be empty".to_string()));
        }

        if key.len() < 10 {
            return Err(SecretsError::InvalidKey(
                "API key appears too short".to_string(),
            ));
        }

        log::debug!(
            "Attempting to store {} API key (len={}) in keyring service='{}' user='{}'",
            provider.display_name(),
            key.len(),
            provider.keyring_service(),
            provider.keyring_user()
        );

        // Try keyring first
        let keyring_result = self.save_to_keyring(provider, key);

        // Always save to file as backup (keyring may not work on all systems)
        let file_result = self.save_to_file(provider, key);

        // If both failed, return error
        if let (Err(e), Err(_)) = (&keyring_result, &file_result) {
            return Err(e.clone());
        }

        if keyring_result.is_ok() {
            log::info!(
                "Stored {} API key in OS keyring",
                provider.display_name()
            );
        } else {
            log::info!(
                "Stored {} API key in file storage (keyring unavailable)",
                provider.display_name()
            );
        }

        Ok(())
    }

    /// Save key to OS keyring
    fn save_to_keyring(&self, provider: ApiProvider, key: &str) -> Result<(), SecretsError> {
        let entry = keyring::Entry::new(provider.keyring_service(), provider.keyring_user())
            .map_err(|e| {
                log::warn!("Failed to create keyring entry: {}", e);
                SecretsError::KeyringAccess(e.to_string())
            })?;

        entry
            .set_password(key)
            .map_err(|e| {
                log::warn!("Failed to set password in keyring: {}", e);
                SecretsError::StoreFailed(e.to_string())
            })?;

        Ok(())
    }

    /// Save key to file storage (fallback)
    fn save_to_file(&self, provider: ApiProvider, key: &str) -> Result<(), SecretsError> {
        let file_path = Self::secrets_file_path(provider)
            .ok_or_else(|| SecretsError::StoreFailed("Could not determine secrets directory".to_string()))?;

        // Create secrets directory if needed
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| SecretsError::StoreFailed(format!("Failed to create secrets dir: {}", e)))?;
        }

        // Write key to file (simple obfuscation - not secure, but better than plaintext)
        let encoded = base64_encode(key);
        std::fs::write(&file_path, encoded)
            .map_err(|e| SecretsError::StoreFailed(format!("Failed to write key file: {}", e)))?;

        log::debug!("Saved {} key to file: {:?}", provider.display_name(), file_path);
        Ok(())
    }

    /// Read key from file storage
    fn get_from_file(&self, provider: ApiProvider) -> Option<String> {
        let file_path = Self::secrets_file_path(provider)?;

        if !file_path.exists() {
            return None;
        }

        match std::fs::read_to_string(&file_path) {
            Ok(encoded) => {
                base64_decode(&encoded)
            }
            Err(e) => {
                log::warn!("Failed to read key file: {}", e);
                None
            }
        }
    }

    /// Delete an API key from the OS keyring
    pub fn delete_api_key(&self, provider: ApiProvider) -> Result<(), SecretsError> {
        let entry = keyring::Entry::new(provider.keyring_service(), provider.keyring_user())
            .map_err(|e| SecretsError::KeyringAccess(e.to_string()))?;

        // Try to delete - if it doesn't exist, that's okay
        match entry.delete_credential() {
            Ok(()) => {
                log::info!(
                    "Deleted {} API key from OS keyring",
                    provider.display_name()
                );
                Ok(())
            }
            Err(keyring::Error::NoEntry) => {
                // Key didn't exist, that's fine
                Ok(())
            }
            Err(e) => Err(SecretsError::DeleteFailed(e.to_string())),
        }
    }

    /// Check if an API key is configured (from any source)
    pub fn has_api_key(&self, provider: ApiProvider) -> bool {
        self.get_api_key(provider).is_available()
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
                log::debug!("Found {} key in keyring (len={})", provider.display_name(), password.len());
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

/// Simple base64 encoding (for basic obfuscation of stored keys)
fn base64_encode(input: &str) -> String {
    use base64::{Engine, engine::general_purpose::STANDARD};
    STANDARD.encode(input.as_bytes())
}

/// Simple base64 decoding
fn base64_decode(input: &str) -> Option<String> {
    use base64::{Engine, engine::general_purpose::STANDARD};
    STANDARD.decode(input.trim())
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_properties() {
        assert_eq!(ApiProvider::Databento.env_var(), "DATABENTO_API_KEY");
        assert_eq!(ApiProvider::Massive.env_var(), "MASSIVE_API_KEY");
        assert_eq!(ApiProvider::Databento.display_name(), "Databento");
        assert_eq!(ApiProvider::Massive.display_name(), "Massive (Polygon)");
    }

    #[test]
    fn test_api_key_status() {
        let from_keyring = ApiKeyStatus::FromKeyring("test_key".to_string());
        assert!(from_keyring.is_available());
        assert_eq!(from_keyring.key(), Some("test_key"));

        let not_configured = ApiKeyStatus::NotConfigured;
        assert!(!not_configured.is_available());
        assert_eq!(not_configured.key(), None);
    }

    #[test]
    fn test_key_validation() {
        let manager = SecretsManager::new();

        // Empty key should fail
        let result = manager.set_api_key(ApiProvider::Databento, "");
        assert!(result.is_err());

        // Too short key should fail
        let result = manager.set_api_key(ApiProvider::Databento, "short");
        assert!(result.is_err());
    }
}
