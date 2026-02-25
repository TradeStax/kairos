//! Secret/API key domain types
//!
//! Pure value types for API providers and key status. No I/O operations.
//! The actual key storage (keyring, file system) lives in the GUI crate's
//! `secrets` module.

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
    /// OpenRouter API for AI assistant
    OpenRouter,
}

impl ApiProvider {
    /// Get the keyring service name for this provider
    pub fn keyring_service(&self) -> &'static str {
        "kairos"
    }

    /// Get the keyring username (key identifier) for this provider
    pub fn keyring_user(&self) -> &'static str {
        match self {
            ApiProvider::Databento => "databento_api_key",
            ApiProvider::Massive => "massive_api_key",
            ApiProvider::Rithmic => "rithmic_password",
            ApiProvider::OpenRouter => "openrouter_api_key",
        }
    }

    /// Get the environment variable name for this provider
    pub fn env_var(&self) -> &'static str {
        match self {
            ApiProvider::Databento => "DATABENTO_API_KEY",
            ApiProvider::Massive => "MASSIVE_API_KEY",
            ApiProvider::Rithmic => "RITHMIC_PASSWORD",
            ApiProvider::OpenRouter => "OPENROUTER_API_KEY",
        }
    }

    /// Get the display name for this provider
    pub fn display_name(&self) -> &'static str {
        match self {
            ApiProvider::Databento => "Databento",
            ApiProvider::Massive => "Massive (Polygon)",
            ApiProvider::Rithmic => "Rithmic",
            ApiProvider::OpenRouter => "OpenRouter",
        }
    }

    /// Get the URL to the provider's API key page
    pub fn api_key_url(&self) -> &'static str {
        match self {
            ApiProvider::Databento => "https://databento.com/portal/keys",
            ApiProvider::Massive => {
                "https://polygon.io/dashboard/api-keys"
            }
            ApiProvider::Rithmic => "https://rithmic.com",
            ApiProvider::OpenRouter => "https://openrouter.ai/keys",
        }
    }

    /// Get a brief description of what this API key is used for
    pub fn description(&self) -> &'static str {
        match self {
            ApiProvider::Databento => {
                "Required for CME futures market data (ES, NQ, etc.)"
            }
            ApiProvider::Massive => {
                "Required for US options data and GEX analysis"
            }
            ApiProvider::Rithmic => {
                "Required for Rithmic realtime + historical futures data"
            }
            ApiProvider::OpenRouter => {
                "Required for AI assistant (supports many LLM providers)"
            }
        }
    }

    /// List all providers
    pub fn all() -> &'static [ApiProvider] {
        &[
            ApiProvider::Databento,
            ApiProvider::Massive,
            ApiProvider::Rithmic,
            ApiProvider::OpenRouter,
        ]
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
    /// Key was found in OS keyring (UI-configured, encrypted by OS credential store)
    FromKeyring(String),
    /// Key was loaded from the file-based fallback (base64-encoded, NOT encrypted).
    /// Used when the OS keyring is unavailable (e.g., headless, unsupported platform).
    FromFile(String),
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
            ApiKeyStatus::FromKeyring(key)
            | ApiKeyStatus::FromFile(key)
            | ApiKeyStatus::FromEnv(key) => Some(key),
            ApiKeyStatus::NotConfigured => None,
        }
    }

    /// Get a description of the key source
    pub fn source_description(&self) -> &'static str {
        match self {
            ApiKeyStatus::FromKeyring(_) => "Configured in app (keyring)",
            ApiKeyStatus::FromFile(_) => "Configured in app (file)",
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_properties() {
        assert_eq!(ApiProvider::Databento.env_var(), "DATABENTO_API_KEY");
        assert_eq!(ApiProvider::Massive.env_var(), "MASSIVE_API_KEY");
        assert_eq!(ApiProvider::Databento.display_name(), "Databento");
        assert_eq!(
            ApiProvider::Massive.display_name(),
            "Massive (Polygon)"
        );
    }

    #[test]
    fn test_api_key_status() {
        let from_keyring =
            ApiKeyStatus::FromKeyring("test_key".to_string());
        assert!(from_keyring.is_available());
        assert_eq!(from_keyring.key(), Some("test_key"));

        let from_file = ApiKeyStatus::FromFile("file_key".to_string());
        assert!(from_file.is_available());
        assert_eq!(from_file.key(), Some("file_key"));
        assert_eq!(from_file.source_description(), "Configured in app (file)");

        let not_configured = ApiKeyStatus::NotConfigured;
        assert!(!not_configured.is_available());
        assert_eq!(not_configured.key(), None);
    }
}
