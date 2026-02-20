//! Massive API Adapter
//!
//! Provides integration with the Massive (Polygon) API for options data.
//! Supports fetching option chains, Greeks, implied volatility, and contract metadata.
//!
//! ## Features
//! - Historical option snapshots with Greeks and IV
//! - Full option chain retrieval
//! - Contract metadata and specifications
//! - Per-day caching with gap detection
//! - Rate limiting and cost management
//!
//! ## Architecture
//! ```text
//! OptionsDataService (data layer)
//!      ↓
//! MassiveSnapshotRepository (repository impl)
//!      ↓
//! HistoricalOptionsManager (this module)
//!      ├─→ [JSON cache files] (fast, local)
//!      └─→ Massive API (REST, rate limited)
//! ```

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

pub mod cache;
pub mod client;
pub mod decoder;
pub mod fetcher;
pub mod mapper;

// Re-export main types
pub use cache::CacheManager;
pub use client::create_client;
pub use fetcher::HistoricalOptionsManager;
pub use mapper::{convert_chain_response, convert_contract_response, convert_snapshot_response};

/// Massive API configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MassiveConfig {
    /// API key for authentication
    pub api_key: String,

    /// Enable local caching
    #[serde(default = "default_cache_enabled")]
    pub cache_enabled: bool,

    /// Maximum age of cache files in days
    #[serde(default = "default_cache_max_days")]
    pub cache_max_days: u32,

    /// Cache directory path
    #[serde(default = "default_cache_dir")]
    pub cache_dir: PathBuf,

    /// Rate limit (requests per minute)
    #[serde(default = "default_rate_limit")]
    pub rate_limit_per_minute: u32,

    /// Warn when approaching rate limits
    #[serde(default = "default_warn_on_rate_limits")]
    pub warn_on_rate_limits: bool,

    /// Request timeout in seconds
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,

    /// Maximum retries for failed requests
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,

    /// Retry delay in milliseconds
    #[serde(default = "default_retry_delay_ms")]
    pub retry_delay_ms: u64,
}

fn default_cache_enabled() -> bool {
    true
}

fn default_cache_max_days() -> u32 {
    90 // 3 months
}

fn default_cache_dir() -> PathBuf {
    dirs_next::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("flowsurface")
        .join("massive")
}

fn default_rate_limit() -> u32 {
    5 // Conservative default (Massive typically allows more)
}

fn default_warn_on_rate_limits() -> bool {
    true
}

fn default_timeout_secs() -> u64 {
    30
}

fn default_max_retries() -> u32 {
    3
}

fn default_retry_delay_ms() -> u64 {
    1000
}

impl MassiveConfig {
    /// Create a new configuration with API key
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            cache_enabled: default_cache_enabled(),
            cache_max_days: default_cache_max_days(),
            cache_dir: default_cache_dir(),
            rate_limit_per_minute: default_rate_limit(),
            warn_on_rate_limits: default_warn_on_rate_limits(),
            timeout_secs: default_timeout_secs(),
            max_retries: default_max_retries(),
            retry_delay_ms: default_retry_delay_ms(),
        }
    }

    /// Create from environment variable
    pub fn from_env() -> Result<Self, MassiveError> {
        let api_key = std::env::var("MASSIVE_API_KEY")
            .map_err(|_| MassiveError::Config("MASSIVE_API_KEY not set".to_string()))?;

        Ok(Self::new(api_key))
    }

    /// Create configuration from SecretsManager (keyring with env fallback)
    pub fn from_secrets() -> Result<Self, MassiveError> {
        use flowsurface_data::{ApiKeyStatus, ApiProvider, SecretsManager};

        let secrets = SecretsManager::new();
        match secrets.get_api_key(ApiProvider::Massive) {
            ApiKeyStatus::FromKeyring(key) | ApiKeyStatus::FromEnv(key) => Ok(Self::new(key)),
            ApiKeyStatus::NotConfigured => Err(MassiveError::Config(
                "Massive API key not configured. Set via UI or MASSIVE_API_KEY environment variable.".to_string(),
            )),
        }
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<(), MassiveError> {
        if self.api_key.is_empty() {
            return Err(MassiveError::Config("API key is empty".to_string()));
        }

        if self.api_key.len() < 10 {
            return Err(MassiveError::Config("API key is too short".to_string()));
        }

        if self.cache_max_days == 0 {
            return Err(MassiveError::Config(
                "cache_max_days must be greater than 0".to_string(),
            ));
        }

        if self.rate_limit_per_minute == 0 {
            return Err(MassiveError::Config(
                "rate_limit_per_minute must be greater than 0".to_string(),
            ));
        }

        if self.timeout_secs == 0 {
            return Err(MassiveError::Config(
                "timeout_secs must be greater than 0".to_string(),
            ));
        }

        Ok(())
    }

    /// Get cache directory for a specific data type
    pub fn cache_dir_for(&self, data_type: &str) -> PathBuf {
        self.cache_dir.join(data_type)
    }
}

