use crate::domain::{DateRange, GexProfile, OptionChain, OptionContract, OptionSnapshot};
use crate::domain::LoadingStatus;
use crate::repository::{
    OptionChainRepository, OptionContractRepository, OptionSnapshotRepository, RepositoryError,
    RepositoryResult,
};
use chrono::NaiveDate;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Options data service
///
/// Orchestrates fetching and managing options data from repositories.
/// Provides a clean API for the application layer.
pub struct OptionsDataService {
    snapshot_repo: Arc<dyn OptionSnapshotRepository>,
    chain_repo: Arc<dyn OptionChainRepository>,
    contract_repo: Arc<dyn OptionContractRepository>,
    loading_status: Arc<Mutex<HashMap<String, LoadingStatus>>>,
}

impl OptionsDataService {
    /// Create a new options data service
    pub fn new(
        snapshot_repo: Arc<dyn OptionSnapshotRepository>,
        chain_repo: Arc<dyn OptionChainRepository>,
        contract_repo: Arc<dyn OptionContractRepository>,
    ) -> Self {
        Self {
            snapshot_repo,
            chain_repo,
            contract_repo,
            loading_status: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Get option chain with Greeks and IV data for a specific date
    ///
    /// Returns the complete option chain including all strikes and expirations
    /// with Greeks, implied volatility, and open interest data.
    ///
    /// # Performance
    /// - First load: 2-5s (API bound)
    /// - Cached load: <100ms (local disk)
    pub async fn get_chain_with_greeks(
        &self,
        underlying_ticker: &str,
        date: NaiveDate,
    ) -> RepositoryResult<OptionChain> {
        let key = format!("chain_{}_{}",underlying_ticker, date);
        self.set_loading_status(&key, LoadingStatus::LoadingFromCache {
            schema: crate::domain::DataSchema::Options,
            days_total: 1,
            days_loaded: 0,
            items_loaded: 0,
        })
        .await;

        // Fetch chain from repository
        let result = self.chain_repo.get_chain(underlying_ticker, date).await;

        match &result {
            Ok(chain) => {
                self.set_loading_status(&key, LoadingStatus::Ready).await;
                log::info!(
                    "Loaded option chain for {} with {} contracts",
                    underlying_ticker,
                    chain.contract_count()
                );
            }
            Err(e) => {
                self.set_loading_status(
                    &key,
                    LoadingStatus::Error {
                        message: format!("Failed to load chain: {}", e),
                    },
                )
                .await;
            }
        }

        result
    }

    /// Get option chains for a date range
    ///
    /// Returns one chain per date in the range. Uses smart gap detection
    /// to minimize API calls.
    pub async fn get_chains_for_range(
        &self,
        underlying_ticker: &str,
        date_range: &DateRange,
    ) -> RepositoryResult<Vec<OptionChain>> {
        let key = format!(
            "chains_{}_{:?}",
            underlying_ticker,
            date_range
        );

        self.set_loading_status(&key, LoadingStatus::Downloading {
            schema: crate::domain::DataSchema::Options,
            days_total: date_range.num_days() as usize,
            days_complete: 0,
            current_day: String::new(),
        })
        .await;

        let result = self
            .chain_repo
            .get_chains(underlying_ticker, date_range)
            .await;

        match &result {
            Ok(chains) => {
                self.set_loading_status(&key, LoadingStatus::Ready).await;
                log::info!(
                    "Loaded {} option chains for {}",
                    chains.len(),
                    underlying_ticker
                );
            }
            Err(e) => {
                self.set_loading_status(
                    &key,
                    LoadingStatus::Error {
                        message: format!("Failed to load chains: {}", e),
                    },
                )
                .await;
            }
        }

        result
    }

    /// Get a specific option contract snapshot
    pub async fn get_snapshot(
        &self,
        contract_ticker: &str,
        date: NaiveDate,
    ) -> RepositoryResult<OptionSnapshot> {
        self.snapshot_repo.get_snapshot(contract_ticker, date).await
    }

    /// Get snapshots for multiple contracts
    pub async fn get_snapshots_for_contracts(
        &self,
        contract_tickers: &[String],
        date: NaiveDate,
    ) -> RepositoryResult<Vec<OptionSnapshot>> {
        self.snapshot_repo
            .get_snapshots_for_contracts(contract_tickers, date)
            .await
    }

    /// Get all available contracts for an underlying
    pub async fn get_available_contracts(
        &self,
        underlying_ticker: &str,
    ) -> RepositoryResult<Vec<OptionContract>> {
        self.contract_repo.get_contracts(underlying_ticker).await
    }

    /// Get active contracts (not expired)
    pub async fn get_active_contracts(
        &self,
        underlying_ticker: &str,
        as_of: NaiveDate,
    ) -> RepositoryResult<Vec<OptionContract>> {
        self.contract_repo
            .get_active_contracts(underlying_ticker, as_of)
            .await
    }

    /// Get contracts expiring on a specific date
    pub async fn get_contracts_by_expiration(
        &self,
        underlying_ticker: &str,
        expiration: NaiveDate,
    ) -> RepositoryResult<Vec<OptionContract>> {
        self.contract_repo
            .get_contracts_by_expiration(underlying_ticker, expiration)
            .await
    }

    /// Search for contracts matching criteria
    pub async fn search_contracts(
        &self,
        underlying_ticker: Option<&str>,
        expiration: Option<NaiveDate>,
        min_strike: Option<f64>,
        max_strike: Option<f64>,
        include_expired: bool,
    ) -> RepositoryResult<Vec<OptionContract>> {
        self.contract_repo
            .search_contracts(underlying_ticker, expiration, min_strike, max_strike, include_expired)
            .await
    }

    /// Get historical implied volatility for a contract
    ///
    /// Returns snapshots over time showing IV evolution.
    pub async fn get_historical_iv(
        &self,
        underlying_ticker: &str,
        date_range: &DateRange,
    ) -> RepositoryResult<Vec<OptionSnapshot>> {
        let snapshots = self
            .snapshot_repo
            .get_snapshots(underlying_ticker, date_range)
            .await?;

        // Filter to only snapshots with IV data
        let with_iv: Vec<OptionSnapshot> = snapshots
            .into_iter()
            .filter(|s| s.implied_volatility.is_some())
            .collect();

        Ok(with_iv)
    }

    /// Get GEX (gamma exposure) profile for an underlying at a date
    ///
    /// Calculates gamma exposure from the option chain, identifying
    /// key support/resistance levels.
    ///
    /// # Requirements
    /// - Option chain must have underlying_price set
    /// - Contracts must have greeks.gamma and open_interest
    pub async fn get_gex_profile(
        &self,
        underlying_ticker: &str,
        date: NaiveDate,
    ) -> RepositoryResult<GexProfile> {
        // Fetch the complete chain
        let chain = self.get_chain_with_greeks(underlying_ticker, date).await?;

        // Validate underlying price is present
        if chain.underlying_price.is_none() {
            return Err(RepositoryError::InvalidData(
                "Underlying price is required for GEX calculation but not available".to_string(),
            ));
        }

        // Calculate GEX profile from chain
        let profile = GexProfile::from_option_chain(&chain).ok_or_else(|| {
            RepositoryError::InvalidData(
                "Failed to calculate GEX profile - underlying price required".to_string(),
            )
        })?;

        if !profile.has_data() {
            return Err(RepositoryError::InvalidData(
                "No gamma data available in chain".to_string(),
            ));
        }

        log::info!(
            "Calculated GEX profile for {} with {} exposure levels",
            underlying_ticker,
            profile.exposure_count()
        );

        if let Some(call_wall) = profile.call_wall {
            log::info!("  Call wall: ${:.2}", call_wall.to_f64());
        }

        if let Some(put_wall) = profile.put_wall {
            log::info!("  Put wall: ${:.2}", put_wall.to_f64());
        }

        if let Some(zero_gamma) = profile.zero_gamma_level {
            log::info!("  Zero gamma: ${:.2}", zero_gamma.to_f64());
        }

        Ok(profile)
    }

    /// Get GEX profiles for a date range
    pub async fn get_gex_profiles(
        &self,
        underlying_ticker: &str,
        date_range: &DateRange,
    ) -> RepositoryResult<Vec<GexProfile>> {
        let chains = self
            .get_chains_for_range(underlying_ticker, date_range)
            .await?;

        let profiles: Vec<GexProfile> = chains
            .iter()
            .filter_map(|chain| GexProfile::from_option_chain(chain))
            .collect();

        Ok(profiles)
    }

    /// Get chain filtered by strike range (near-the-money options)
    pub async fn get_chain_near_atm(
        &self,
        underlying_ticker: &str,
        date: NaiveDate,
        strike_range_pct: f64, // e.g., 0.1 for ±10%
    ) -> RepositoryResult<OptionChain> {
        // Get full chain first to determine ATM strike
        let full_chain = self.get_chain_with_greeks(underlying_ticker, date).await?;

        let underlying_price = full_chain.underlying_price.ok_or_else(|| {
            RepositoryError::InvalidData("Underlying price not available".to_string())
        })?;

        let underlying_f64 = underlying_price.to_f64();
        let min_strike = underlying_f64 * (1.0 - strike_range_pct);
        let max_strike = underlying_f64 * (1.0 + strike_range_pct);

        // Fetch filtered chain
        self.chain_repo
            .get_chain_by_strike_range(underlying_ticker, date, min_strike, max_strike)
            .await
    }

    /// Get chain for a specific expiration
    pub async fn get_chain_by_expiration(
        &self,
        underlying_ticker: &str,
        date: NaiveDate,
        expiration: NaiveDate,
    ) -> RepositoryResult<OptionChain> {
        self.chain_repo
            .get_chain_by_expiration(underlying_ticker, date, expiration)
            .await
    }

    /// Get loading status for a key
    pub async fn get_loading_status(&self, key: &str) -> Option<LoadingStatus> {
        let status = self.loading_status.lock().await;
        status.get(key).cloned()
    }

    /// Get all loading statuses
    pub async fn get_all_loading_statuses(&self) -> HashMap<String, LoadingStatus> {
        let status = self.loading_status.lock().await;
        status.clone()
    }

    /// Clear completed and errored loading statuses
    pub async fn clear_old_statuses(&self) {
        let mut status = self.loading_status.lock().await;
        status.retain(|_, s| {
            !matches!(s, LoadingStatus::Ready | LoadingStatus::Error { .. })
        });
    }

    /// Get cache statistics from repositories
    pub async fn get_cache_stats(&self) -> String {
        let snapshot_stats = self.snapshot_repo.stats().await.ok();
        let chain_stats = self.chain_repo.stats().await.ok();
        let contract_stats = self.contract_repo.stats().await.ok();

        format!(
            "Snapshots: {}\nChains: {}\nContracts: {}",
            snapshot_stats.map_or("N/A".to_string(), |s| s.to_string()),
            chain_stats.map_or("N/A".to_string(), |s| s.to_string()),
            contract_stats.map_or("N/A".to_string(), |s| s.to_string())
        )
    }

    // Private helper methods

    async fn set_loading_status(&self, key: &str, status: LoadingStatus) {
        let mut statuses = self.loading_status.lock().await;
        statuses.insert(key.to_string(), status);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{ExerciseStyle, OptionType, Price, Timestamp};
    use crate::repository::RepositoryStats;
    use async_trait::async_trait;
    use chrono::Utc;

    // Mock repository for testing
    struct MockSnapshotRepository;

    #[async_trait]
    impl OptionSnapshotRepository for MockSnapshotRepository {
        async fn get_snapshots(
            &self,
            _underlying_ticker: &str,
            _date_range: &DateRange,
        ) -> RepositoryResult<Vec<OptionSnapshot>> {
            Ok(vec![])
        }

        async fn get_snapshot(
            &self,
            _contract_ticker: &str,
            _date: NaiveDate,
        ) -> RepositoryResult<OptionSnapshot> {
            let contract = OptionContract::new(
                "O:TEST".to_string(),
                "TEST".to_string(),
                Price::from_f64(100.0),
                NaiveDate::from_ymd_opt(2024, 12, 31).unwrap(),
                OptionType::Call,
                ExerciseStyle::American,
            );
            Ok(OptionSnapshot::new(contract, Timestamp(Utc::now().timestamp_millis() as u64)))
        }

        async fn get_snapshots_for_contracts(
            &self,
            _contract_tickers: &[String],
            _date: NaiveDate,
        ) -> RepositoryResult<Vec<OptionSnapshot>> {
            Ok(vec![])
        }

        async fn has_snapshots(
            &self,
            _underlying_ticker: &str,
            _date: NaiveDate,
        ) -> RepositoryResult<bool> {
            Ok(true)
        }

        async fn store_snapshots(
            &self,
            _underlying_ticker: &str,
            _date: NaiveDate,
            _snapshots: Vec<OptionSnapshot>,
        ) -> RepositoryResult<()> {
            Ok(())
        }

        async fn find_gaps(
            &self,
            _underlying_ticker: &str,
            _date_range: &DateRange,
        ) -> RepositoryResult<Vec<DateRange>> {
            Ok(vec![])
        }

        async fn stats(&self) -> RepositoryResult<RepositoryStats> {
            Ok(RepositoryStats::new())
        }
    }

    struct MockChainRepository;

    #[async_trait]
    impl OptionChainRepository for MockChainRepository {
        async fn get_chain(
            &self,
            underlying_ticker: &str,
            date: NaiveDate,
        ) -> RepositoryResult<OptionChain> {
            Ok(OptionChain::new(
                underlying_ticker.to_string(),
                date,
                Timestamp(Utc::now().timestamp_millis() as u64),
            ))
        }

        async fn get_chains(
            &self,
            _underlying_ticker: &str,
            _date_range: &DateRange,
        ) -> RepositoryResult<Vec<OptionChain>> {
            Ok(vec![])
        }

        async fn get_chain_by_strike_range(
            &self,
            underlying_ticker: &str,
            date: NaiveDate,
            _min_strike: f64,
            _max_strike: f64,
        ) -> RepositoryResult<OptionChain> {
            Ok(OptionChain::new(
                underlying_ticker.to_string(),
                date,
                Timestamp(Utc::now().timestamp_millis() as u64),
            ))
        }

        async fn get_chain_by_expiration(
            &self,
            underlying_ticker: &str,
            date: NaiveDate,
            _expiration: NaiveDate,
        ) -> RepositoryResult<OptionChain> {
            Ok(OptionChain::new(
                underlying_ticker.to_string(),
                date,
                Timestamp(Utc::now().timestamp_millis() as u64),
            ))
        }

        async fn has_chain(
            &self,
            _underlying_ticker: &str,
            _date: NaiveDate,
        ) -> RepositoryResult<bool> {
            Ok(true)
        }

        async fn store_chain(
            &self,
            _underlying_ticker: &str,
            _date: NaiveDate,
            _chain: OptionChain,
        ) -> RepositoryResult<()> {
            Ok(())
        }

        async fn find_gaps(
            &self,
            _underlying_ticker: &str,
            _date_range: &DateRange,
        ) -> RepositoryResult<Vec<DateRange>> {
            Ok(vec![])
        }

        async fn stats(&self) -> RepositoryResult<RepositoryStats> {
            Ok(RepositoryStats::new())
        }
    }

    struct MockContractRepository;

    #[async_trait]
    impl OptionContractRepository for MockContractRepository {
        async fn get_contracts(
            &self,
            _underlying_ticker: &str,
        ) -> RepositoryResult<Vec<OptionContract>> {
            Ok(vec![])
        }

        async fn get_active_contracts(
            &self,
            _underlying_ticker: &str,
            _as_of: NaiveDate,
        ) -> RepositoryResult<Vec<OptionContract>> {
            Ok(vec![])
        }

        async fn get_contract(
            &self,
            _contract_ticker: &str,
        ) -> RepositoryResult<OptionContract> {
            let contract = OptionContract::new(
                "O:TEST".to_string(),
                "TEST".to_string(),
                Price::from_f64(100.0),
                NaiveDate::from_ymd_opt(2024, 12, 31).unwrap(),
                OptionType::Call,
                ExerciseStyle::American,
            );
            Ok(contract)
        }

        async fn search_contracts(
            &self,
            _underlying_ticker: Option<&str>,
            _expiration: Option<NaiveDate>,
            _min_strike: Option<f64>,
            _max_strike: Option<f64>,
            _include_expired: bool,
        ) -> RepositoryResult<Vec<OptionContract>> {
            Ok(vec![])
        }

        async fn get_contracts_by_expiration(
            &self,
            _underlying_ticker: &str,
            _expiration: NaiveDate,
        ) -> RepositoryResult<Vec<OptionContract>> {
            Ok(vec![])
        }

        async fn has_contract(&self, _contract_ticker: &str) -> RepositoryResult<bool> {
            Ok(true)
        }

        async fn store_contract(&self, _contract: OptionContract) -> RepositoryResult<()> {
            Ok(())
        }

        async fn store_contracts(&self, _contracts: Vec<OptionContract>) -> RepositoryResult<()> {
            Ok(())
        }

        async fn stats(&self) -> RepositoryResult<RepositoryStats> {
            Ok(RepositoryStats::new())
        }
    }

    #[tokio::test]
    async fn test_options_data_service() {
        let snapshot_repo = Arc::new(MockSnapshotRepository);
        let chain_repo = Arc::new(MockChainRepository);
        let contract_repo = Arc::new(MockContractRepository);

        let service =
            OptionsDataService::new(snapshot_repo, chain_repo, contract_repo);

        // Test get_snapshot
        let date = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
        let snapshot = service.get_snapshot("O:TEST", date).await;
        assert!(snapshot.is_ok());

        // Test get_chain_with_greeks
        let chain = service.get_chain_with_greeks("TEST", date).await;
        assert!(chain.is_ok());
    }
}
