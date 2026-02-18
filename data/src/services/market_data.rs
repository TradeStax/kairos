//! Market Data Service
//!
//! High-level service for fetching and aggregating market data.
//! Coordinates repositories and applies business logic.

use crate::domain::{
    Candle, DateRange, Price, Trade,
    aggregation::{AggregationError, aggregate_trades_to_candles, aggregate_trades_to_ticks},
    chart::{ChartBasis, ChartConfig, ChartData, DataSchema, LoadingStatus},
};
use crate::domain::{FuturesTicker, FuturesTickerInfo};
use crate::repository::{DepthRepository, RepositoryError, TradeRepository};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use thiserror::Error;

/// Service error types
#[derive(Error, Debug)]
pub enum ServiceError {
    #[error("Repository error: {0}")]
    Repository(#[from] RepositoryError),

    #[error("Aggregation error: {0}")]
    Aggregation(#[from] AggregationError),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("No data available: {0}")]
    NoData(String),
}

pub type ServiceResult<T> = Result<T, ServiceError>;

/// Market data service
///
/// Provides high-level operations for fetching and aggregating market data.
/// This is the primary entry point for chart data in the application.
///
/// ## Example Usage
///
/// ```rust
/// use data::services::MarketDataService;
/// use data::domain::chart::{ChartConfig, ChartBasis};
///
/// let service = MarketDataService::new(trade_repo, depth_repo);
///
/// // Fetch chart data
/// let chart_data = service.get_chart_data(config).await?;
///
/// // Trades are now in memory, can switch basis instantly
/// let new_chart_data = service.rebuild_chart_data(
///     &chart_data.trades,
///     ChartBasis::Tick(50),
///     ticker_info
/// )?;
/// ```
pub struct MarketDataService {
    trade_repo: Arc<dyn TradeRepository>,
    depth_repo: Arc<dyn DepthRepository>,
    loading_status: Arc<Mutex<HashMap<String, LoadingStatus>>>,
}

impl MarketDataService {
    /// Create a new market data service
    pub fn new(trade_repo: Arc<dyn TradeRepository>, depth_repo: Arc<dyn DepthRepository>) -> Self {
        Self {
            trade_repo,
            depth_repo,
            loading_status: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Get chart data based on configuration
    ///
    /// This is the PRIMARY method for loading chart data.
    /// It:
    /// 1. Fetches trades from repository (cache + remote)
    /// 2. Aggregates trades to target basis
    /// 3. Returns complete ChartData with trades in memory
    ///
    /// ## Performance
    /// - First load: 15-25s (API bound)
    /// - Cached load: <1s
    /// - Trades kept in memory for instant basis switching
    pub async fn get_chart_data(
        &self,
        config: &ChartConfig,
        ticker_info: &FuturesTickerInfo,
    ) -> ServiceResult<ChartData> {
        log::info!(
            "MarketDataService::get_chart_data: {} {:?}",
            config.ticker.as_str(),
            config.basis
        );

        // For heatmaps, automatically limit to most recent day to prevent memory issues
        let effective_date_range = if config.chart_type == crate::domain::ChartType::Heatmap {
            if config.date_range.num_days() > 1 {
                log::warn!(
                    "Heatmap: date range truncated from {} days to 1 day \
                     (showing {}). Multi-day heatmaps are not supported \
                     due to memory constraints.",
                    config.date_range.num_days(),
                    config.date_range.end
                );
                DateRange::new(config.date_range.end, config.date_range.end)
            } else {
                config.date_range
            }
        } else {
            config.date_range
        };

        // Create unique key for this chart configuration
        let chart_key = format!(
            "{}-{:?}-{:?}",
            config.ticker, config.basis, config.date_range
        );

        log::debug!("About to acquire loading_status lock...");

        // Update loading status: start downloading
        {
            let mut status_map = crate::lock_or_recover(&self.loading_status);
            log::debug!("Loading status lock acquired successfully");
            let date_range_days = effective_date_range.num_days() as usize;
            status_map.insert(
                chart_key.clone(),
                LoadingStatus::Downloading {
                    schema: DataSchema::Trades,
                    days_total: date_range_days,
                    days_complete: 0,
                    current_day: effective_date_range.start.to_string(),
                },
            );
        }

        // Step 1: Fetch trades from repository WITH PROGRESS CALLBACK
        // Create a progress callback that updates loading_status
        let status_map_clone = Arc::clone(&self.loading_status);
        let chart_key_clone = chart_key.clone();
        let progress_callback: Box<dyn Fn(LoadingStatus) + Send + Sync> =
            Box::new(move |status: LoadingStatus| {
                let mut map = crate::lock_or_recover(&status_map_clone);
                map.insert(chart_key_clone.clone(), status);
            });

        let trades = match self
            .trade_repo
            .get_trades_with_progress(&config.ticker, &effective_date_range, progress_callback)
            .await
        {
            Ok(trades) => trades,
            Err(e) => {
                // Update status to error
                let mut status_map = crate::lock_or_recover(&self.loading_status);
                status_map.insert(
                    chart_key.clone(),
                    LoadingStatus::Error {
                        message: format!("Failed to fetch trades: {:?}", e),
                    },
                );
                return Err(ServiceError::Repository(e));
            }
        };

        if trades.is_empty() {
            return Err(ServiceError::NoData(format!(
                "No trades found for {} in range {:?}",
                config.ticker.as_str(),
                effective_date_range
            )));
        }

        log::info!("Loaded {} trades from repository", trades.len());

        // Update status: now building chart (aggregating trades)
        {
            let mut status_map = crate::lock_or_recover(&self.loading_status);
            status_map.insert(
                chart_key.clone(),
                LoadingStatus::Building {
                    operation: format!("Aggregating {} trades", trades.len()),
                    progress: 0.3,
                },
            );
        }

        // Step 2: Aggregate to target basis
        let candles = match self.aggregate_to_basis(&trades, config.basis, ticker_info) {
            Ok(candles) => candles,
            Err(e) => {
                // Update status to error
                let mut status_map = crate::lock_or_recover(&self.loading_status);
                status_map.insert(
                    chart_key.clone(),
                    LoadingStatus::Error {
                        message: format!("Failed to aggregate: {:?}", e),
                    },
                );
                return Err(e);
            }
        };

        log::info!(
            "Aggregated to {} candles ({:?})",
            candles.len(),
            config.basis
        );

        // Update status: building candles complete
        {
            let mut status_map = crate::lock_or_recover(&self.loading_status);
            status_map.insert(
                chart_key.clone(),
                LoadingStatus::Building {
                    operation: format!("Built {} candles", candles.len()),
                    progress: 0.6,
                },
            );
        }

        // Step 3: Load depth data for heatmap charts
        let mut chart_data = ChartData::from_trades(trades, candles);

        if config.chart_type == crate::domain::ChartType::Heatmap {
            log::info!("Heatmap chart detected - loading MBP-10 depth data for {} ({} to {})",
                config.ticker, effective_date_range.start, effective_date_range.end);

            // Update status: loading depth data
            {
                let mut status_map = crate::lock_or_recover(&self.loading_status);
                status_map.insert(
                    chart_key.clone(),
                    LoadingStatus::Downloading {
                        schema: DataSchema::MBP10,
                        days_total: effective_date_range.num_days() as usize,
                        days_complete: 0,
                        current_day: effective_date_range.start.to_string(),
                    },
                );
            }

            let depth_start = std::time::Instant::now();
            match self.depth_repo.get_depth(&config.ticker, &effective_date_range).await {
                Ok(depth_snapshots) => {
                    log::info!("Loaded {} depth snapshots for heatmap in {:.2}s",
                        depth_snapshots.len(), depth_start.elapsed().as_secs_f32());

                    // Update status: processing depth data
                    {
                        let mut status_map = crate::lock_or_recover(&self.loading_status);
                        status_map.insert(
                            chart_key.clone(),
                            LoadingStatus::Building {
                                operation: format!("Processing {} depth snapshots", depth_snapshots.len()),
                                progress: 0.9,
                            },
                        );
                    }

                    chart_data = chart_data.with_depth(depth_snapshots);
                }
                Err(e) => {
                    log::warn!("Failed to load depth data for heatmap: {:?}. Chart will show trades only.", e);
                    // Don't fail the entire load - heatmap can still show trades
                }
            }
        }

        // Update status: ready
        {
            let mut status_map = crate::lock_or_recover(&self.loading_status);
            status_map.insert(chart_key.clone(), LoadingStatus::Ready);
        }

        // Step 4: Return chart data with depth (if heatmap)
        Ok(chart_data)
    }

    /// Rebuild chart data with a new basis (INSTANT - uses trades in memory)
    ///
    /// This enables instant basis switching without refetching data.
    /// Takes existing trades and aggregates to new basis in <100ms.
    ///
    /// ## Example
    /// ```rust
    /// // User has 5M chart loaded
    /// let chart_data = service.get_chart_data(config_5m, ticker_info).await?;
    ///
    /// // User switches to 50T - INSTANT rebuild from memory
    /// let new_chart_data = service.rebuild_chart_data(
    ///     &chart_data.trades,
    ///     ChartBasis::Tick(50),
    ///     ticker_info
    /// )?;
    /// // <100ms, no API calls
    /// ```
    pub fn rebuild_chart_data(
        &self,
        trades: &[Trade],
        new_basis: ChartBasis,
        ticker_info: &FuturesTickerInfo,
    ) -> ServiceResult<ChartData> {
        log::info!(
            "MarketDataService::rebuild_chart_data: {} trades to {:?}",
            trades.len(),
            new_basis
        );

        if trades.is_empty() {
            return Err(ServiceError::NoData(
                "No trades to rebuild from".to_string(),
            ));
        }

        // Aggregate to new basis (local, fast)
        let candles = self.aggregate_to_basis(trades, new_basis, ticker_info)?;

        log::info!("Rebuilt to {} candles in <100ms", candles.len());

        // Create new chart data (trades copied, candles new)
        Ok(ChartData::from_trades(trades.to_vec(), candles))
    }

    /// Aggregate trades to target basis
    fn aggregate_to_basis(
        &self,
        trades: &[Trade],
        basis: ChartBasis,
        ticker_info: &FuturesTickerInfo,
    ) -> ServiceResult<Vec<Candle>> {
        let tick_size = Price::from_f32(ticker_info.tick_size);

        let candles = match basis {
            ChartBasis::Time(timeframe) => {
                // Time-based aggregation (M1, M5, H1, etc.)
                let timeframe_millis = timeframe.to_milliseconds();
                aggregate_trades_to_candles(trades, timeframe_millis, tick_size)?
            }
            ChartBasis::Tick(tick_count) => {
                // Tick-based aggregation (50T, 100T, etc.)
                aggregate_trades_to_ticks(trades, tick_count, tick_size)?
            }
        };

        Ok(candles)
    }

    /// Get loading status for a chart configuration
    ///
    /// This can be used by the UI to show progress during data loading.
    pub fn get_loading_status(&self, config: &ChartConfig) -> LoadingStatus {
        let chart_key = format!(
            "{}-{:?}-{:?}",
            config.ticker, config.basis, config.date_range
        );

        let status_map = crate::lock_or_recover(&self.loading_status);
        status_map
            .get(&chart_key)
            .cloned()
            .unwrap_or(LoadingStatus::Idle)
    }

    /// Get all loading statuses
    ///
    /// Returns all ongoing operations across all charts.
    pub fn get_all_loading_statuses(&self) -> HashMap<String, LoadingStatus> {
        let status_map = crate::lock_or_recover(&self.loading_status);
        status_map.clone()
    }

    /// Clear completed and errored loading statuses
    ///
    /// Removes Ready and Error statuses, keeping only in-progress ones.
    pub fn clear_old_statuses(&self) {
        let mut status_map = crate::lock_or_recover(&self.loading_status);
        status_map.retain(|_, status| {
            matches!(
                status,
                LoadingStatus::Downloading { .. }
                    | LoadingStatus::LoadingFromCache { .. }
                    | LoadingStatus::Building { .. }
            )
        });
    }

    /// Get repository statistics
    pub async fn get_cache_stats(&self) -> ServiceResult<String> {
        let stats = self.trade_repo.stats().await?;
        Ok(format!("{}", stats))
    }

    /// Estimate cache coverage and cost for a data request
    ///
    /// Returns (total_days, cached_days, uncached_days, gaps_description, actual_cost_usd, cached_dates)
    pub async fn estimate_data_request(
        &self,
        ticker: &FuturesTicker,
        schema_discriminant: u16,
        date_range: &DateRange,
    ) -> ServiceResult<(usize, usize, usize, String, f64, Vec<chrono::NaiveDate>)> {
        log::info!("Estimating cost for {} ({}) from {} to {}",
            ticker, schema_discriminant, date_range.start, date_range.end);

        // Get cache coverage
        let coverage = self
            .trade_repo
            .check_cache_coverage_databento(ticker, schema_discriminant, date_range)
            .await?;

        log::debug!("Cache coverage: {} cached, {} uncached out of {} total days",
            coverage.cached_count, coverage.uncached_count, date_range.num_days());

        let total_days = date_range.num_days() as usize;
        let cached_days = coverage.cached_count;
        let uncached_days = coverage.uncached_count;

        // Get cost from Databento API for FULL range
        let full_range_cost = self
            .trade_repo
            .get_actual_cost_databento(ticker, schema_discriminant, date_range)
            .await?; // Propagate error instead of defaulting to $0

        log::info!("Databento cost API: ${:.4} USD for full range ({} days)", full_range_cost, total_days);

        // Calculate ACTUAL download cost (only for uncached days)
        let actual_cost_usd = if total_days > 0 && uncached_days > 0 {
            // Cost per day = total cost / total days
            let cost_per_day = full_range_cost / total_days as f64;
            // Actual cost = cost per day × uncached days
            let cost = cost_per_day * uncached_days as f64;
            log::info!("Actual cost for {} uncached days: ${:.4} (${:.4}/day)", uncached_days, cost, cost_per_day);
            cost
        } else {
            0.0 // All cached or no days
        };

        // Format gaps description
        let gaps_desc = if coverage.gaps.is_empty() {
            "All days cached".to_string()
        } else {
            let gap_strs: Vec<String> = coverage
                .gaps
                .iter()
                .map(|(start, end): &(chrono::NaiveDate, chrono::NaiveDate)| {
                    if start == end {
                        format!("{}", start.format("%b %d"))
                    } else {
                        format!("{} to {}", start.format("%b %d"), end.format("%b %d"))
                    }
                })
                .collect();
            format!("Gaps: {}", gap_strs.join(", "))
        };

        Ok((total_days, cached_days, uncached_days, gaps_desc, actual_cost_usd, coverage.cached_dates))
    }

    /// Download data to cache without loading into memory
    ///
    /// Returns number of days successfully downloaded
    pub async fn download_to_cache(
        &self,
        ticker: &FuturesTicker,
        schema_discriminant: u16,
        date_range: &DateRange,
    ) -> ServiceResult<usize> {
        let days_downloaded = self
            .trade_repo
            .prefetch_to_cache_databento(ticker, schema_discriminant, date_range)
            .await?;

        Ok(days_downloaded)
    }

    /// Download data to cache WITH progress callbacks
    ///
    /// Calls progress_callback after each day is downloaded for UI updates.
    /// Returns number of days successfully downloaded.
    pub async fn download_to_cache_with_progress(
        &self,
        ticker: &FuturesTicker,
        schema_discriminant: u16,
        date_range: &DateRange,
        progress_callback: Box<dyn Fn(usize, usize) + Send + Sync>,
    ) -> ServiceResult<usize> {
        let days_downloaded = self
            .trade_repo
            .prefetch_to_cache_databento_with_progress(
                ticker,
                schema_discriminant,
                date_range,
                progress_callback,
            )
            .await?;

        Ok(days_downloaded)
    }

    /// Get list of tickers with cached data
    pub async fn get_cached_tickers(&self) -> ServiceResult<std::collections::HashSet<String>> {
        self.trade_repo
            .list_cached_symbols_databento()
            .await
            .map_err(ServiceError::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Quantity, Timestamp, types::Side};
    use crate::repository::traits::{
        DepthRepository, RepositoryResult, RepositoryStats, TradeRepository,
    };
    use chrono::NaiveDate;

    // Mock repository for testing
    struct MockTradeRepository {
        trades: Vec<Trade>,
    }

    #[async_trait::async_trait]
    impl TradeRepository for MockTradeRepository {
        async fn get_trades(
            &self,
            _ticker: &FuturesTicker,
            _date_range: &DateRange,
        ) -> RepositoryResult<Vec<Trade>> {
            Ok(self.trades.clone())
        }

        async fn has_trades(
            &self,
            _ticker: &FuturesTicker,
            _date: NaiveDate,
        ) -> RepositoryResult<bool> {
            Ok(true)
        }

        async fn get_trades_for_date(
            &self,
            _ticker: &FuturesTicker,
            _date: NaiveDate,
        ) -> RepositoryResult<Vec<Trade>> {
            Ok(self.trades.clone())
        }

        async fn store_trades(
            &self,
            _ticker: &FuturesTicker,
            _date: NaiveDate,
            _trades: Vec<Trade>,
        ) -> RepositoryResult<()> {
            Ok(())
        }

        async fn find_gaps(
            &self,
            _ticker: &FuturesTicker,
            _date_range: &DateRange,
        ) -> RepositoryResult<Vec<DateRange>> {
            Ok(Vec::new())
        }

        async fn stats(&self) -> RepositoryResult<RepositoryStats> {
            Ok(RepositoryStats::new())
        }
    }

    struct MockDepthRepository;

    #[async_trait::async_trait]
    impl DepthRepository for MockDepthRepository {
        async fn get_depth(
            &self,
            _ticker: &FuturesTicker,
            _date_range: &DateRange,
        ) -> RepositoryResult<Vec<crate::domain::DepthSnapshot>> {
            Ok(Vec::new())
        }

        async fn has_depth(
            &self,
            _ticker: &FuturesTicker,
            _date: NaiveDate,
        ) -> RepositoryResult<bool> {
            Ok(false)
        }

        async fn get_depth_for_date(
            &self,
            _ticker: &FuturesTicker,
            _date: NaiveDate,
        ) -> RepositoryResult<Vec<crate::domain::DepthSnapshot>> {
            Ok(Vec::new())
        }

        async fn store_depth(
            &self,
            _ticker: &FuturesTicker,
            _date: NaiveDate,
            _depth: Vec<crate::domain::DepthSnapshot>,
        ) -> RepositoryResult<()> {
            Ok(())
        }

        async fn find_gaps(
            &self,
            _ticker: &FuturesTicker,
            _date_range: &DateRange,
        ) -> RepositoryResult<Vec<DateRange>> {
            Ok(Vec::new())
        }

        async fn stats(&self) -> RepositoryResult<RepositoryStats> {
            Ok(RepositoryStats::new())
        }
    }

    fn create_test_trades() -> Vec<Trade> {
        vec![
            Trade::new(
                Timestamp(1000),
                Price::from_f32(100.0),
                Quantity(10.0),
                Side::Buy,
            ),
            Trade::new(
                Timestamp(2000),
                Price::from_f32(101.0),
                Quantity(5.0),
                Side::Sell,
            ),
            Trade::new(
                Timestamp(3000),
                Price::from_f32(99.5),
                Quantity(8.0),
                Side::Sell,
            ),
        ]
    }

    #[tokio::test]
    async fn test_rebuild_chart_data() {
        let trades = create_test_trades();
        let trade_repo = Arc::new(MockTradeRepository {
            trades: trades.clone(),
        });
        let depth_repo = Arc::new(MockDepthRepository);

        let service = MarketDataService::new(trade_repo, depth_repo);

        let ticker_info = FuturesTickerInfo::new(
            FuturesTicker::new("ES.c.0", crate::domain::FuturesVenue::CMEGlobex),
            0.25,
            1.0,
            50.0,
        );

        // Rebuild with tick basis
        let result = service.rebuild_chart_data(&trades, ChartBasis::Tick(2), &ticker_info);

        assert!(result.is_ok());
        let chart_data = result.unwrap();
        assert_eq!(chart_data.trades.len(), 3);
        assert!(!chart_data.candles.is_empty());
    }
}
