use super::cache::CacheManager;
use super::client::MassiveClient;
use super::util::extract_underlying_massive;
use super::decoder::{
    MassiveContractMetadata, MassiveOptionSnapshot, parse_array_results, parse_single_result,
};
use super::mapper::{convert_chain_response, convert_contract_response, convert_snapshot_response};
use super::{MassiveConfig, MassiveError, MassiveResult};
use chrono::NaiveDate;
use kairos_data::domain::{DateRange, OptionChain, OptionContract, OptionSnapshot};
use std::sync::Arc;

/// Historical options data manager
///
/// Provides intelligent fetching of historical options data with:
/// - Smart gap detection to minimize API calls
/// - Per-day caching for reuse
/// - Progress logging for UI integration
/// - Batch optimization
pub struct HistoricalOptionsManager {
    client: Arc<MassiveClient>,
    pub cache: Arc<CacheManager>,
    config: MassiveConfig,
}

impl HistoricalOptionsManager {
    /// Create a new historical options manager
    pub async fn new(config: MassiveConfig) -> MassiveResult<Self> {
        let client = Arc::new(MassiveClient::new(config.clone())?);
        let cache = Arc::new(CacheManager::new(
            config.cache_dir.clone(),
            config.cache_max_days,
        ));

        // Initialize cache
        cache.init().await?;

        Ok(Self {
            client,
            cache,
            config,
        })
    }

    /// Fetch option chain for a single date
    ///
    /// Returns the complete chain with all strikes and expirations.
    pub async fn fetch_option_chain(
        &self,
        underlying_ticker: &str,
        date: NaiveDate,
    ) -> MassiveResult<OptionChain> {
        log::debug!(
            "Fetching option chain for {} on {}",
            underlying_ticker,
            date
        );

        // Check cache first
        if self.config.cache_enabled
            && self
                .cache
                .has_cached("chains", underlying_ticker, date)
                .await
        {
            log::debug!("Cache hit for {} chain on {}", underlying_ticker, date);
            return self
                .cache
                .load("chains", underlying_ticker, Some(date))
                .await;
        }

        // Fetch from API
        log::debug!("Fetching {} chain from API...", underlying_ticker);

        let url = format!(
            "https://api.polygon.io/v3/snapshot/options/{}",
            underlying_ticker,
        );

        let response = self.client.get(&url).await?;
        let status = response.status();

        if status == reqwest::StatusCode::NOT_FOUND {
            return Err(MassiveError::SymbolNotFound(underlying_ticker.to_string()));
        }

        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(MassiveError::Api(format!(
                "Failed to fetch chain: {} - {}",
                status, body
            )));
        }

        let body = response.text().await?;
        let snapshots: Vec<MassiveOptionSnapshot> = parse_array_results(&body)?;

        if snapshots.is_empty() {
            log::warn!("No options data found for {}", underlying_ticker);
        }

        let chain = convert_chain_response(underlying_ticker.to_string(), date, snapshots)?;

        log::debug!("Fetched chain with {} contracts", chain.contract_count());

        // Cache result
        if self.config.cache_enabled {
            self.cache
                .store("chains", underlying_ticker, Some(date), &chain)
                .await?;
        }

