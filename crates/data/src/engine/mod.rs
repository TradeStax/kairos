//! DataEngine — primary API facade for all market data operations.
//!
//! Routes requests to adapters (Databento historical, Rithmic live), manages
//! per-day caching via [`CacheStore`], and delivers
//! events through `mpsc::UnboundedReceiver<DataEvent>`.
//!
//! - [`chart`] — `aggregate_to_basis`, `rebuild_chart_data`
//! - [`merger`] — `merge_segments` for combining multi-feed data with dedup and gap detection

pub mod chart;
pub mod merger;

pub use chart::{aggregate_to_basis, rebuild_chart_data};
pub use merger::{MergeOptions, merge_segments, merge_segments_with};

use crate::cache::store::{CacheProvider, CacheSchema, CacheStore};
use crate::connection::types::ConnectionProvider;
use crate::domain::DataIndex;
use crate::domain::FeedId;
use crate::domain::{
    ChartBasis, ChartData, DateRange, Depth, FuturesTicker, FuturesTickerInfo, Price, Trade,
};
use crate::event::DataEvent;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};

/// Main entry point for all market data operations.
///
/// Wraps adapters and provides a unified API:
/// - Cache-first data access (checks cache before fetching)
/// - Adapter routing (Databento historical, Rithmic live + historical)
/// - Event delivery via mpsc channel
/// - Multi-day download with progress reporting
pub struct DataEngine {
    /// Shared cache store for reading/writing per-day data files
    cache: Arc<CacheStore>,
    /// Shared data availability index
    data_index: Arc<Mutex<DataIndex>>,
    /// Event sender for notifying the app layer
    event_tx: mpsc::UnboundedSender<DataEvent>,

    /// Databento adapter for historical CME data
    #[cfg(feature = "databento")]
    databento: Option<crate::adapter::databento::fetcher::DatabentoAdapter>,

    /// Rithmic client for live/historical CME data
    #[cfg(feature = "rithmic")]
    rithmic: Option<std::sync::Arc<tokio::sync::Mutex<crate::adapter::rithmic::RithmicClient>>>,

    /// Maps feed IDs to their provider type
    connections: HashMap<FeedId, ConnectionProvider>,

    /// Optional progress callback invoked during multi-day fetches with
    /// `(days_loaded, days_total)`. Set by the app layer before calling
    /// `get_chart_data`, cleared after completion.
    pub progress_callback: Option<Box<dyn Fn(usize, usize) + Send + Sync>>,
}

impl DataEngine {
    /// Create a new DataEngine.
    ///
    /// Returns the engine and an event receiver. The app subscribes to the
    /// receiver in an Iced subscription to receive `DataEvent`s.
    pub async fn new(
        cache_root: PathBuf,
    ) -> Result<(Self, mpsc::UnboundedReceiver<DataEvent>), crate::Error> {
        let cache = Arc::new(CacheStore::new(cache_root));
        cache.init().await?;

        let (event_tx, event_rx) = mpsc::unbounded_channel();

        let engine = Self {
            cache,
            data_index: Arc::new(Mutex::new(DataIndex::new())),
            event_tx,
            #[cfg(feature = "databento")]
            databento: None,
            #[cfg(feature = "rithmic")]
            rithmic: None,
            connections: HashMap::new(),
            progress_callback: None,
        };

        Ok((engine, event_rx))
    }

    // ── Connection lifecycle ──────────────────────────────────────────

    #[cfg(feature = "databento")]
    pub async fn connect_databento(
        &mut self,
        config: crate::adapter::databento::DatabentoConfig,
    ) -> Result<FeedId, crate::Error> {
        let adapter = crate::adapter::databento::fetcher::DatabentoAdapter::new(config)
            .await
            .map_err(crate::Error::from)?;

        let feed_id = FeedId::new_v4();

        // Scan cache and update index
        let index = adapter.scan_cache(feed_id).await;
        self.data_index.lock().await.merge(index.clone());
        let _ = self.event_tx.send(DataEvent::DataIndexUpdated(index));

        self.databento = Some(adapter);
        self.connections
            .insert(feed_id, ConnectionProvider::Databento);

        let _ = self.event_tx.send(DataEvent::Connected {
            feed_id,
            provider: ConnectionProvider::Databento,
        });

        log::info!("DataEngine: Databento connected (feed_id: {})", feed_id);
        Ok(feed_id)
    }

