//! Repository Traits
//!
//! Defines the interface for data repositories.
//! Implementations can be cached, remote, in-memory, etc.

use crate::domain::{DateRange, DepthSnapshot, Trade};
use crate::domain::{FuturesTicker, OptionChain, OptionContract, OptionSnapshot};
use chrono::NaiveDate;
use std::fmt;
use thiserror::Error;

/// Cache coverage report
#[derive(Debug, Clone)]
pub struct CacheCoverageReport {
    pub cached_count: usize,
    pub uncached_count: usize,
    pub gaps: Vec<(chrono::NaiveDate, chrono::NaiveDate)>, // (start, end) inclusive
    pub cached_dates: Vec<chrono::NaiveDate>, // List of all cached dates
}

/// Repository error types
#[derive(Error, Debug)]
pub enum RepositoryError {
    #[error("Data not found: {0}")]
    NotFound(String),

    #[error("Cache error: {0}")]
    Cache(String),

    #[error("Remote error: {0}")]
    Remote(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid data: {0}")]
    InvalidData(String),
}

pub type RepositoryResult<T> = Result<T, RepositoryError>;

/// Trade repository interface
///
/// Provides access to tick-by-tick trade data.
/// Implementations can cache, fetch from API, or use in-memory storage.
#[async_trait::async_trait]
pub trait TradeRepository: Send + Sync {
    /// Get trades for a ticker in a date range
    ///
    /// This is the primary method for fetching trade data.
    /// Implementations should:
    /// 1. Check cache first
    /// 2. Identify gaps
    /// 3. Fetch missing data
    /// 4. Cache results
    /// 5. Return complete dataset
    async fn get_trades(
        &self,
        ticker: &FuturesTicker,
        date_range: &DateRange,
    ) -> RepositoryResult<Vec<Trade>>;

    /// Check if trades are available for a specific date
    async fn has_trades(&self, ticker: &FuturesTicker, date: NaiveDate) -> RepositoryResult<bool>;

    /// Get trades for a specific date
    async fn get_trades_for_date(
        &self,
        ticker: &FuturesTicker,
        date: NaiveDate,
    ) -> RepositoryResult<Vec<Trade>>;

    /// Store trades for a specific date
    async fn store_trades(
        &self,
        ticker: &FuturesTicker,
        date: NaiveDate,
        trades: Vec<Trade>,
    ) -> RepositoryResult<()>;

    /// Find missing dates (gaps) in the cache
    async fn find_gaps(
        &self,
        ticker: &FuturesTicker,
        date_range: &DateRange,
    ) -> RepositoryResult<Vec<DateRange>>;

    /// Get repository statistics
    async fn stats(&self) -> RepositoryResult<RepositoryStats>;

    /// Check which days are cached vs need download (Databento-specific)
    ///
    /// Note: This method accepts Databento Schema type for exchange-specific operations.
    /// The data layer can call this via dynamic dispatch without knowing Schema details.
    async fn check_cache_coverage_databento(
        &self,
        ticker: &FuturesTicker,
        schema_discriminant: u16, // Databento Schema as u16
        date_range: &DateRange,
    ) -> RepositoryResult<CacheCoverageReport> {
        // Default implementation returns error
        Err(RepositoryError::InvalidData(
            "Cache coverage not supported by this repository".to_string(),
        ))
    }

    /// Prefetch data to cache without loading (Databento-specific)
    ///
    /// Note: This method accepts Databento Schema type for exchange-specific operations.
    async fn prefetch_to_cache_databento(
        &self,
        ticker: &FuturesTicker,
        schema_discriminant: u16, // Databento Schema as u16
        date_range: &DateRange,
    ) -> RepositoryResult<usize> {
        // Default implementation returns error
        Err(RepositoryError::InvalidData(
            "Prefetch not supported by this repository".to_string(),
        ))
    }

    /// Get actual cost from Databento API (Databento-specific)
    ///
    /// Calls Databento's metadata.get_cost() API to get real USD cost.
    /// Must be overridden by Databento repository implementations.
    async fn get_actual_cost_databento(
        &self,
        ticker: &FuturesTicker,
        schema_discriminant: u16,
        date_range: &DateRange,
    ) -> RepositoryResult<f64>;

    /// List symbols with cached data (Databento-specific)
    ///
    /// Returns set of symbol strings that have at least one cached file.
    /// Default implementation returns empty set.
    async fn list_cached_symbols_databento(&self) -> RepositoryResult<std::collections::HashSet<String>> {
        Ok(std::collections::HashSet::new())
    }
}

/// Depth (orderbook) repository interface
///
/// Provides access to MBP-10 depth snapshots for heatmap visualization.
#[async_trait::async_trait]
pub trait DepthRepository: Send + Sync {
    /// Get depth snapshots for a ticker in a date range
    async fn get_depth(
        &self,
        ticker: &FuturesTicker,
        date_range: &DateRange,
    ) -> RepositoryResult<Vec<DepthSnapshot>>;

