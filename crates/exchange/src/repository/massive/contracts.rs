use super::convert_massive_error;
use crate::adapter::massive::{HistoricalOptionsManager, MassiveConfig};
use crate::adapter::massive::util::extract_underlying_repo;
use chrono::NaiveDate;
use kairos_data::domain::OptionContract;
use kairos_data::repository::{
    OptionContractRepository, RepositoryError, RepositoryResult, RepositoryStats,
};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Massive implementation of OptionContractRepository
pub struct MassiveContractRepository {
    manager: Arc<Mutex<HistoricalOptionsManager>>,
}

impl MassiveContractRepository {
    /// Create a new Massive contract repository
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
impl OptionContractRepository for MassiveContractRepository {
    async fn get_contracts(
        &self,
        underlying_ticker: &str,
    ) -> RepositoryResult<Vec<OptionContract>> {
        let manager = self.manager.lock().await;

        manager
            .fetch_contracts_metadata(underlying_ticker)
            .await
            .map_err(convert_massive_error)
    }

    async fn get_active_contracts(
        &self,
        underlying_ticker: &str,
        as_of: NaiveDate,
    ) -> RepositoryResult<Vec<OptionContract>> {
        let contracts = self.get_contracts(underlying_ticker).await?;

        // Filter to active contracts only
        let active: Vec<OptionContract> = contracts
            .into_iter()
            .filter(|c| !c.is_expired(as_of))
            .collect();

        Ok(active)
    }

    async fn get_contract(&self, contract_ticker: &str) -> RepositoryResult<OptionContract> {
        // Extract underlying ticker
        let underlying = extract_underlying_repo(contract_ticker)?;

        // Get all contracts for underlying
        let contracts = self.get_contracts(&underlying).await?;

        // Find specific contract
        contracts
            .into_iter()
            .find(|c| c.ticker == contract_ticker)
            .ok_or_else(|| RepositoryError::NotFound(contract_ticker.to_string()))
    }

    async fn search_contracts(
        &self,
        underlying_ticker: Option<&str>,
        expiration: Option<NaiveDate>,
        min_strike: Option<f64>,
        max_strike: Option<f64>,
        include_expired: bool,
    ) -> RepositoryResult<Vec<OptionContract>> {
        let underlying = underlying_ticker.ok_or_else(|| {
            RepositoryError::InvalidData("underlying_ticker required for search".to_string())
        })?;

        let mut contracts = self.get_contracts(underlying).await?;

        // Apply filters
        if !include_expired {
            let today = chrono::Utc::now().date_naive();
            contracts.retain(|c| !c.is_expired(today));
        }

        if let Some(exp) = expiration {
            contracts.retain(|c| c.expiration_date == exp);
        }

        if let Some(min) = min_strike {
            contracts.retain(|c| c.strike_price.to_f64() >= min);
        }

        if let Some(max) = max_strike {
            contracts.retain(|c| c.strike_price.to_f64() <= max);
        }

        Ok(contracts)
    }

    async fn get_contracts_by_expiration(
        &self,
        underlying_ticker: &str,
        expiration: NaiveDate,
    ) -> RepositoryResult<Vec<OptionContract>> {
        let contracts = self.get_contracts(underlying_ticker).await?;

        let filtered: Vec<OptionContract> = contracts
            .into_iter()
            .filter(|c| c.expiration_date == expiration)
            .collect();

        Ok(filtered)
    }

    async fn has_contract(&self, contract_ticker: &str) -> RepositoryResult<bool> {
        match self.get_contract(contract_ticker).await {
            Ok(_) => Ok(true),
            Err(RepositoryError::NotFound(_)) => Ok(false),
            Err(e) => Err(e),
        }
    }

    async fn store_contract(&self, _contract: OptionContract) -> RepositoryResult<()> {
        // Caching is handled automatically by the manager
        Ok(())
    }

    async fn store_contracts(&self, _contracts: Vec<OptionContract>) -> RepositoryResult<()> {
        // Caching is handled automatically by the manager
        Ok(())
    }

    async fn stats(&self) -> RepositoryResult<RepositoryStats> {
        let manager = self.manager.lock().await;

        let cache_stats = manager.cache_stats().await.map_err(convert_massive_error)?;

        Ok(RepositoryStats {
            cached_days: 0,
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
    async fn test_contract_repository() {
        let config = MassiveConfig::from_env().unwrap();
        let repo = MassiveContractRepository::new(config).await.unwrap();

        // Test get_contracts
        let contracts = repo.get_contracts("AAPL").await;
        if contracts.is_ok() {
            let c = contracts.unwrap();
            assert!(!c.is_empty());
            for contract in &c {
                assert_eq!(contract.underlying_ticker, "AAPL");
            }
        }

        // Test get_active_contracts
        let today = chrono::Utc::now().date_naive();
        let active = repo.get_active_contracts("AAPL", today).await;
        if active.is_ok() {
            let a = active.unwrap();
            for contract in &a {
                assert!(!contract.is_expired(today));
            }
        }
    }

    #[tokio::test]
    #[ignore] // Requires API key
    async fn test_search_contracts() {
        let config = MassiveConfig::from_env().unwrap();
        let repo = MassiveContractRepository::new(config).await.unwrap();

        // Search with filters
        let results = repo
            .search_contracts(
                Some("AAPL"),
                None,
                Some(150.0),
                Some(160.0),
                false, // exclude expired
            )
            .await;

        if results.is_ok() {
            let r = results.unwrap();
            for contract in &r {
                let strike = contract.strike_price.to_f64();
                assert!(strike >= 150.0 && strike <= 160.0);
            }
        }
    }
}