    /// Disconnects an adapter by feed ID, cleaning up the index and notifying listeners
    pub async fn disconnect(&mut self, feed_id: FeedId) -> Result<(), crate::Error> {
        if let Some(provider) = self.connections.remove(&feed_id) {
            #[cfg(feature = "databento")]
            if provider == ConnectionProvider::Databento {
                self.databento = None;
            }

            #[cfg(feature = "rithmic")]
            if provider == ConnectionProvider::Rithmic {
                self.rithmic = None;
            }

            self.data_index.lock().await.remove_feed(feed_id);

            let _ = self.event_tx.send(DataEvent::Disconnected {
                feed_id,
                reason: "User requested disconnect".to_string(),
            });

            log::info!("DataEngine: disconnected {}", feed_id);
        }
        Ok(())
    }

    // ── Data access ───────────────────────────────────────────────────

    /// Get trades for a date range — routes to the best available adapter.
    ///
    /// Priority: Databento (highest fidelity historical) → Rithmic (tick replay).
    pub async fn get_trades(
        &mut self,
        ticker: &FuturesTicker,
        date_range: &DateRange,
    ) -> Result<Vec<Trade>, crate::Error> {
        let start = date_range.start.and_hms_opt(0, 0, 0).unwrap().and_utc();
        let end = date_range.end.and_hms_opt(23, 59, 59).unwrap().and_utc();

        #[cfg(feature = "databento")]
        if let Some(adapter) = &mut self.databento {
            return adapter
                .get_trades(ticker.as_str(), (start, end))
                .await
                .map_err(crate::Error::from);
        }

        #[cfg(feature = "rithmic")]
        if let Some(rithmic) = self.rithmic.clone() {
            use chrono::Datelike;

            let product = ticker.product().to_string();
            let symbol = ticker.as_str().to_string();
            let total_start = std::time::Instant::now();

            log::info!(
                "Fetching historical trades from Rithmic: {} ({}) \
                 range={}..{}",
                ticker,
                product,
                date_range.start,
                date_range.end,
            );

            // Collect weekday dates for progress reporting
            let weekday_dates: Vec<_> = date_range
                .dates()
                .filter(|d| {
                    let wd = d.weekday();
                    wd != chrono::Weekday::Sat && wd != chrono::Weekday::Sun
                })
                .collect();
            let days_total = weekday_dates.len();

            // ── Partition into cached vs uncached ─────────────────
            // Today is always re-fetched since the trading day is still
            // in progress and the cache file would be stale.
            let today = DateRange::today_et();
            let mut cached_days = Vec::new();
            let mut uncached_days = Vec::new();

            for day in &weekday_dates {
                if *day == today {
                    uncached_days.push(*day);
                } else if self
                    .cache
                    .has_day(CacheProvider::Rithmic, &symbol, CacheSchema::Trades, *day)
                    .await
                {
                    cached_days.push(*day);
                } else {
                    uncached_days.push(*day);
                }
            }

            log::info!(
                "Rithmic {} {}..{}: {} cached, {} to fetch",
                symbol,
                date_range.start,
                date_range.end,
                cached_days.len(),
                uncached_days.len(),
            );

            // ── Read cached days ──────────────────────────────────
            let completed = Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let mut day_results: Vec<(chrono::NaiveDate, Vec<Trade>)> =
                Vec::with_capacity(days_total);

            for day in &cached_days {
                match self
                    .cache
                    .read_day::<Trade>(CacheProvider::Rithmic, &symbol, CacheSchema::Trades, *day)
                    .await
                {
                    Ok(trades) => {
                        log::debug!(
                            "Rithmic cache hit: {} {} ({} trades)",
                            symbol,
                            day,
                            trades.len(),
                        );
                        day_results.push((*day, trades));
                    }
                    Err(e) => {
                        log::warn!(
                            "Rithmic cache read failed for {} {}: {} \
                             — adding to fetch list",
                            symbol,
                            day,
                            e,
                        );
                        uncached_days.push(*day);
                    }
                }

                let done = completed.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                if let Some(ref cb) = self.progress_callback {
                    cb(done, days_total);
                }
            }

            // ── Fetch uncached days in parallel via pool ──────────
            if !uncached_days.is_empty() {
                use futures_util::stream::{FuturesUnordered, StreamExt as _};

                // Each task briefly locks the client to acquire a
                // pool handle, then drops the lock. The pool
                // semaphore bounds actual concurrency.
                let tasks: FuturesUnordered<_> = uncached_days
                    .iter()
                    .map(|day| {
                        let rithmic = rithmic.clone();
                        let product = product.clone();
                        let symbol = symbol.clone();
                        let cache = self.cache.clone();
                        let completed = completed.clone();
                        let day = *day;

                        async move {
                            let guard = rithmic.lock().await;
                            let pool = guard.history_pool().ok_or_else(|| {
                                crate::Error::Connection(
                                    "Rithmic history pool not available".to_string(),
                                )
                            })?;
                            let handle = pool.acquire().await.map_err(crate::Error::from)?;
                            drop(guard);

                            let day_start =
                                day.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp() as i32;
                            let day_end =
                                day.and_hms_opt(23, 59, 59).unwrap().and_utc().timestamp() as i32;

                            let responses = handle
                                .load_ticks(&product, "CME", day_start, day_end)
                                .await
                                .map_err(crate::Error::from)?;

                            let trades = crate::adapter::rithmic::mapper::map_tick_replay_to_trades(
                                &responses,
                            );

                            log::info!(
                                "Rithmic historical {}: {} responses \
                                 → {} trades",
                                day,
                                responses.len(),
                                trades.len(),
                            );

                            // Write to cache (non-fatal)
                            if let Err(e) = cache
                                .write_day(
                                    CacheProvider::Rithmic,
                                    &symbol,
                                    CacheSchema::Trades,
                                    day,
                                    &trades,
                                )
                                .await
                            {
                                log::warn!(
                                    "Failed to cache Rithmic trades \
                                     for {} {}: {}",
                                    symbol,
                                    day,
                                    e,
                                );
                            }

                            let done =
                                completed.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;

                            Ok::<_, crate::Error>((day, trades, done))
                        }
                    })
                    .collect();

                // Collect results — continue on individual failures
                let mut fetch_errors = Vec::new();
                futures_util::pin_mut!(tasks);
                while let Some(result) = tasks.next().await {
                    match result {
                        Ok((day, trades, done)) => {
                            if let Some(ref cb) = self.progress_callback {
                                cb(done, days_total);
                            }
                            day_results.push((day, trades));
                        }
                        Err(e) => {
                            log::warn!("Rithmic fetch failed for a day: {}", e,);
                            fetch_errors.push(e);
                        }
                    }
                }

                if !fetch_errors.is_empty() {
                    if day_results.is_empty() {
                        // No data at all — propagate
                        return Err(fetch_errors.remove(0));
                    }
                    // Some fetched days failed but we have cached data
                    // — proceed with what we have; live stream fills
                    // today.
                    log::warn!(
                        "Rithmic {}: {} fetch(es) failed but {} \
                         cached day(s) available — proceeding \
                         with partial data",
                        symbol,
                        fetch_errors.len(),
                        day_results.len(),
                    );
                }
            }

            // Report fetch complete
            if let Some(ref cb) = self.progress_callback {
                cb(days_total, days_total);
            }

            // Sort by date and flatten
            day_results.sort_by_key(|(day, _)| *day);
            let total_trades: usize = day_results.iter().map(|(_, t)| t.len()).sum();
            let mut all_trades = Vec::with_capacity(total_trades);
            for (_, trades) in day_results {
                all_trades.extend(trades);
            }

            let elapsed = total_start.elapsed();
            log::info!(
                "Rithmic historical total: {} trades across {}..{} \
                 in {:.1}s ({:.0} trades/s)",
                all_trades.len(),
                date_range.start,
                date_range.end,
                elapsed.as_secs_f64(),
                all_trades.len() as f64 / elapsed.as_secs_f64().max(0.001),
            );

            return Ok(all_trades);
        }

        log::warn!(
            "No adapter available for get_trades({}) \
             — no Databento or Rithmic connection",
            ticker
        );
        Ok(Vec::new())
    }

