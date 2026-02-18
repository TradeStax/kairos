//! Production-ready configuration system for FlowSurface
//!
//! Provides comprehensive configuration management with:
//! - Environment variable loading
//! - File-based config (TOML)
//! - Sensible defaults
//! - Validation
//! - Cost tracking

use crate::error::{Error, ExchangeResult};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Configuration profile for different environments
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConfigProfile {
    /// Development environment - aggressive caching, mock support
    Development,
    /// Staging environment - production-like with safety limits
    Staging,
    /// Production environment - optimized settings, strict validation
    Production,
}

impl ConfigProfile {
    /// Get profile from environment or default to development
    pub fn from_env() -> Self {
        match std::env::var("FLOWSURFACE_PROFILE")
            .unwrap_or_default()
            .to_lowercase()
            .as_str()
        {
            "production" | "prod" => ConfigProfile::Production,
            "staging" | "stage" => ConfigProfile::Staging,
            _ => ConfigProfile::Development,
        }
    }

    /// Get default configuration for profile
    pub fn defaults(&self) -> Config {
        match self {
            ConfigProfile::Development => Config {
                databento: DatabentoConfig {
                    api_key: String::new(),
                    dataset: "GLBX.MDP3".to_string(),
                    timeout_secs: 30,
                    max_retries: 3,
                    rate_limit_per_min: 60,
                },
                fetching: FetchingConfig {
                    auto_backfill: false,
                    max_fetch_days: 7,
                    depth_schema: "MBP-10".to_string(),
                    parallel_fetch: false,
                    max_concurrent_fetches: 2,
                    preferred_schemas: vec!["Trades".to_string(), "MBP-1".to_string()],
                },
                cache: CacheConfig {
                    enabled: true,
                    directory: dirs_next::cache_dir()
                        .unwrap_or_else(|| PathBuf::from(".cache"))
                        .join("flowsurface")
                        .join("dev"),
                    max_size_mb: 1000,
                    max_age_days: 30,
                    compression: false,
                    cleanup_on_startup: false,
                },
                cost: CostConfig {
                    warn_on_expensive: true,
                    max_cost_per_request: 10.0,
                    daily_budget: 100.0,
                    track_usage: true,
                    strict_mode: false,
                },
                network: NetworkConfig {
                    connect_timeout_secs: 10,
                    read_timeout_secs: 30,
                    max_retry_delay_secs: 60,
                    exponential_backoff: true,
                    keepalive_secs: Some(60),
                },
                validation: ValidationConfig {
                    strict: false,
                    check_connectivity: false,
                    validate_schemas: true,
                    check_permissions: true,
                },
            },
            ConfigProfile::Staging => Config {
                databento: DatabentoConfig {
                    api_key: String::new(),
                    dataset: "GLBX.MDP3".to_string(),
                    timeout_secs: 60,
                    max_retries: 5,
                    rate_limit_per_min: 120,
                },
                fetching: FetchingConfig {
                    auto_backfill: false,
                    max_fetch_days: 30,
                    depth_schema: "MBP-10".to_string(),
                    parallel_fetch: true,
                    max_concurrent_fetches: 4,
                    preferred_schemas: vec!["Trades".to_string(), "MBP-10".to_string()],
                },
                cache: CacheConfig {
                    enabled: true,
                    directory: dirs_next::cache_dir()
                        .unwrap_or_else(|| PathBuf::from(".cache"))
                        .join("flowsurface")
                        .join("staging"),
                    max_size_mb: 5000,
                    max_age_days: 60,
                    compression: true,
                    cleanup_on_startup: true,
                },
                cost: CostConfig {
                    warn_on_expensive: true,
                    max_cost_per_request: 50.0,
                    daily_budget: 500.0,
                    track_usage: true,
                    strict_mode: true,
                },
                network: NetworkConfig {
                    connect_timeout_secs: 20,
                    read_timeout_secs: 60,
                    max_retry_delay_secs: 120,
                    exponential_backoff: true,
                    keepalive_secs: Some(120),
                },
                validation: ValidationConfig {
                    strict: true,
                    check_connectivity: true,
                    validate_schemas: true,
                    check_permissions: true,
                },
            },
            ConfigProfile::Production => Config {
                databento: DatabentoConfig {
                    api_key: String::new(),
                    dataset: "GLBX.MDP3".to_string(),
                    timeout_secs: 120,
                    max_retries: 10,
                    rate_limit_per_min: 300,
                },
                fetching: FetchingConfig {
                    auto_backfill: true,
                    max_fetch_days: 90,
                    depth_schema: "MBP-10".to_string(),
                    parallel_fetch: true,
                    max_concurrent_fetches: 8,
                    preferred_schemas: vec![
                        "Trades".to_string(),
                        "MBP-10".to_string(),
                        "OHLCV-1D".to_string(),
                    ],
                },
                cache: CacheConfig {
                    enabled: true,
                    directory: dirs_next::cache_dir()
                        .unwrap_or_else(|| PathBuf::from("/var/cache"))
                        .join("flowsurface")
                        .join("prod"),
                    max_size_mb: 50000, // 50GB
                    max_age_days: 365,
                    compression: true,
                    cleanup_on_startup: false,
                },
                cost: CostConfig {
                    warn_on_expensive: true,
                    max_cost_per_request: 100.0,
                    daily_budget: 1000.0,
                    track_usage: true,
                    strict_mode: true,
                },
                network: NetworkConfig {
                    connect_timeout_secs: 30,
                    read_timeout_secs: 120,
                    max_retry_delay_secs: 300,
                    exponential_backoff: true,
                    keepalive_secs: Some(300),
                },
                validation: ValidationConfig {
                    strict: true,
                    check_connectivity: true,
                    validate_schemas: true,
                    check_permissions: true,
                },
            },
        }
    }
}