    /// Check if depth data is available for a specific date
    async fn has_depth(&self, ticker: &FuturesTicker, date: NaiveDate) -> RepositoryResult<bool>;

    /// Get depth snapshots for a specific date
    async fn get_depth_for_date(
        &self,
        ticker: &FuturesTicker,
        date: NaiveDate,
    ) -> RepositoryResult<Vec<DepthSnapshot>>;

    /// Store depth snapshots for a specific date
    async fn store_depth(
        &self,
        ticker: &FuturesTicker,
        date: NaiveDate,
        depth: Vec<DepthSnapshot>,
    ) -> RepositoryResult<()>;

    /// Find missing dates (gaps) in the cache
    async fn find_gaps(
        &self,
        ticker: &FuturesTicker,
        date_range: &DateRange,
    ) -> RepositoryResult<Vec<DateRange>>;

    /// Get repository statistics
    async fn stats(&self) -> RepositoryResult<RepositoryStats>;
}

/// Repository statistics
#[derive(Debug, Clone)]
pub struct RepositoryStats {
    /// Total number of cached days
    pub cached_days: usize,
    /// Total size in bytes
    pub total_size: u64,
    /// Cache hit rate (0.0 to 1.0)
    pub hit_rate: f64,
    /// Number of cache hits
    pub hits: u64,
    /// Number of cache misses
    pub misses: u64,
}

impl RepositoryStats {
    pub fn new() -> Self {
        Self {
            cached_days: 0,
            total_size: 0,
            hit_rate: 0.0,
            hits: 0,
            misses: 0,
        }
    }

    pub fn record_hit(&mut self) {
        self.hits += 1;
        self.update_hit_rate();
    }

    pub fn record_miss(&mut self) {
        self.misses += 1;
        self.update_hit_rate();
    }

    fn update_hit_rate(&mut self) {
        let total = self.hits + self.misses;
        if total > 0 {
            self.hit_rate = self.hits as f64 / total as f64;
        }
    }

    pub fn size_human_readable(&self) -> String {
        const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
        let mut size = self.total_size as f64;
        let mut unit_idx = 0;

        while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
            size /= 1024.0;
            unit_idx += 1;
        }

        format!("{:.2} {}", size, UNITS[unit_idx])
    }
}

impl fmt::Display for RepositoryStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Cached Days: {}, Size: {}, Hit Rate: {:.1}% ({}/{})",
            self.cached_days,
            self.size_human_readable(),
            self.hit_rate * 100.0,
            self.hits,
            self.hits + self.misses
        )
    }
}

impl Default for RepositoryStats {
    fn default() -> Self {
        Self::new()
    }
}

/// Option snapshot repository interface
///
/// Provides access to option snapshots with Greeks and IV data.
#[async_trait::async_trait]
pub trait OptionSnapshotRepository: Send + Sync {
    /// Get option snapshots for an underlying asset in a date range
    ///
    /// Returns snapshots for all available contracts for the underlying.
    async fn get_snapshots(
        &self,
        underlying_ticker: &str,
        date_range: &DateRange,
    ) -> RepositoryResult<Vec<OptionSnapshot>>;

    /// Get a snapshot for a specific option contract at a date
    async fn get_snapshot(
        &self,
        contract_ticker: &str,
        date: NaiveDate,
    ) -> RepositoryResult<OptionSnapshot>;

    /// Get snapshots for multiple contract tickers at a date
    async fn get_snapshots_for_contracts(
        &self,
        contract_tickers: &[String],
        date: NaiveDate,
    ) -> RepositoryResult<Vec<OptionSnapshot>>;

    /// Check if snapshots are available for a date
    async fn has_snapshots(
        &self,
        underlying_ticker: &str,
        date: NaiveDate,
    ) -> RepositoryResult<bool>;

    /// Store snapshots for a specific date
    async fn store_snapshots(
        &self,
        underlying_ticker: &str,
        date: NaiveDate,
        snapshots: Vec<OptionSnapshot>,
    ) -> RepositoryResult<()>;

    /// Find missing dates (gaps) in the cache
    async fn find_gaps(
        &self,
        underlying_ticker: &str,
        date_range: &DateRange,
    ) -> RepositoryResult<Vec<DateRange>>;