    /// Get depth snapshots for a date range (Databento only)
    pub async fn get_depth(
        &mut self,
        ticker: &FuturesTicker,
        date_range: &DateRange,
    ) -> Result<Vec<Depth>, crate::Error> {
        let start = date_range.start.and_hms_opt(0, 0, 0).unwrap().and_utc();
        let end = date_range.end.and_hms_opt(23, 59, 59).unwrap().and_utc();

        #[cfg(feature = "databento")]
        if let Some(adapter) = &mut self.databento {
            return adapter
                .get_depth(ticker.as_str(), (start, end))
                .await
                .map_err(crate::Error::from);
        }

        log::warn!(
            "No adapter available for get_depth({}) \
             — historical depth requires Databento",
            ticker
        );
        Ok(Vec::new())
    }

    /// Get chart data — fetch trades + aggregate
    pub async fn get_chart_data(
        &mut self,
        ticker: &FuturesTicker,
        basis: ChartBasis,
        date_range: &DateRange,
        ticker_info: &FuturesTickerInfo,
    ) -> Result<ChartData, crate::Error> {
        let trades = self.get_trades(ticker, date_range).await?;

        if trades.is_empty() {
            return Ok(ChartData::from_trades(Vec::new(), Vec::new()));
        }

        let tick_size = Price::from_f32(ticker_info.tick_size);
        let candles =
            chart::aggregate_to_basis(&trades, basis, tick_size).map_err(crate::Error::from)?;

        Ok(ChartData::from_trades(trades, candles))
    }

