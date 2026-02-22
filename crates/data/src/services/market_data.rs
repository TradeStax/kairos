//! Market Data Service
//!
//! High-level service for fetching and aggregating market data.
//! Coordinates repositories and applies business logic.

use crate::domain::{
    Candle, DateRange, FuturesTicker, FuturesTickerInfo, Price, Trade,
    aggregation::{AggregationError, aggregate_trades_to_candles, aggregate_trades_to_ticks},
    chart::{ChartBasis, ChartConfig, ChartData, DataSchema, LoadingStatus},
    error::{AppError, ErrorSeverity},
};
use crate::repository::{
    DepthRepository, DownloadRepository, RepositoryError, TradeRepository,
};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::Mutex;

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

impl AppError for ServiceError {
    fn user_message(&self) -> String {
        match self {
            Self::Repository(e) => e.user_message(),
            Self::Aggregation(e) => format!("Data processing error: {e}"),
            Self::InvalidConfig(s) => format!("Invalid configuration: {s}"),
            Self::NoData(s) => format!("No data available: {s}"),
        }
    }

    fn is_retriable(&self) -> bool {
        match self {
            Self::Repository(e) => e.is_retriable(),
            Self::NoData(_) => true,
            _ => false,
        }
    }

    fn severity(&self) -> ErrorSeverity {
        match self {
            Self::Repository(e) => e.severity(),
            Self::Aggregation(_) => ErrorSeverity::Recoverable,
            Self::InvalidConfig(_) => ErrorSeverity::Recoverable,
            Self::NoData(_) => ErrorSeverity::Info,
        }
    }
}

pub type ServiceResult<T> = Result<T, ServiceError>;

/// Estimate of data availability and cost for a requested date range
#[derive(Debug, Clone)]
pub struct DataRequestEstimate {
    pub date_range: crate::domain::types::DateRange,
    pub total_days: usize,
    pub cached_dates: Vec<chrono::NaiveDate>,
    pub uncached_dates: Vec<chrono::NaiveDate>,
    pub uncached_count: usize,
    pub estimated_cost_usd: f64,
}

/// Market data service
///
/// Provides high-level operations for fetching and aggregating market data.
/// This is the primary entry point for chart data in the application.
///
/// ## Example Usage
///
/// ```rust,ignore
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
    download_repo: Option<Arc<dyn DownloadRepository>>,
    loading_status: Arc<Mutex<HashMap<String, LoadingStatus>>>,
}