/// Complete configuration with validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub databento: DatabentoConfig,
    pub fetching: FetchingConfig,
    pub cache: CacheConfig,
    pub cost: CostConfig,
    pub network: NetworkConfig,
    pub validation: ValidationConfig,
}

impl Config {
    /// Create configuration for a specific profile
    pub fn for_profile(profile: ConfigProfile) -> Self {
        profile.defaults()
    }

    /// Load from environment and file with validation
    pub fn load() -> ExchangeResult<Self> {
        let profile = ConfigProfile::from_env();
        let mut config = profile.defaults();

        // Load API key from environment
        if let Ok(key) = std::env::var("DATABENTO_API_KEY") {
            config.databento.api_key = key;
        }

        // Try to load from file
        if let Some(path) = Self::config_file_path() {
            if path.exists() {
                if let Ok(file_config) = Self::load_from_file(&path) {
                    config = file_config;
                    log::info!("Loaded configuration from {:?}", path);
                }
            }
        }

        // Validate the final configuration
        config.validate()?;

        Ok(config)
    }

    /// Load from TOML file
    pub fn load_from_file(path: &PathBuf) -> ExchangeResult<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| Error::Config(format!("Failed to read config: {}", e)))?;

        toml::from_str(&content).map_err(|e| Error::Config(format!("Invalid TOML: {}", e)))
    }

    /// Save to TOML file
    pub fn save_to_file(&self, path: &PathBuf) -> ExchangeResult<()> {
        let content = toml::to_string_pretty(self)
            .map_err(|e| Error::Config(format!("Failed to serialize: {}", e)))?;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| Error::Config(format!("Failed to create directory: {}", e)))?;
        }

        std::fs::write(path, content)
            .map_err(|e| Error::Config(format!("Failed to write config: {}", e)))?;

        Ok(())
    }

    /// Get default config file path
    pub fn config_file_path() -> Option<PathBuf> {
        dirs_next::config_dir().map(|d| d.join("flowsurface").join("config.toml"))
    }

    /// Comprehensive validation
    pub fn validate(&self) -> ExchangeResult<()> {
        self.databento.validate()?;
        self.fetching.validate()?;
        self.cache.validate()?;
        self.cost.validate()?;
        self.network.validate()?;

        // Cross-field validation
        self.validate_consistency()?;

        Ok(())
    }

    /// Validate cross-field consistency
    fn validate_consistency(&self) -> ExchangeResult<()> {
        // Check that timeouts are consistent
        if self.network.read_timeout_secs < self.databento.timeout_secs {
            return Err(Error::Config(
                "Network read timeout must be >= Databento timeout".to_string(),
            ));
        }

        // Check cache size vs available disk space
        if self.cache.enabled {
            if let Ok(metadata) =
                std::fs::metadata(&self.cache.directory.parent().unwrap_or(&PathBuf::from("/")))
            {
                // Simple check - would need platform-specific code for accurate disk space
                if !metadata.is_dir() {
                    return Err(Error::Config(
                        "Cache directory parent must exist".to_string(),
                    ));
                }
            }
        }

        Ok(())
    }
}