    /// Rebuild chart data from existing trades (instant — no I/O)
    pub fn rebuild_chart_data(
        &self,
        trades: &[Trade],
        basis: ChartBasis,
        ticker_info: &FuturesTickerInfo,
    ) -> Result<ChartData, crate::Error> {
        chart::rebuild_chart_data(trades, basis, ticker_info).map_err(crate::Error::from)
    }

    // ── Index ─────────────────────────────────────────────────────────

    /// Returns a reference to the shared data index
    pub fn data_index(&self) -> &Arc<Mutex<DataIndex>> {
        &self.data_index
    }

    // ── Cache ─────────────────────────────────────────────────────────

    /// Scans all provider cache directories and returns a merged data index
    pub async fn scan_cache(&self) -> DataIndex {
        let mut index = DataIndex::new();

        #[cfg(feature = "databento")]
        if self.databento.is_some() {
            // Get feed_id for databento
            let feed_id = self
                .connections
                .iter()
                .find(|(_, p)| **p == ConnectionProvider::Databento)
                .map(|(&id, _)| id)
                .unwrap_or_else(FeedId::new_v4);

            let db_index = self
                .cache
                .scan_to_index(CacheProvider::Databento, feed_id)
                .await;
            index.merge(db_index);
        }

        // Scan Rithmic cache
        {
            let feed_id = self
                .connections
                .iter()
                .find(|(_, p)| **p == ConnectionProvider::Rithmic)
                .map(|(&id, _)| id)
                .unwrap_or_else(FeedId::new_v4);

            let rithmic_index = self
                .cache
                .scan_to_index(CacheProvider::Rithmic, feed_id)
                .await;
            index.merge(rithmic_index);
        }

        index
    }