impl MarketDataService {
    /// Create a new market data service
    pub fn new(trade_repo: Arc<dyn TradeRepository>, depth_repo: Arc<dyn DepthRepository>) -> Self {
        Self {
            trade_repo,
            depth_repo,
            download_repo: None,
            loading_status: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Create a new market data service with download capabilities
    pub fn with_download_repo(
        trade_repo: Arc<dyn TradeRepository>,
        depth_repo: Arc<dyn DepthRepository>,
        download_repo: Arc<dyn DownloadRepository>,
    ) -> Self {
        Self {
            trade_repo,
            depth_repo,
            download_repo: Some(download_repo),
            loading_status: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Update loading status for a chart key
    async fn set_loading_status(&self, key: &str, status: LoadingStatus) {
        let mut map = self.loading_status.lock().await;
        map.insert(key.to_string(), status);
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

        let effective_date_range = config.date_range;

        // Create unique key for this chart configuration
        let chart_key = format!(
            "{}-{:?}-{:?}",
            config.ticker, config.basis, config.date_range
        );

        // Update loading status: start downloading
        self.set_loading_status(
            &chart_key,
            LoadingStatus::Downloading {
                schema: DataSchema::Trades,
                days_total: effective_date_range.num_days() as usize,
                days_complete: 0,
                current_day: effective_date_range.start.to_string(),
            },
        )
        .await;

        // Step 1: Fetch trades from repository WITH PROGRESS CALLBACK
        // Create a progress callback that updates loading_status
        let status_map_clone = Arc::clone(&self.loading_status);
        let chart_key_clone = chart_key.clone();
        let progress_callback: Box<dyn Fn(LoadingStatus) + Send + Sync> =
            Box::new(move |status: LoadingStatus| {
                if let Ok(mut map) = status_map_clone.try_lock() {
                    map.insert(chart_key_clone.clone(), status);
                }
            });

        let trades = match self
            .trade_repo
            .get_trades_with_progress(&config.ticker, &effective_date_range, progress_callback)
            .await
        {
            Ok(trades) => trades,
            Err(e) => {
                self.set_loading_status(
                    &chart_key,
                    LoadingStatus::Error {
                        message: format!("Failed to fetch trades: {:?}", e),
                    },
                )
                .await;
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
        self.set_loading_status(
            &chart_key,
            LoadingStatus::Building {
                operation: format!("Aggregating {} trades", trades.len()),
                progress: 0.3,
            },
        )
        .await;

        // Step 2: Aggregate to target basis
        let candles = match self.aggregate_to_basis(&trades, config.basis, ticker_info) {
            Ok(candles) => candles,
            Err(e) => {
                self.set_loading_status(
                    &chart_key,
                    LoadingStatus::Error {
                        message: format!("Failed to aggregate: {:?}", e),
                    },
                )
                .await;
                return Err(e);
            }
        };

        log::info!(
            "Aggregated to {} candles ({:?})",
            candles.len(),
            config.basis
        );

        // Update status: building candles complete
        self.set_loading_status(
            &chart_key,
            LoadingStatus::Building {
                operation: format!("Built {} candles", candles.len()),
                progress: 0.6,
            },
        )
        .await;

        // Step 3: Load depth data for heatmap charts
        let mut chart_data = ChartData::from_trades(trades, candles);

        if config.chart_type == crate::domain::ChartType::Heatmap {
            log::info!(
                "Heatmap chart detected - loading MBP-10 depth data for {} ({} to {})",
                config.ticker,
                effective_date_range.start,
                effective_date_range.end
            );

            // Update status: loading depth data
            self.set_loading_status(
                &chart_key,
                LoadingStatus::Downloading {
                    schema: DataSchema::MBP10,
                    days_total: effective_date_range.num_days() as usize,
                    days_complete: 0,
                    current_day: effective_date_range.start.to_string(),
                },
            )
            .await;

            let depth_start = std::time::Instant::now();
            match self
                .depth_repo
                .get_depth(&config.ticker, &effective_date_range)
                .await
            {
                Ok(depth_snapshots) => {
                    log::info!(
                        "Loaded {} depth snapshots for heatmap in {:.2}s",
                        depth_snapshots.len(),
                        depth_start.elapsed().as_secs_f32()
                    );

                    // Update status: processing depth data
                    self.set_loading_status(
                        &chart_key,
                        LoadingStatus::Building {
                            operation: format!(
                                "Processing {} depth snapshots",
                                depth_snapshots.len()
                            ),
                            progress: 0.9,
                        },
                    )
                    .await;

                    chart_data = chart_data.with_depth(depth_snapshots);
                }
                Err(e) => {
                    log::warn!(
                        "Failed to load depth data for heatmap: {:?}. Chart will show trades only.",
                        e
                    );
                    // Don't fail the entire load - heatmap can still show trades
                }
            }
        }

        // Update status: ready
        self.set_loading_status(&chart_key, LoadingStatus::Ready).await;

        // Step 4: Return chart data with depth (if heatmap)
        Ok(chart_data)
    }

    /// Rebuild chart data with a new basis (INSTANT - uses trades in memory)
    ///
    /// This enables instant basis switching without refetching data.
    /// Takes existing trades and aggregates to new basis in <100ms.
    ///
    /// ## Example
    /// ```rust,ignore
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
    pub async fn get_loading_status(&self, config: &ChartConfig) -> LoadingStatus {
        let chart_key = format!(
            "{}-{:?}-{:?}",
            config.ticker, config.basis, config.date_range
        );

        let status_map = self.loading_status.lock().await;
        status_map
            .get(&chart_key)
            .cloned()
            .unwrap_or(LoadingStatus::Idle)
    }

    /// Get all loading statuses
    ///
    /// Returns all ongoing operations across all charts.
    pub async fn get_all_loading_statuses(&self) -> HashMap<String, LoadingStatus> {
        let status_map = self.loading_status.lock().await;
        status_map.clone()
    }

    /// Clear completed and errored loading statuses
    ///
    /// Removes Ready, Idle, and Error statuses, keeping only active operations.
    pub async fn clear_old_statuses(&self) {
        let mut status_map = self.loading_status.lock().await;

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
    ///
    /// Requires a `DownloadRepository` to be configured via `with_download_repo`.
    pub async fn estimate_data_request(
        &self,
        ticker: &FuturesTicker,
        schema_discriminant: u16,
        date_range: &DateRange,
    ) -> ServiceResult<DataRequestEstimate> {
        let download_repo = self.download_repo.as_ref().ok_or_else(|| {
            ServiceError::InvalidConfig("Download repository not configured".into())
        })?;

        log::info!(
            "Estimating cost for {} ({}) from {} to {}",
            ticker,
            schema_discriminant,
            date_range.start,
            date_range.end
        );

        // Get cache coverage
        let coverage = download_repo
            .check_cache_coverage(ticker, schema_discriminant, date_range)
            .await?;

        log::debug!(
            "Cache coverage: {} cached, {} uncached out of {} total days",
            coverage.cached_count,
            coverage.uncached_count,
            date_range.num_days()
        );

        let total_days = date_range.num_days() as usize;
        let uncached_count = coverage.uncached_count;

        // Derive uncached dates from date range minus cached dates
        let uncached_dates: Vec<chrono::NaiveDate> = date_range
            .dates()
            .filter(|d| !coverage.cached_dates.contains(d))
            .collect();

        // Get cost for FULL range
        let full_range_cost = download_repo
            .get_download_cost(ticker, schema_discriminant, date_range)
            .await?;

        log::info!(
            "Cost API: ${:.4} USD for full range ({} days)",
            full_range_cost,
            total_days
        );

        // Calculate ACTUAL download cost (only for uncached days)
        let estimated_cost_usd = if total_days > 0 && uncached_count > 0 {
            let cost_per_day = full_range_cost / total_days as f64;
            let cost = cost_per_day * uncached_count as f64;
            log::info!(
                "Actual cost for {} uncached days: ${:.4} (${:.4}/day)",
                uncached_count,
                cost,
                cost_per_day
            );
            cost
        } else {
            0.0
        };

        Ok(DataRequestEstimate {
            date_range: *date_range,
            total_days,
            cached_dates: coverage.cached_dates,
            uncached_dates,
            uncached_count,
            estimated_cost_usd,
        })
    }

    /// Download data to cache without loading into memory
    ///
    /// Returns number of days successfully downloaded.
    pub async fn download_to_cache(
        &self,
        ticker: &FuturesTicker,
        schema_discriminant: u16,
        date_range: &DateRange,
    ) -> ServiceResult<usize> {
        let download_repo = self.download_repo.as_ref().ok_or_else(|| {
            ServiceError::InvalidConfig("Download repository not configured".into())
        })?;

        let days_downloaded = download_repo
            .prefetch_to_cache(ticker, schema_discriminant, date_range)
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
        let download_repo = self.download_repo.as_ref().ok_or_else(|| {
            ServiceError::InvalidConfig("Download repository not configured".into())
        })?;

        let days_downloaded = download_repo
            .prefetch_to_cache_with_progress(
                ticker,
                schema_discriminant,
                date_range,
                progress_callback,
            )
            .await?;

        Ok(days_downloaded)
    }

    /// Get trades from cache for preview (no remote fetch)
    pub async fn get_trades_for_preview(
        &self,
        ticker: &FuturesTicker,
        date_range: &DateRange,
    ) -> ServiceResult<Vec<Trade>> {
        self.trade_repo
            .get_trades(ticker, date_range)
            .await
            .map_err(ServiceError::from)
    }

    /// Get list of tickers with cached data
    pub async fn get_cached_tickers(&self) -> ServiceResult<std::collections::HashSet<String>> {
        let download_repo = self.download_repo.as_ref().ok_or_else(|| {
            ServiceError::InvalidConfig("Download repository not configured".into())
        })?;

        download_repo
            .list_cached_symbols()
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