/// Databento API configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabentoConfig {
    pub api_key: String,
    pub dataset: String,
    pub timeout_secs: u64,
    pub max_retries: u32,
    pub rate_limit_per_min: u32,
}

impl DatabentoConfig {
    pub fn validate(&self) -> ExchangeResult<()> {
        if self.api_key.is_empty() {
            return Err(Error::Config("API key is required".to_string()));
        }

        if self.api_key.len() < 20 || self.api_key.len() > 100 {
            return Err(Error::Config("API key length seems invalid".to_string()));
        }

        if self.timeout_secs == 0 || self.timeout_secs > 300 {
            return Err(Error::Config(
                "Timeout must be between 1-300 seconds".to_string(),
            ));
        }

        if self.max_retries > 20 {
            return Err(Error::Config("Max retries too high (max 20)".to_string()));
        }

        if self.rate_limit_per_min == 0 || self.rate_limit_per_min > 1000 {
            return Err(Error::Config(
                "Rate limit must be 1-1000 per minute".to_string(),
            ));
        }

        // Validate dataset
        if self.dataset != "GLBX.MDP3" {
            log::warn!("Only GLBX.MDP3 dataset is currently supported");
        }

        Ok(())
    }

    // test_connection removed: was a no-op stub
}

/// Data fetching configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchingConfig {
    pub auto_backfill: bool,
    pub max_fetch_days: u32,
    pub depth_schema: String,
    pub parallel_fetch: bool,
    pub max_concurrent_fetches: usize,
    pub preferred_schemas: Vec<String>,
}

impl FetchingConfig {
    pub fn validate(&self) -> ExchangeResult<()> {
        if self.max_fetch_days == 0 || self.max_fetch_days > 365 {
            return Err(Error::Config("Max fetch days must be 1-365".to_string()));
        }

        if self.max_concurrent_fetches == 0 || self.max_concurrent_fetches > 20 {
            return Err(Error::Config("Concurrent fetches must be 1-20".to_string()));
        }

        // Validate depth schema
        let valid_schemas = vec!["MBP-1", "MBP-10", "MBO", "Trades", "TBBO"];
        if !valid_schemas.contains(&self.depth_schema.as_str()) {
            return Err(Error::Config(format!(
                "Invalid depth schema: {}. Valid: {:?}",
                self.depth_schema, valid_schemas
            )));
        }

        // Validate preferred schemas
        for schema in &self.preferred_schemas {
            if !Self::is_valid_schema(schema) {
                return Err(Error::Config(format!("Invalid schema: {}", schema)));
            }
        }

        Ok(())
    }

    fn is_valid_schema(schema: &str) -> bool {
        matches!(
            schema,
            "Trades"
                | "MBP-1"
                | "MBP-10"
                | "MBO"
                | "TBBO"
                | "OHLCV-1S"
                | "OHLCV-1M"
                | "OHLCV-1H"
                | "OHLCV-1D"
        )
    }
}

/// Cache configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    pub enabled: bool,
    pub directory: PathBuf,
    pub max_size_mb: u64,
    pub max_age_days: u32,
    pub compression: bool,
    pub cleanup_on_startup: bool,
}

impl CacheConfig {
    pub fn validate(&self) -> ExchangeResult<()> {
        if self.enabled {
            // Check directory permissions
            if self.directory.exists() {
                let metadata = std::fs::metadata(&self.directory)
                    .map_err(|e| Error::Config(format!("Cannot access cache directory: {}", e)))?;

                if !metadata.is_dir() {
                    return Err(Error::Config("Cache path is not a directory".to_string()));
                }

                // Try to create a test file
                let test_file = self.directory.join(".write_test");
                if let Err(e) = std::fs::write(&test_file, "test") {
                    return Err(Error::Config(format!(
                        "Cache directory not writable: {}",
                        e
                    )));
                }
                let _ = std::fs::remove_file(test_file);
            }

            if self.max_size_mb < 100 {
                return Err(Error::Config(
                    "Cache size too small (min 100MB)".to_string(),
                ));
            }

            if self.max_size_mb > 1000000 {
                // 1TB
                return Err(Error::Config("Cache size too large (max 1TB)".to_string()));
            }

            if self.max_age_days == 0 || self.max_age_days > 3650 {
                return Err(Error::Config("Cache age must be 1-3650 days".to_string()));
            }
        }

        Ok(())
    }

