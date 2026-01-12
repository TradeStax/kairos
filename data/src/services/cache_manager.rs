//! Cache Manager Service
//!
//! Manages cache lifecycle, cleanup, and statistics.

use crate::domain::FuturesTicker;
use crate::repository::traits::{
    DepthRepository, RepositoryError, RepositoryStats, TradeRepository,
};
use chrono::NaiveDate;
use std::sync::Arc;

/// Cache manager service error
#[derive(thiserror::Error, Debug)]
pub enum CacheManagerError {
    #[error("Repository error: {0}")]
    Repository(#[from] RepositoryError),

    #[error("Invalid operation: {0}")]
    InvalidOperation(String),
}

pub type CacheManagerResult<T> = Result<T, CacheManagerError>;

/// Cache manager service
///
/// Provides operations for managing cached data:
/// - Get cache statistics
/// - Clean up old data
/// - Verify cache integrity
/// - Preload frequently used data
pub struct CacheManagerService {
    trade_repo: Arc<dyn TradeRepository>,
    depth_repo: Arc<dyn DepthRepository>,
}

impl CacheManagerService {
    /// Create a new cache manager service
    pub fn new(trade_repo: Arc<dyn TradeRepository>, depth_repo: Arc<dyn DepthRepository>) -> Self {
        Self {
            trade_repo,
            depth_repo,
        }
    }

    /// Get cache statistics for trades
    pub async fn get_trade_stats(&self) -> CacheManagerResult<RepositoryStats> {
        self.trade_repo
            .stats()
            .await
            .map_err(CacheManagerError::from)
    }

    /// Get cache statistics for depth data
    pub async fn get_depth_stats(&self) -> CacheManagerResult<RepositoryStats> {
        self.depth_repo
            .stats()
            .await
            .map_err(CacheManagerError::from)
    }

    /// Get combined cache statistics
    pub async fn get_combined_stats(&self) -> CacheManagerResult<CombinedStats> {
        let trade_stats = self.get_trade_stats().await?;
        let depth_stats = self.get_depth_stats().await?;

        Ok(CombinedStats {
            trade_stats,
            depth_stats,
        })
    }

    /// Check if data is cached for a specific ticker and date
    pub async fn is_cached(
        &self,
        ticker: &FuturesTicker,
        date: NaiveDate,
    ) -> CacheManagerResult<CacheStatus> {
        let has_trades = self.trade_repo.has_trades(ticker, date).await?;
        let has_depth = self.depth_repo.has_depth(ticker, date).await?;

        Ok(CacheStatus {
            has_trades,
            has_depth,
            date,
        })
    }

    /// Get human-readable cache summary
    pub async fn get_summary(&self) -> CacheManagerResult<String> {
        let stats = self.get_combined_stats().await?;

        Ok(format!(
            "Cache Summary:\n\
            Trades: {}\n\
            Depth: {}",
            stats.trade_stats, stats.depth_stats
        ))
    }
}

/// Combined cache statistics
#[derive(Debug, Clone)]
pub struct CombinedStats {
    pub trade_stats: RepositoryStats,
    pub depth_stats: RepositoryStats,
}

impl CombinedStats {
    /// Total cached days across all data types
    pub fn total_cached_days(&self) -> usize {
        self.trade_stats.cached_days + self.depth_stats.cached_days
    }

    /// Total cache size
    pub fn total_size(&self) -> u64 {
        self.trade_stats.total_size + self.depth_stats.total_size
    }

    /// Average hit rate across both repositories
    pub fn average_hit_rate(&self) -> f64 {
        (self.trade_stats.hit_rate + self.depth_stats.hit_rate) / 2.0
    }

    /// Human-readable total size
    pub fn total_size_human_readable(&self) -> String {
        const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
        let mut size = self.total_size() as f64;
        let mut unit_idx = 0;

        while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
            size /= 1024.0;
            unit_idx += 1;
        }

        format!("{:.2} {}", size, UNITS[unit_idx])
    }
}

/// Cache status for a specific date
#[derive(Debug, Clone)]
pub struct CacheStatus {
    pub has_trades: bool,
    pub has_depth: bool,
    pub date: NaiveDate,
}

impl CacheStatus {
    /// Check if any data is cached for this date
    pub fn has_any_data(&self) -> bool {
        self.has_trades || self.has_depth
    }

    /// Check if all data is cached for this date
    pub fn has_all_data(&self) -> bool {
        self.has_trades && self.has_depth
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_combined_stats() {
        let mut trade_stats = RepositoryStats::new();
        trade_stats.cached_days = 10;
        trade_stats.total_size = 1_000_000;
        trade_stats.record_hit();
        trade_stats.record_hit();

        let mut depth_stats = RepositoryStats::new();
        depth_stats.cached_days = 5;
        depth_stats.total_size = 500_000;
        depth_stats.record_hit();

        let combined = CombinedStats {
            trade_stats,
            depth_stats,
        };

        assert_eq!(combined.total_cached_days(), 15);
        assert_eq!(combined.total_size(), 1_500_000);
        assert!(combined.average_hit_rate() > 0.0);
    }

    #[test]
    fn test_cache_status() {
        let status = CacheStatus {
            has_trades: true,
            has_depth: false,
            date: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
        };

        assert!(status.has_any_data());
        assert!(!status.has_all_data());
    }
}
