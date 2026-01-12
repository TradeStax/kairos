//! Databento adapter for CME Globex futures market data
//!
//! This module provides integration with Databento's market data API for futures markets,
//! specifically targeting CME Globex (GLBX.MDP3 dataset).
//!
//! Features:
//! - Historical data fetching and caching
//! - Continuous contract resolution
//! - Trading hours tracking
//! - Contract expiration management
//! - Historical data replay system
//! - Live WebSocket streaming

// Consolidated module structure per architecture plan
pub mod cache; // Low-level cache operations
pub mod client; // HTTP client initialization
pub mod decoder; // DBN decoding utilities
pub mod fetcher; // Fetch orchestration and gap detection
pub mod mapper; // Type conversions (Databento → Domain)
// WebSocket module temporarily disabled (historical-only for now)
// pub mod websocket; // WebSocket client for live streaming

use super::AdapterError;
use databento::dbn::Dataset;
use std::path::PathBuf;

// Re-export main manager and utilities
pub use fetcher::HistoricalDataManager;
pub use mapper::{fetch_historical_prices, get_continuous_ticker_info};
// WebSocket client temporarily disabled (historical-only for now)
// pub use websocket::WebSocketClient;

/// Databento dataset identifier for CME Globex
pub const DATASET: Dataset = Dataset::GlbxMdp3;

/// Configuration for Databento API access (historical-only)
#[derive(Debug, Clone)]
pub struct DatabentoConfig {
    /// Databento API key (can be loaded from environment)
    pub api_key: String,
    /// Primary dataset to use
    pub dataset: Dataset,
    /// Enable local caching of historical data (HIGHLY RECOMMENDED to reduce costs)
    pub cache_enabled: bool,
    /// Maximum days to keep in cache
    pub cache_max_days: u32,
    /// Automatically backfill visible range on chart load (WARNING: can be expensive)
    pub auto_backfill: bool,
    /// Path to cache directory
    pub cache_dir: PathBuf,
    /// Warn on expensive API calls (MBO, large date ranges, etc.)
    pub warn_on_expensive_calls: bool,
}

impl Default for DatabentoConfig {
    fn default() -> Self {
        let cache_dir = dirs_next::cache_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("flowsurface")
            .join("databento");

        Self {
            api_key: std::env::var("DATABENTO_API_KEY").unwrap_or_default(),
            dataset: DATASET,
            cache_enabled: true,  // Always cache to minimize API costs
            cache_max_days: 90,   // Keep longer cache (data doesn't change)
            auto_backfill: false, // Manual backfill to control costs
            cache_dir,
            warn_on_expensive_calls: true, // Warn users about costly operations
        }
    }
}

impl DatabentoConfig {
    /// Create a new configuration with API key from environment
    pub fn from_env() -> Result<Self, AdapterError> {
        let api_key = std::env::var("DATABENTO_API_KEY").map_err(|_| {
            AdapterError::InvalidRequest(
                "DATABENTO_API_KEY environment variable not set".to_string(),
            )
        })?;

        Ok(Self {
            api_key,
            ..Default::default()
        })
    }

    /// Create a new configuration with explicit API key
    pub fn with_api_key(api_key: String) -> Self {
        Self {
            api_key,
            ..Default::default()
        }
    }

    /// Check if a schema is expensive
    pub fn is_expensive_schema(schema: databento::dbn::Schema) -> bool {
        matches!(
            schema,
            databento::dbn::Schema::Mbo // Market By Order - VERY expensive
        )
    }

    /// Estimate relative cost of a schema (1-10 scale, 10 being most expensive)
    pub fn schema_cost_estimate(schema: databento::dbn::Schema) -> u8 {
        use databento::dbn::Schema;
        match schema {
            Schema::Mbo => 10,       // Very expensive
            Schema::Mbp10 => 3,      // Moderate - aggregated 10 levels
            Schema::Mbp1 => 2,       // Low - just top of book
            Schema::Trades => 2,     // Low - reasonable volume
            Schema::Ohlcv1M => 1,    // Very cheap - aggregated
            Schema::Ohlcv1H => 1,    // Very cheap
            Schema::Ohlcv1D => 1,    // Very cheap
            Schema::Tbbo => 2,       // Low - just BBO updates
            Schema::Statistics => 1, // Very cheap - daily statistics
            _ => 5,                  // Unknown - assume moderate
        }
    }