impl Default for MassiveConfig {
    fn default() -> Self {
        Self::new(String::new())
    }
}

/// Massive API error types
#[derive(Error, Debug)]
pub enum MassiveError {
    /// API request error
    #[error("API request failed: {0}")]
    Api(String),

    /// HTTP client error
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// Rate limit exceeded
    #[error("Rate limit exceeded: {0}")]
    RateLimit(String),

    /// Response parsing error
    #[error("Failed to parse response: {0}")]
    Parse(String),

    /// Cache error
    #[error("Cache error: {0}")]
    Cache(String),

    /// Symbol not found
    #[error("Symbol not found: {0}")]
    SymbolNotFound(String),

    /// Invalid contract ticker
    #[error("Invalid contract ticker: {0}")]
    InvalidContractTicker(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(String),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization/deserialization error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Date/time error
    #[error("DateTime error: {0}")]
    DateTime(String),

    /// Invalid data
    #[error("Invalid data: {0}")]
    InvalidData(String),

    /// Timeout
    #[error("Request timeout: {0}")]
    Timeout(String),

    /// Authentication error
    #[error("Authentication failed: {0}")]
    Auth(String),
}

impl MassiveError {
    /// Get user-friendly error message
    pub fn user_message(&self) -> String {
        match self {
            MassiveError::Api(msg) => format!("API error: {}", msg),
            MassiveError::Http(_) => "Network connection error".to_string(),
            MassiveError::RateLimit(_) => "Rate limit exceeded, please wait".to_string(),
            MassiveError::Parse(_) => "Failed to parse server response".to_string(),
            MassiveError::Cache(_) => "Cache access error".to_string(),
            MassiveError::SymbolNotFound(symbol) => format!("Symbol '{}' not found", symbol),
            MassiveError::InvalidContractTicker(_) => "Invalid option contract ticker".to_string(),
            MassiveError::Config(_) => "Configuration error".to_string(),
            MassiveError::Io(_) => "File system error".to_string(),
            MassiveError::Json(_) => "Data format error".to_string(),
            MassiveError::DateTime(_) => "Date/time parsing error".to_string(),
            MassiveError::InvalidData(_) => "Invalid data received".to_string(),
            MassiveError::Timeout(_) => "Request timeout".to_string(),
            MassiveError::Auth(_) => "Authentication failed, check API key".to_string(),
        }
    }

    /// Check if error is retriable
    pub fn is_retriable(&self) -> bool {
        matches!(
            self,
            MassiveError::Http(_)
                | MassiveError::Timeout(_)
                | MassiveError::Api(_)
                | MassiveError::RateLimit(_)
        )
    }

    /// Get error severity
    pub fn severity(&self) -> flowsurface_data::domain::error::ErrorSeverity {
        use flowsurface_data::domain::error::ErrorSeverity;
        match self {
            MassiveError::Config(_) | MassiveError::Auth(_) => ErrorSeverity::Critical,
            MassiveError::SymbolNotFound(_) | MassiveError::InvalidContractTicker(_) => {
                ErrorSeverity::Warning
            }
            MassiveError::RateLimit(_) => ErrorSeverity::Recoverable,
            _ => ErrorSeverity::Recoverable,
        }
    }
}

/// Result type alias for Massive operations
pub type MassiveResult<T> = Result<T, MassiveError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_validation() {
        let mut config = MassiveConfig::new("test_api_key_123".to_string());
        assert!(config.validate().is_ok());

        config.api_key = String::new();
        assert!(config.validate().is_err());

        config.api_key = "short".to_string();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_defaults() {
        let config = MassiveConfig::new("test_key".to_string());
        assert_eq!(config.cache_enabled, true);
        assert_eq!(config.cache_max_days, 90);
        assert_eq!(config.rate_limit_per_minute, 5);
        assert_eq!(config.timeout_secs, 30);
    }

    #[test]
    fn test_error_retriability() {
        // Test that config errors are not retriable
        let config_err = MassiveError::Config("test".to_string());
        assert!(!config_err.is_retriable());

        // Test that API errors can be retriable
        let api_err = MassiveError::Api("Rate limit exceeded".to_string());
        assert!(api_err.is_retriable());
    }

    #[test]
    fn test_error_severity() {
        use flowsurface_data::domain::error::ErrorSeverity;

        let auth_err = MassiveError::Auth("test".to_string());
        assert_eq!(auth_err.severity(), ErrorSeverity::Critical);

        let symbol_err = MassiveError::SymbolNotFound("TEST".to_string());
        assert_eq!(symbol_err.severity(), ErrorSeverity::Warning);

        let rate_err = MassiveError::RateLimit("test".to_string());
        assert_eq!(rate_err.severity(), ErrorSeverity::Recoverable);
    }
}