    /// List all cached dates for a ticker across all providers.
    ///
    /// Checks both the DataEngine's CacheStore (Rithmic data) and the
    /// Databento adapter's separate CacheStore.
    pub async fn list_cached_dates(
        &self,
        symbol: &str,
        schema: CacheSchema,
    ) -> std::collections::BTreeSet<chrono::NaiveDate> {
        let mut dates = std::collections::BTreeSet::new();

        // Check DataEngine's own CacheStore (Rithmic data lives here)
        for provider in [CacheProvider::Databento, CacheProvider::Rithmic] {
            dates.extend(self.cache.list_dates(provider, symbol, schema).await);
        }

        // Check Databento adapter's separate CacheStore
        #[cfg(feature = "databento")]
        if let Some(adapter) = &self.databento {
            dates.extend(
                adapter
                    .cache
                    .list_dates(CacheProvider::Databento, symbol, schema)
                    .await,
            );
        }

        dates
    }

    /// Returns aggregate cache statistics (file count, total size, oldest file)
    pub async fn cache_stats(&self) -> crate::cache::CacheStats {
        self.cache.stats().await
    }

    // ── Event access ───────────────────────────────────────────────

    /// Get a clone of the event sender for external use
    pub fn event_sender(&self) -> mpsc::UnboundedSender<DataEvent> {
        self.event_tx.clone()
    }

    // ── Download ───────────────────────────────────────────────────

    /// Download data to cache for a ticker/schema/date range.
    /// Emits DownloadProgress and DownloadComplete events.
    /// Routes to Databento or Rithmic depending on which adapter is
    /// connected.
    pub async fn download_to_cache(
        &mut self,
        ticker: &FuturesTicker,
        _schema: crate::stream::DownloadSchema,
        date_range: &DateRange,
    ) -> Result<usize, crate::Error> {
        let request_id = uuid::Uuid::new_v4();

        #[cfg(feature = "databento")]
        if let Some(adapter) = &mut self.databento {
            let dates: Vec<_> = date_range.dates().collect();
            let total_days = dates.len();

            let start = date_range.start.and_hms_opt(0, 0, 0).unwrap().and_utc();
            let end = date_range.end.and_hms_opt(23, 59, 59).unwrap().and_utc();

            // Emit progress for each day
            for (i, _date) in dates.iter().enumerate() {
                let _ = self.event_tx.send(DataEvent::DownloadProgress {
                    request_id,
                    current_day: i + 1,
                    total_days,
                });
            }

            // Fetch trades (which caches them per-day)
            let trades = adapter
                .get_trades(ticker.as_str(), (start, end))
                .await
                .map_err(crate::Error::from)?;

            let days_cached = total_days;

            // Re-scan cache to update index
            let feed_id = self
                .connections
                .iter()
                .find(|(_, p)| **p == ConnectionProvider::Databento)
                .map(|(&id, _)| id)
                .unwrap_or_else(FeedId::new_v4);

            let index = adapter.scan_cache(feed_id).await;
            self.data_index.lock().await.merge(index.clone());
            let _ = self.event_tx.send(DataEvent::DataIndexUpdated(index));

            let _ = self.event_tx.send(DataEvent::DownloadComplete {
                request_id,
                days_cached,
            });

            return Ok(trades.len());
        }

        // Rithmic download path: wire progress callback to emit
        // DataEvent::DownloadProgress, then delegate to get_trades
        // which handles parallel fetching and caching.
        #[cfg(feature = "rithmic")]
        if self.rithmic.is_some() {
            let event_tx = self.event_tx.clone();
            self.progress_callback = Some(Box::new(move |current, total| {
                let _ = event_tx.send(DataEvent::DownloadProgress {
                    request_id,
                    current_day: current,
                    total_days: total,
                });
            }));

            let trades = self.get_trades(ticker, date_range).await?;
            self.progress_callback = None;

            // Re-scan cache to update index
            let index = self.scan_cache().await;
            self.data_index.lock().await.merge(index.clone());
            let _ = self.event_tx.send(DataEvent::DataIndexUpdated(index));

            let _ = self.event_tx.send(DataEvent::DownloadComplete {
                request_id,
                days_cached: date_range.num_days() as usize,
            });

            return Ok(trades.len());
        }

        Err(crate::Error::Config(
            "No adapter connected for download".to_string(),
        ))
    }