    /// Get cost warning for date range size
    pub fn check_date_range_cost(
        &self,
        start: chrono::DateTime<chrono::Utc>,
        end: chrono::DateTime<chrono::Utc>,
        schema: databento::dbn::Schema,
    ) -> Option<String> {
        if !self.warn_on_expensive_calls {
            return None;
        }

        let days = (end - start).num_days();
        let schema_cost = Self::schema_cost_estimate(schema);

        // Warn if fetching expensive schema over multiple days
        if schema_cost >= 8 && days > 1 {
            return Some(format!(
                "WARNING: Fetching {:?} for {} days will be very expensive. Consider using MBP-10 or caching.",
                schema, days
            ));
        }

        if schema_cost >= 5 && days > 7 {
            return Some(format!(
                "WARNING: Fetching {:?} for {} days may be costly. Consider caching or smaller date ranges.",
                schema, days
            ));
        }

        None
    }
}

// FuturesVenue and ContractType moved to futures.rs module
// No duplication - using types from crate::futures

// FuturesInstrument and ExpirationInfo moved to futures.rs module
// Using types from crate::futures

/// Trading session type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionType {
    /// Pre-market session
    PreMarket,
    /// Regular trading hours
    Regular,
    /// Post-market session
    PostMarket,
    /// Market closed
    Closed,
}

/// Trading status for an instrument
#[derive(Debug, Clone)]
pub struct TradingStatus {
    /// Timestamp of status
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Whether market is currently trading
    pub is_trading: bool,
    /// Current session type
    pub session_type: SessionType,
}

/// Error types specific to databento operations
#[derive(Debug, thiserror::Error)]
pub enum DatabentoError {
    #[error("Databento API error: {0}")]
    Api(#[from] databento::Error),
    #[error("Databento DBN error: {0}")]
    Dbn(#[from] databento::dbn::Error),
    #[error("Symbol not found: {0}")]
    SymbolNotFound(String),
    #[error("Invalid instrument ID: {0}")]
    InvalidInstrumentId(u32),
    #[error("Cache error: {0}")]
    Cache(String),
    #[error("Configuration error: {0}")]
    Config(String),
}

impl From<DatabentoError> for AdapterError {
    fn from(err: DatabentoError) -> Self {
        match err {
            DatabentoError::Api(e) => {
                AdapterError::ParseError(format!("Databento API error: {}", e))
            }
            DatabentoError::Dbn(e) => {
                AdapterError::ParseError(format!("Databento DBN error: {}", e))
            }
            DatabentoError::SymbolNotFound(s) => {
                AdapterError::InvalidRequest(format!("Symbol not found: {}", s))
            }
            DatabentoError::InvalidInstrumentId(id) => {
                AdapterError::InvalidRequest(format!("Invalid instrument ID: {}", id))
            }
            DatabentoError::Cache(s) => AdapterError::ParseError(format!("Cache error: {}", s)),
            DatabentoError::Config(s) => {
                AdapterError::InvalidRequest(format!("Configuration error: {}", s))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flowsurface_data::domain::ContractType;

    #[test]
    fn test_contract_type_parse_continuous() {
        let ct = ContractType::parse("ES.c.0").unwrap();
        assert_eq!(ct, ContractType::Continuous(0));

        let ct = ContractType::parse("ES.c.1").unwrap();
        assert_eq!(ct, ContractType::Continuous(1));
    }

    #[test]
    fn test_contract_type_parse_specific() {
        let ct = ContractType::parse("ESH5").unwrap();
        assert_eq!(ct, ContractType::Specific("ESH5".to_string()));
    }

    #[test]
    fn test_contract_type_to_symbol() {
        let ct = ContractType::Continuous(0);
        assert_eq!(ct.to_symbol("ES"), "ES.c.0");

        let ct = ContractType::Specific("ESH5".to_string());
        assert_eq!(ct.to_symbol("ES"), "ESH5");
    }
}