    /// Get actual cache size on disk (recursive)
    pub fn get_cache_size(&self) -> ExchangeResult<u64> {
        if !self.directory.exists() {
            return Ok(0);
        }

        fn dir_size(path: &std::path::Path) -> std::io::Result<u64> {
            let mut total = 0u64;
            for entry in std::fs::read_dir(path)? {
                let entry = entry?;
                let metadata = entry.metadata()?;
                if metadata.is_dir() {
                    total += dir_size(&entry.path())?;
                } else {
                    total += metadata.len();
                }
            }
            Ok(total)
        }

        let total_bytes = dir_size(&self.directory)
            .map_err(|e| {
                Error::Config(format!(
                    "Cannot read cache directory: {}",
                    e
                ))
            })?;

        Ok(total_bytes / 1_048_576) // Convert to MB
    }
}

/// Cost management configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostConfig {
    pub warn_on_expensive: bool,
    pub max_cost_per_request: f64,
    pub daily_budget: f64,
    pub track_usage: bool,
    pub strict_mode: bool,
}

impl CostConfig {
    pub fn validate(&self) -> ExchangeResult<()> {
        if self.max_cost_per_request < 0.0 || self.max_cost_per_request > 10000.0 {
            return Err(Error::Config(
                "Max cost per request must be 0-10000".to_string(),
            ));
        }

        if self.daily_budget < 0.0 || self.daily_budget > 100000.0 {
            return Err(Error::Config("Daily budget must be 0-100000".to_string()));
        }

        if self.strict_mode && self.daily_budget == 0.0 {
            return Err(Error::Config(
                "Strict mode requires non-zero daily budget".to_string(),
            ));
        }

        Ok(())
    }

    /// Estimate cost for a schema and date range
    pub fn estimate_cost(&self, schema: &str, days: u32) -> f64 {
        let base_cost = match schema {
            "MBO" => 10.0,
            "MBP-10" => 3.0,
            "MBP-1" => 2.0,
            "Trades" => 2.0,
            "OHLCV-1D" => 0.5,
            "OHLCV-1H" => 0.8,
            "OHLCV-1M" => 1.0,
            _ => 5.0,
        };

        base_cost * days as f64
    }
}

/// Network configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub connect_timeout_secs: u64,
    pub read_timeout_secs: u64,
    pub max_retry_delay_secs: u64,
    pub exponential_backoff: bool,
    pub keepalive_secs: Option<u64>,
}

impl NetworkConfig {
    pub fn validate(&self) -> ExchangeResult<()> {
        if self.connect_timeout_secs == 0 || self.connect_timeout_secs > 60 {
            return Err(Error::Config(
                "Connect timeout must be 1-60 seconds".to_string(),
            ));
        }

        if self.read_timeout_secs == 0 || self.read_timeout_secs > 300 {
            return Err(Error::Config(
                "Read timeout must be 1-300 seconds".to_string(),
            ));
        }

        if self.max_retry_delay_secs < 1 || self.max_retry_delay_secs > 600 {
            return Err(Error::Config(
                "Max retry delay must be 1-600 seconds".to_string(),
            ));
        }

        if let Some(keepalive) = self.keepalive_secs {
            if keepalive == 0 || keepalive > 600 {
                return Err(Error::Config("Keepalive must be 1-600 seconds".to_string()));
            }
        }

        Ok(())
    }
}

/// Validation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationConfig {
    pub strict: bool,
    pub check_connectivity: bool,
    pub validate_schemas: bool,
    pub check_permissions: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_defaults() {
        let dev = Config::for_profile(ConfigProfile::Development);
        assert!(!dev.fetching.auto_backfill);
        assert_eq!(dev.cache.max_size_mb, 1000);

        let prod = Config::for_profile(ConfigProfile::Production);
        assert!(prod.fetching.auto_backfill);
        assert_eq!(prod.cache.max_size_mb, 50000);
    }

    #[test]
    fn test_validation() {
        let mut config = Config::for_profile(ConfigProfile::Development);

        // Should fail without API key
        assert!(config.validate().is_err());

        // Should pass with API key
        config.databento.api_key = "valid_api_key_1234567890".to_string();
        assert!(config.validate().is_ok());

        // Invalid timeout
        config.databento.timeout_secs = 0;
        assert!(config.validate().is_err());
    }
}
