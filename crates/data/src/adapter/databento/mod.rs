//! Databento adapter for CME Globex historical futures data.
//!
//! Feature-gated: `feature = "databento"`.
//!
//! - [`DatabentoAdapter`] — fetches trades and depth, caches per-day,
//!   scans cache for [`crate::domain::index::DataIndex`]
//! - [`DatabentoConfig`] — API key, dataset, cache settings, cost warnings
//! - [`fetcher`] — download orchestration, gap detection, per-day cache logic
//! - [`decoder`] — `.dbn.zst` decompression and record extraction
//! - [`mapper`] — Databento price (10^-9) to domain `Price` (10^-8) conversion
//! - [`symbology`] — continuous contract symbol resolution and tick size lookup
//! - [`client`] — thin wrapper around the `databento` HTTP client

pub mod client;
pub mod decoder;
pub mod fetcher;
pub mod mapper;
pub mod symbology;

use std::path::PathBuf;

use databento::dbn::Dataset;

pub use fetcher::DatabentoAdapter;
pub use mapper::convert_databento_price;
pub use symbology::get_continuous_ticker_info;

/// Databento dataset identifier for CME Globex
pub const DATASET: Dataset = Dataset::GlbxMdp3;

/// Configuration for Databento API access and caching behavior
#[derive(Debug, Clone)]
pub struct DatabentoConfig {
    /// Databento API key for authentication
    pub api_key: String,
    /// Target dataset (defaults to CME Globex MDP3)
    pub dataset: Dataset,
    /// Whether to enable local per-day caching
    pub cache_enabled: bool,
    /// Maximum number of days to retain in the cache
    pub cache_max_days: u32,
    /// Whether to automatically backfill missing days on fetch
    pub auto_backfill: bool,
    /// Directory path for the local cache store
    pub cache_dir: PathBuf,
    /// Whether to emit warnings before expensive API calls
    pub warn_on_expensive_calls: bool,
}

impl Default for DatabentoConfig {
    fn default() -> Self {
        let cache_dir = dirs_next::cache_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("kairos")
            .join("databento");

        Self {
            api_key: std::env::var("DATABENTO_API_KEY").unwrap_or_default(),
            dataset: DATASET,
            cache_enabled: true,
            cache_max_days: 90,
            auto_backfill: false,
            cache_dir,
            warn_on_expensive_calls: true,
        }
    }
}

impl DatabentoConfig {
    /// Creates a config with the given API key and default settings
    pub fn with_api_key(api_key: String) -> Self {
        Self {
            api_key,
            ..Default::default()
        }
    }

    /// Returns `true` if the schema is known to be very expensive (e.g. MBO)
    #[must_use]
    pub fn is_expensive_schema(schema: databento::dbn::Schema) -> bool {
        matches!(schema, databento::dbn::Schema::Mbo)
    }

    /// Returns a relative cost estimate (1-10) for the given schema
    #[must_use]
    pub fn schema_cost_estimate(schema: databento::dbn::Schema) -> u8 {
        use databento::dbn::Schema;
        match schema {
            Schema::Mbo => 10,
            Schema::Mbp10 => 3,
            Schema::Mbp1 => 2,
            Schema::Trades => 2,
            Schema::Ohlcv1M => 1,
            Schema::Ohlcv1H => 1,
            Schema::Ohlcv1D => 1,
            Schema::Tbbo => 2,
            Schema::Statistics => 1,
            _ => 5,
        }
    }

    /// Returns a warning message if the date range + schema combination
    /// is likely to be expensive, or `None` if the cost is acceptable
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
        if schema_cost >= 8 && days > 1 {
            return Some(format!(
                "WARNING: Fetching {:?} for {} days will be very expensive.",
                schema, days
            ));
        }
        if schema_cost >= 5 && days > 7 {
            return Some(format!(
                "WARNING: Fetching {:?} for {} days may be costly.",
                schema, days
            ));
        }
        None
    }
}

/// Error types specific to Databento operations
#[derive(Debug, thiserror::Error)]
pub enum DatabentoError {
    /// Error from the Databento REST API
    #[error("Databento API error: {0}")]
    Api(#[from] databento::Error),
    /// Error decoding DBN wire format
    #[error("Databento DBN error: {0}")]
    Dbn(#[from] databento::dbn::Error),
    /// Requested symbol was not found in the dataset
    #[error("Symbol not found: {0}")]
    SymbolNotFound(String),
    /// Instrument ID could not be resolved to a symbol
    #[error("Invalid instrument ID: {0}")]
    InvalidInstrumentId(u32),
    /// Local cache read/write failure
    #[error("Cache error: {0}")]
    Cache(String),
    /// Invalid or missing configuration
    #[error("Configuration error: {0}")]
    Config(String),
}

impl From<DatabentoError> for crate::Error {
    fn from(e: DatabentoError) -> Self {
        match e {
            DatabentoError::Api(e) => crate::Error::Fetch(e.to_string()),
            DatabentoError::Dbn(e) => crate::Error::Fetch(e.to_string()),
            DatabentoError::SymbolNotFound(s) => crate::Error::Symbol(s),
            DatabentoError::InvalidInstrumentId(id) => {
                crate::Error::Symbol(format!("Invalid instrument ID: {}", id))
            }
            DatabentoError::Cache(s) => crate::Error::Cache(s),
            DatabentoError::Config(s) => crate::Error::Config(s),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::ContractType;

    #[test]
    fn test_contract_type_parse_continuous() {
        let ct = ContractType::parse("ES.c.0").unwrap();
        assert_eq!(ct, ContractType::Continuous(0));
    }

    #[test]
    fn test_contract_type_parse_specific() {
        let ct = ContractType::parse("ESH5").unwrap();
        assert_eq!(ct, ContractType::Specific("ESH5".to_string()));
    }
}