    // ── Rithmic connection ─────────────────────────────────────────

    /// Connect to Rithmic for live streaming data.
    /// Returns the feed_id and a RithmicClient wrapped in
    /// Arc<Mutex<>>.
    #[cfg(feature = "rithmic")]
    pub async fn connect_rithmic(
        &mut self,
        config: crate::adapter::rithmic::RithmicConfig,
        rithmic_conn_config: &crate::adapter::rithmic::protocol::RithmicConnectionConfig,
    ) -> Result<
        (
            FeedId,
            std::sync::Arc<tokio::sync::Mutex<crate::adapter::rithmic::RithmicClient>>,
        ),
        crate::Error,
    > {
        let feed_id = FeedId::new_v4();

        // Create status channel that forwards to our event channel
        let (status_tx, mut status_rx) = tokio::sync::mpsc::unbounded_channel();
        let event_tx = self.event_tx.clone();
        let _status_feed_id = feed_id;

        // Forward status updates to DataEvent channel
        tokio::spawn(async move {
            while let Some(status) = status_rx.recv().await {
                log::debug!("Rithmic status update: {:?}", status);
                // Status forwarding is informational — the main
                // Connected/Disconnected events are sent explicitly
                // below
                let _ = &event_tx;
            }
        });

        let mut client = crate::adapter::rithmic::RithmicClient::new(config, status_tx);

        // Connect with timeout
        tokio::time::timeout(
            std::time::Duration::from_secs(30),
            client.connect(rithmic_conn_config),
        )
        .await
        .map_err(|_| {
            crate::Error::Connection("Rithmic connection timed out after 30s".to_string())
        })?
        .map_err(crate::Error::from)?;

        self.connections
            .insert(feed_id, ConnectionProvider::Rithmic);

        let client = std::sync::Arc::new(tokio::sync::Mutex::new(client));
        self.rithmic = Some(client.clone());

        let _ = self.event_tx.send(DataEvent::Connected {
            feed_id,
            provider: ConnectionProvider::Rithmic,
        });

        // Scan Rithmic cache and merge previously cached data into index
        let rithmic_cache_index = self
            .cache
            .scan_to_index(CacheProvider::Rithmic, feed_id)
            .await;
        let cached_tickers = rithmic_cache_index.available_tickers();
        if !cached_tickers.is_empty() {
            log::info!(
                "DataEngine: found Rithmic cached data for {} tickers",
                cached_tickers.len(),
            );
            self.data_index
                .lock()
                .await
                .merge(rithmic_cache_index.clone());
            let _ = self
                .event_tx
                .send(DataEvent::DataIndexUpdated(rithmic_cache_index));
        }

        log::info!("DataEngine: Rithmic connected (feed_id: {})", feed_id);

        Ok((feed_id, client))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        ChartBasis, FuturesTicker, FuturesTickerInfo, FuturesVenue, Quantity, Side, Timeframe,
        Timestamp,
    };

    #[test]
    fn test_rebuild_chart_data() {
        let trades = vec![
            Trade::new(
                Timestamp(1000),
                crate::domain::Price::from_f32(100.0),
                Quantity(10.0),
                Side::Buy,
            ),
            Trade::new(
                Timestamp(61000),
                crate::domain::Price::from_f32(101.0),
                Quantity(5.0),
                Side::Sell,
            ),
        ];

        let ticker_info = FuturesTickerInfo::new(
            FuturesTicker::new("ES.c.0", FuturesVenue::CMEGlobex),
            0.25,
            1.0,
            50.0,
        );

        // DataEngine::rebuild_chart_data needs an engine instance
        // Test the chart module function directly
        let result = crate::engine::chart::rebuild_chart_data(
            &trades,
            ChartBasis::Time(Timeframe::M1),
            &ticker_info,
        );

        assert!(result.is_ok());
        let chart_data = result.unwrap();
        assert_eq!(chart_data.trades.len(), 2);
        assert_eq!(chart_data.candles.len(), 2);
    }
}