        Ok(chain)
    }

    /// Fetch option chains for a date range
    ///
    /// Uses smart gap detection to minimize API calls.
    pub async fn fetch_option_chains(
        &self,
        underlying_ticker: &str,
        date_range: &DateRange,
    ) -> MassiveResult<Vec<OptionChain>> {
        let total_days = date_range.num_days();
        log::debug!(
            "Fetching {} days of option chains for {}",
            total_days,
            underlying_ticker
        );

        // Find gaps in cache
        let gaps = self.find_chain_gaps(underlying_ticker, date_range).await?;

        if gaps.is_empty() {
            log::debug!("All {} days cached", total_days);
        } else {
            log::debug!(
                "Found {} gaps totaling {} days to fetch",
                gaps.len(),
                gaps.iter().map(|g| g.num_days()).sum::<i64>()
            );

            // Fetch missing data
            for (gap_idx, gap) in gaps.iter().enumerate() {
                log::debug!(
                    "Fetching gap {}/{}: {} days ({} to {})",
                    gap_idx + 1,
                    gaps.len(),
                    gap.num_days(),
                    gap.start,
                    gap.end
                );

                for date in gap.dates() {
                    match self.fetch_option_chain(underlying_ticker, date).await {
                        Ok(_) => {
                            log::debug!("Fetched chain for {}", date);
                        }
                        Err(e) => {
                            log::error!("FAILED: Failed to fetch chain for {}: {}", date, e);
                            // Continue with other dates
                        }
                    }

                    // Small delay to respect rate limits
                    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                }

                log::debug!("Gap {}/{} complete", gap_idx + 1, gaps.len());
            }
        }

        // Load all data from cache
        let mut chains = Vec::new();
        for date in date_range.dates() {
            if let Ok(chain) = self
                .cache
                .load("chains", underlying_ticker, Some(date))
                .await
            {
                chains.push(chain);
            }
        }

        log::info!("Loaded {} chains total", chains.len());

        Ok(chains)
    }

    /// Fetch a single option contract snapshot
    pub async fn fetch_contract_snapshot(
        &self,
        contract_ticker: &str,
        date: NaiveDate,
    ) -> MassiveResult<OptionSnapshot> {
        log::debug!("Fetching snapshot for {} on {}", contract_ticker, date);

        let url = format!(
            "https://api.polygon.io/v3/snapshot/options/{}/{}",
            extract_underlying_massive(contract_ticker)?,
            contract_ticker,
        );

        let response = self.client.get(&url).await?;
        let status = response.status();

        if status == reqwest::StatusCode::NOT_FOUND {
            return Err(MassiveError::SymbolNotFound(contract_ticker.to_string()));
        }

        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(MassiveError::Api(format!(
                "Failed to fetch snapshot: {} - {}",
                status, body
            )));
        }

        let body = response.text().await?;
        let massive_snapshot: MassiveOptionSnapshot = parse_single_result(&body)?;

        convert_snapshot_response(massive_snapshot)
    }

    /// Fetch all available contracts for an underlying
    pub async fn fetch_contracts_metadata(
        &self,
        underlying_ticker: &str,
    ) -> MassiveResult<Vec<OptionContract>> {
        log::debug!("Fetching contracts metadata for {}", underlying_ticker);

        // Check cache first (contracts change less frequently)
        if self.config.cache_enabled
            && let Ok(contracts) = self
                .cache
                .load::<Vec<OptionContract>>("contracts", underlying_ticker, None)
                .await
        {
            log::debug!("Cache hit for {} contracts", underlying_ticker);
            return Ok(contracts);
        }

        // Fetch from API
        log::debug!("Fetching contracts from API...");

        let url = format!(
            "https://api.polygon.io/v3/reference/options/contracts\
            ?underlying_ticker={}&limit=1000",
            underlying_ticker,
        );

        let response = self.client.get(&url).await?;
        let status = response.status();

        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(MassiveError::Api(format!(
                "Failed to fetch contracts: {} - {}",
                status, body
            )));
        }

        let body = response.text().await?;
        let massive_contracts: Vec<MassiveContractMetadata> = parse_array_results(&body)?;

        let mut contracts = Vec::new();
        for massive_contract in massive_contracts {
            match convert_contract_response(massive_contract) {
                Ok(contract) => contracts.push(contract),
                Err(e) => {
                    log::warn!("Failed to convert contract: {}", e);
                }
            }
        }

        log::info!("Fetched {} contracts", contracts.len());

        // Cache result
        if self.config.cache_enabled {
            self.cache
                .store("contracts", underlying_ticker, None, &contracts)
                .await?;
        }

        Ok(contracts)
    }

    /// Find gaps in cached chains
    pub async fn find_chain_gaps(
        &self,
        underlying_ticker: &str,
        date_range: &DateRange,
    ) -> MassiveResult<Vec<DateRange>> {
        if !self.config.cache_enabled {
            return Ok(vec![*date_range]);
        }

        let mut gaps = Vec::new();
        let mut gap_start: Option<NaiveDate> = None;

        for date in date_range.dates() {
            let has_cache = self
                .cache
                .has_cached("chains", underlying_ticker, date)
                .await;

            if !has_cache {
                // Start or continue gap
                if gap_start.is_none() {
                    gap_start = Some(date);
                }
            } else {
                // End gap if one was in progress
                if let Some(start) = gap_start {
                    if let Ok(range) = DateRange::new(start, date - chrono::Duration::days(1)) {
                        gaps.push(range);
                    }
                    gap_start = None;
                }
            }
        }

        // Close final gap if range ends with missing data
        if let Some(start) = gap_start {
            if let Ok(range) = DateRange::new(start, date_range.end) {
                gaps.push(range);
            }
        }

        Ok(gaps)
    }

    /// Get cache statistics
    pub async fn cache_stats(&self) -> MassiveResult<super::cache::CacheStats> {
        self.cache.stats().await
    }

    /// Clean up old cache files
    pub async fn cleanup_cache(&self) -> MassiveResult<usize> {
        self.cache.cleanup_old_files().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_find_chain_gaps() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let config = MassiveConfig {
            api_key: "test_key".to_string(),
            cache_enabled: true,
            cache_dir: temp_dir.path().to_path_buf(),
            cache_max_days: 30,
            rate_limit_per_minute: 5,
            warn_on_rate_limits: false,
            timeout_secs: 30,
            max_retries: 3,
            retry_delay_ms: 1000,
        };

        let manager = HistoricalOptionsManager::new(config).await.unwrap();

        // Create some cached data
        let test_chain = OptionChain::new(
            "TEST".to_string(),
            NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            kairos_data::domain::Timestamp(0),
        );

        manager
            .cache
            .store(
                "chains",
                "TEST",
                Some(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap()),
                &test_chain,
            )
            .await
            .unwrap();

        manager
            .cache
            .store(
                "chains",
                "TEST",
                Some(NaiveDate::from_ymd_opt(2024, 1, 3).unwrap()),
                &test_chain,
            )
            .await
            .unwrap();

        // Test gap detection
        let date_range = DateRange::new(
            NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2024, 1, 5).unwrap(),
        )
        .expect("test: valid date range");

        let gaps = manager.find_chain_gaps("TEST", &date_range).await.unwrap();

        // Should have gaps: Jan 2, Jan 4-5
        assert!(!gaps.is_empty());
    }
}
