use crate::adapter::massive::{HistoricalOptionsManager, MassiveConfig, MassiveError};
use chrono::NaiveDate;
use flowsurface_data::domain::{DateRange, OptionSnapshot};
use flowsurface_data::repository::{
    OptionSnapshotRepository, RepositoryError, RepositoryResult, RepositoryStats,
};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Massive implementation of OptionSnapshotRepository
pub struct MassiveSnapshotRepository {
    manager: Arc<Mutex<HistoricalOptionsManager>>,
}

impl MassiveSnapshotRepository {
    /// Create a new Massive snapshot repository
    pub async fn new(config: MassiveConfig) -> RepositoryResult<Self> {
        let manager = HistoricalOptionsManager::new(config)
            .await
            .map_err(convert_error)?;

        Ok(Self {
            manager: Arc::new(Mutex::new(manager)),
        })
    }
}

#[async_trait::async_trait]
impl OptionSnapshotRepository for MassiveSnapshotRepository {
    async fn get_snapshots(
        &self,
        underlying_ticker: &str,
        date_range: &DateRange,
    ) -> RepositoryResult<Vec<OptionSnapshot>> {
        let manager = self.manager.lock().await;

        // Fetch chains for the date range
        let chains = manager
            .fetch_option_chains(underlying_ticker, date_range)
            .await
            .map_err(convert_error)?;

        // Extract all snapshots from chains
        let mut snapshots = Vec::new();
        for chain in chains {
            snapshots.extend(chain.contracts);
        }

        Ok(snapshots)
    }

    async fn get_snapshot(
        &self,
        contract_ticker: &str,
        date: NaiveDate,
    ) -> RepositoryResult<OptionSnapshot> {
        let manager = self.manager.lock().await;

        manager
            .fetch_contract_snapshot(contract_ticker, date)
            .await
            .map_err(convert_error)
    }

    async fn get_snapshots_for_contracts(
        &self,
        contract_tickers: &[String],
        date: NaiveDate,
    ) -> RepositoryResult<Vec<OptionSnapshot>> {
        let manager = self.manager.lock().await;

        let mut snapshots = Vec::new();

        for ticker in contract_tickers {
            match manager.fetch_contract_snapshot(ticker, date).await {
                Ok(snapshot) => snapshots.push(snapshot),
                Err(e) => {
                    log::warn!("Failed to fetch snapshot for {}: {}", ticker, e);
                    // Continue with other contracts
                }
            }
        }

        Ok(snapshots)
    }

    async fn has_snapshots(
        &self,
        underlying_ticker: &str,
        date: NaiveDate,
    ) -> RepositoryResult<bool> {
        let manager = self.manager.lock().await;

        // Check if chain is cached for this date
        let cache_path = manager
            .cache
            .get_cache_path("chains", underlying_ticker, Some(date));

        let exists: bool = tokio::fs::metadata(&cache_path).await.is_ok();
        Ok(exists)
    }

    async fn store_snapshots(
        &self,
        _underlying_ticker: &str,
        _date: NaiveDate,
        _snapshots: Vec<OptionSnapshot>,
    ) -> RepositoryResult<()> {
        // Caching is handled automatically by the manager
        Ok(())
    }

    async fn find_gaps(
        &self,
        underlying_ticker: &str,
        date_range: &DateRange,
    ) -> RepositoryResult<Vec<DateRange>> {
        let manager = self.manager.lock().await;

        let gaps: Vec<DateRange> = manager
            .find_chain_gaps(underlying_ticker, date_range)
            .await
            .map_err(convert_error)?;

        Ok(gaps)
    }

    async fn stats(&self) -> RepositoryResult<RepositoryStats> {
        let manager = self.manager.lock().await;

        let cache_stats = manager.cache_stats().await.map_err(convert_error)?;

        Ok(RepositoryStats {
            cached_days: 0, // Not tracked per-day for snapshots
            total_size: cache_stats.total_size_bytes,
            hit_rate: 0.0, // Would need to track hits/misses
            hits: 0,
            misses: 0,
        })
    }
}

/// Convert Massive error to Repository error
fn convert_error(e: MassiveError) -> RepositoryError {
    match e {
        MassiveError::SymbolNotFound(s) => RepositoryError::NotFound(s),
        MassiveError::Cache(s) => RepositoryError::Cache(s),
        MassiveError::Parse(s) => RepositoryError::Serialization(s),
        MassiveError::InvalidData(s) => RepositoryError::InvalidData(s),
        MassiveError::Io(e) => RepositoryError::Io(e),
        _ => RepositoryError::Remote(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires API key
    async fn test_snapshot_repository() {
        let config = MassiveConfig::from_env().unwrap();
        let repo = MassiveSnapshotRepository::new(config).await.unwrap();

        let date = NaiveDate::from_ymd_opt(2024, 1, 19).unwrap();

        // Test has_snapshots
        let has = repo.has_snapshots("AAPL", date).await;
        assert!(has.is_ok());

        // Test get_snapshot
        let snapshot = repo.get_snapshot("O:AAPL240119C00150000", date).await;
        if snapshot.is_ok() {
            let s = snapshot.unwrap();
            assert_eq!(s.contract.underlying_ticker, "AAPL");
        }
    }
}
