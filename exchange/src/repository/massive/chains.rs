use super::convert_massive_error;
use crate::adapter::massive::{HistoricalOptionsManager, MassiveConfig};
use chrono::NaiveDate;
use flowsurface_data::domain::{DateRange, OptionChain};
use flowsurface_data::repository::{OptionChainRepository, RepositoryResult, RepositoryStats};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Massive implementation of OptionChainRepository
pub struct MassiveChainRepository {
    manager: Arc<Mutex<HistoricalOptionsManager>>,
}

impl MassiveChainRepository {
    /// Create a new Massive chain repository
    pub async fn new(config: MassiveConfig) -> RepositoryResult<Self> {
        let manager = HistoricalOptionsManager::new(config)
            .await
            .map_err(convert_massive_error)?;

        Ok(Self {
            manager: Arc::new(Mutex::new(manager)),
        })
    }
}

#[async_trait::async_trait]
impl OptionChainRepository for MassiveChainRepository {
    async fn get_chain(
        &self,
        underlying_ticker: &str,
        date: NaiveDate,
    ) -> RepositoryResult<OptionChain> {
        let manager = self.manager.lock().await;

        manager
            .fetch_option_chain(underlying_ticker, date)
            .await
            .map_err(convert_massive_error)
    }

    async fn get_chains(
        &self,
        underlying_ticker: &str,
        date_range: &DateRange,
    ) -> RepositoryResult<Vec<OptionChain>> {
        let manager = self.manager.lock().await;

        manager
            .fetch_option_chains(underlying_ticker, date_range)
            .await
            .map_err(convert_massive_error)
    }

    async fn get_chain_by_strike_range(
        &self,
        underlying_ticker: &str,
        date: NaiveDate,
        min_strike: f64,
        max_strike: f64,
    ) -> RepositoryResult<OptionChain> {
        // Fetch full chain and filter
        let mut chain = self.get_chain(underlying_ticker, date).await?;

        // Filter contracts by strike range
        chain.contracts.retain(|snapshot| {
            let strike = snapshot.contract.strike_price.to_f64();
            strike >= min_strike && strike <= max_strike
        });

        Ok(chain)
    }

    async fn get_chain_by_expiration(
        &self,
        underlying_ticker: &str,
        date: NaiveDate,
        expiration: NaiveDate,
    ) -> RepositoryResult<OptionChain> {
        // Fetch full chain and filter
        let mut chain = self.get_chain(underlying_ticker, date).await?;

        // Filter contracts by expiration date
        chain
            .contracts
            .retain(|snapshot| snapshot.contract.expiration_date == expiration);

        Ok(chain)
    }

    async fn has_chain(&self, underlying_ticker: &str, date: NaiveDate) -> RepositoryResult<bool> {
        let manager = self.manager.lock().await;

        let cache_path = manager
            .cache
            .get_cache_path("chains", underlying_ticker, Some(date));

        let exists: bool = tokio::fs::metadata(&cache_path).await.is_ok();
        Ok(exists)
    }

    async fn store_chain(
        &self,
        _underlying_ticker: &str,
        _date: NaiveDate,
        _chain: OptionChain,
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

        manager
            .find_chain_gaps(underlying_ticker, date_range)
            .await
            .map_err(convert_massive_error)
    }

    async fn stats(&self) -> RepositoryResult<RepositoryStats> {
        let manager = self.manager.lock().await;

        let cache_stats = manager.cache_stats().await.map_err(convert_massive_error)?;

        Ok(RepositoryStats {
            cached_days: 0, // Would need to count cache files
            total_size: cache_stats.total_size_bytes,
            hit_rate: 0.0,
            hits: 0,
            misses: 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires API key
    async fn test_chain_repository() {
        let config = MassiveConfig::from_env().unwrap();
        let repo = MassiveChainRepository::new(config).await.unwrap();

        let date = NaiveDate::from_ymd_opt(2024, 1, 19).unwrap();

        // Test get_chain
        let chain = repo.get_chain("AAPL", date).await;
        if chain.is_ok() {
            let c = chain.unwrap();
            assert_eq!(c.underlying_ticker, "AAPL");
            assert!(!c.is_empty());
        }

        // Test has_chain
        let has = repo.has_chain("AAPL", date).await;
        assert!(has.is_ok());
    }

    #[tokio::test]
    #[ignore] // Requires API key
    async fn test_chain_filtering() {
        let config = MassiveConfig::from_env().unwrap();
        let repo = MassiveChainRepository::new(config).await.unwrap();

        let date = NaiveDate::from_ymd_opt(2024, 1, 19).unwrap();
        let expiration = NaiveDate::from_ymd_opt(2024, 3, 15).unwrap();

        // Test filter by expiration
        let chain = repo.get_chain_by_expiration("AAPL", date, expiration).await;
        if chain.is_ok() {
            let c = chain.unwrap();
            for snapshot in &c.contracts {
                assert_eq!(snapshot.contract.expiration_date, expiration);
            }
        }

        // Test filter by strike range
        let chain = repo
            .get_chain_by_strike_range("AAPL", date, 150.0, 160.0)
            .await;
        if chain.is_ok() {
            let c = chain.unwrap();
            for snapshot in &c.contracts {
                let strike = snapshot.contract.strike_price.to_f64();
                assert!(strike >= 150.0 && strike <= 160.0);
            }
        }
    }
}