    /// Get repository statistics
    async fn stats(&self) -> RepositoryResult<RepositoryStats>;
}

/// Option chain repository interface
///
/// Provides access to complete option chains for underlying assets.
#[async_trait::async_trait]
pub trait OptionChainRepository: Send + Sync {
    /// Get option chain for an underlying asset at a specific date
    ///
    /// Returns the complete chain with all strikes and expirations.
    async fn get_chain(
        &self,
        underlying_ticker: &str,
        date: NaiveDate,
    ) -> RepositoryResult<OptionChain>;

    /// Get option chains for a date range
    ///
    /// Returns one chain per date in the range.
    async fn get_chains(
        &self,
        underlying_ticker: &str,
        date_range: &DateRange,
    ) -> RepositoryResult<Vec<OptionChain>>;

    /// Get chain filtered by strike price range
    async fn get_chain_by_strike_range(
        &self,
        underlying_ticker: &str,
        date: NaiveDate,
        min_strike: f64,
        max_strike: f64,
    ) -> RepositoryResult<OptionChain>;

    /// Get chain filtered by expiration date
    async fn get_chain_by_expiration(
        &self,
        underlying_ticker: &str,
        date: NaiveDate,
        expiration: NaiveDate,
    ) -> RepositoryResult<OptionChain>;

    /// Check if chain data is available for a date
    async fn has_chain(&self, underlying_ticker: &str, date: NaiveDate)
        -> RepositoryResult<bool>;

    /// Store option chain for a specific date
    async fn store_chain(
        &self,
        underlying_ticker: &str,
        date: NaiveDate,
        chain: OptionChain,
    ) -> RepositoryResult<()>;

    /// Find missing dates (gaps) in the cache
    async fn find_gaps(
        &self,
        underlying_ticker: &str,
        date_range: &DateRange,
    ) -> RepositoryResult<Vec<DateRange>>;

    /// Get repository statistics
    async fn stats(&self) -> RepositoryResult<RepositoryStats>;
}

/// Option contract repository interface
///
/// Provides access to option contract metadata and specifications.
#[async_trait::async_trait]
pub trait OptionContractRepository: Send + Sync {
    /// Get all available contracts for an underlying asset
    ///
    /// Returns both active and expired contracts by default.
    async fn get_contracts(
        &self,
        underlying_ticker: &str,
    ) -> RepositoryResult<Vec<OptionContract>>;

    /// Get active contracts for an underlying asset
    ///
    /// Returns only contracts that haven't expired as of the given date.
    async fn get_active_contracts(
        &self,
        underlying_ticker: &str,
        as_of: NaiveDate,
    ) -> RepositoryResult<Vec<OptionContract>>;

    /// Get contract details by ticker symbol
    async fn get_contract(&self, contract_ticker: &str) -> RepositoryResult<OptionContract>;

    /// Search contracts by criteria
    async fn search_contracts(
        &self,
        underlying_ticker: Option<&str>,
        expiration: Option<NaiveDate>,
        min_strike: Option<f64>,
        max_strike: Option<f64>,
        include_expired: bool,
    ) -> RepositoryResult<Vec<OptionContract>>;

    /// Get contracts expiring on a specific date
    async fn get_contracts_by_expiration(
        &self,
        underlying_ticker: &str,
        expiration: NaiveDate,
    ) -> RepositoryResult<Vec<OptionContract>>;

    /// Check if contract metadata is cached
    async fn has_contract(&self, contract_ticker: &str) -> RepositoryResult<bool>;

    /// Store contract metadata
    async fn store_contract(&self, contract: OptionContract) -> RepositoryResult<()>;

    /// Store multiple contracts
    async fn store_contracts(&self, contracts: Vec<OptionContract>) -> RepositoryResult<()>;

    /// Get repository statistics
    async fn stats(&self) -> RepositoryResult<RepositoryStats>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_repository_stats() {
        let mut stats = RepositoryStats::new();
        assert_eq!(stats.hit_rate, 0.0);

        stats.record_hit();
        stats.record_hit();
        stats.record_miss();

        assert_eq!(stats.hits, 2);
        assert_eq!(stats.misses, 1);
        assert!((stats.hit_rate - 0.6666).abs() < 0.001);
    }

    #[test]
    fn test_size_human_readable() {
        let mut stats = RepositoryStats::new();

        stats.total_size = 500;
        assert!(stats.size_human_readable().contains("B"));

        stats.total_size = 1500;
        assert!(stats.size_human_readable().contains("KB"));

        stats.total_size = 1_500_000;
        assert!(stats.size_human_readable().contains("MB"));
    }
}
