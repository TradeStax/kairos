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
    /// `(days_loaded_frac, days_total)`. The first argument is a float
    /// that increments by sub-day fractions during paginated Rithmic
    /// fetches, giving smooth intra-day progress.
    ///
    /// **Precision note**: The `f32` first argument loses integer precision
    /// beyond ~16 million days (2^24). This is not a concern in practice
    /// since download ranges are at most a few thousand days, but callers
    /// should not rely on exact equality comparisons on the progress value.
    ///
    /// Use [`set_progress_callback`](Self::set_progress_callback) and
    /// [`clear_progress_callback`](Self::clear_progress_callback) to manage.
    pub(crate) progress_callback: Option<Arc<dyn Fn(f32, usize) + Send + Sync>>,
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

    /// Returns `true` if the Databento adapter is initialized.
    #[cfg(feature = "databento")]
    pub fn has_databento(&self) -> bool {
        self.databento.is_some()
    }

    /// Drops the Databento adapter so the next call to `connect_databento`
    /// (or `ensure_databento_adapter`) will reinitialize it with a fresh key.
    #[cfg(feature = "databento")]
    pub fn disconnect_databento(&mut self) {
        self.databento = None;
        log::info!("DataEngine: Databento adapter disconnected");
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
    /// When `provider` is `None`, uses default priority:
    /// Databento (highest fidelity historical) → Rithmic (tick replay).
    /// When `provider` is `Some(p)`, only uses that specific adapter.
    pub async fn get_trades(
        &mut self,
        ticker: &FuturesTicker,
        date_range: &DateRange,
        provider: Option<ConnectionProvider>,
    ) -> Result<Vec<Trade>, crate::Error> {
        let start = date_range
            .start
            .and_hms_opt(0, 0, 0)
            .expect("midnight is always valid")
            .and_utc();
        let end = date_range
            .end
            .and_hms_opt(23, 59, 59)
            .expect("23:59:59 is always valid")
            .and_utc();

        #[cfg(feature = "databento")]
        if provider != Some(ConnectionProvider::Rithmic)
            && let Some(adapter) = &mut self.databento
        {
            return adapter
                .get_trades(ticker.as_str(), (start, end))
                .await
                .map_err(crate::Error::from);
        }

        #[cfg(feature = "rithmic")]
        if provider != Some(ConnectionProvider::Databento)
            && let Some(rithmic) = self.rithmic.clone()
        {
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
                    cb(done as f32, days_total);
                }
            }

            // ── Fetch uncached days in parallel via pool ──────────
            if !uncached_days.is_empty() {
                use futures_util::stream::{FuturesUnordered, StreamExt as _};

                // Each task briefly locks the client to acquire a
                // pool handle, then drops the lock. The pool
                // semaphore bounds actual concurrency.
                let progress_cb = self.progress_callback.clone();

                let tasks: FuturesUnordered<_> = uncached_days
                    .iter()
                    .map(|day| {
                        let rithmic = rithmic.clone();
                        let product = product.clone();
                        let symbol = symbol.clone();
                        let cache = self.cache.clone();
                        let completed = completed.clone();
                        let progress_cb = progress_cb.clone();
                        let day = *day;
                        let is_today = day == today;

                        async move {
                            let guard = rithmic.lock().await;
                            let pool = guard.history_pool().ok_or_else(|| {
                                crate::Error::Connection(
                                    "Rithmic history pool not available".to_string(),
                                )
                            })?;
                            let handle = pool.acquire().await.map_err(crate::Error::from)?;
                            drop(guard);

                            let day_start_ts = day
                                .and_hms_opt(0, 0, 0)
                                .expect("midnight is always valid")
                                .and_utc()
                                .timestamp();
                            let day_start = i32::try_from(day_start_ts).unwrap_or(i32::MAX);
                            // For today's date, extend to now_utc to cover
                            // the UTC/ET offset (ET lags UTC by 4-5h, so
                            // trades after midnight UTC are still part of
                            // "today" in ET but would be missed with a
                            // strict end-of-day UTC cutoff).
                            let day_end = if is_today {
                                let ts = chrono::Utc::now().timestamp();
                                i32::try_from(ts).unwrap_or(i32::MAX)
                            } else {
                                let ts = day
                                    .and_hms_opt(23, 59, 59)
                                    .expect("23:59:59 is always valid")
                                    .and_utc()
                                    .timestamp();
                                i32::try_from(ts).unwrap_or(i32::MAX)
                            };

                            // Build per-page progress callback for smooth
                            // sub-day reporting. Uses response count for
                            // asymptotic progress within each day.
                            let on_page: Option<Box<dyn Fn(usize, usize) + Send + Sync>> =
                                progress_cb.as_ref().map(|cb| {
                                    let cb = cb.clone();
                                    let completed = completed.clone();
                                    Box::new(move |_page: usize, responses: usize| {
                                        let base = completed
                                            .load(std::sync::atomic::Ordering::Relaxed)
                                            as f32;
                                        // Asymptotic: r/(r+50000) gives smooth
                                        // progress that approaches but never
                                        // reaches 1.0 before the day completes.
                                        // 50K→0.50, 100K→0.67, 200K→0.80,
                                        // 500K→0.91
                                        let frac = (responses as f32
                                            / (responses as f32 + 50_000.0))
                                            .min(0.95);
                                        cb(base + frac, days_total);
                                    })
                                        as Box<dyn Fn(usize, usize) + Send + Sync>
                                });

                            let responses = handle
                                .load_ticks_with_progress(
                                    &product,
                                    "CME",
                                    day_start,
                                    day_end,
                                    on_page.as_deref(),
                                )
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
                                cb(done as f32, days_total);
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
                        return Err(fetch_errors.swap_remove(0));
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
                cb(days_total as f32, days_total);
            }

            // Sort by date and flatten
            day_results.sort_by_key(|(day, _)| *day);
            let total_trades: usize = day_results.iter().map(|(_, t)| t.len()).sum();
            let mut all_trades = Vec::with_capacity(total_trades);
            for (_, trades) in day_results {
                all_trades.extend(trades);
            }
            // Ensure global sort — Rithmic tick replay doesn't guarantee
            // intra-day order and CME sessions span UTC day boundaries.
            // Timsort is O(n) on nearly-sorted data.
            all_trades.sort_by_key(|t| t.time.0);

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
        let start = date_range
            .start
            .and_hms_opt(0, 0, 0)
            .expect("midnight is always valid")
            .and_utc();
        let end = date_range
            .end
            .and_hms_opt(23, 59, 59)
            .expect("23:59:59 is always valid")
            .and_utc();

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

    /// Get chart data — fetch trades + aggregate.
    ///
    /// When `provider` is `Some(p)`, routes exclusively to that adapter.
    /// When `None`, uses default priority (Databento → Rithmic).
    pub async fn get_chart_data(
        &mut self,
        ticker: &FuturesTicker,
        basis: ChartBasis,
        date_range: &DateRange,
        ticker_info: &FuturesTickerInfo,
        provider: Option<ConnectionProvider>,
    ) -> Result<ChartData, crate::Error> {
        let trades = self.get_trades(ticker, date_range, provider).await?;

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

    /// Returns a reference to the engine's data index.
    ///
    /// **Note**: The app layer maintains a separate `std::sync::Mutex<DataIndex>`
    /// for synchronous access in Iced's view/update cycle. The engine's
    /// `tokio::sync::Mutex<DataIndex>` is used in async adapter operations.
    /// These are kept in sync via `DataEvent::DataIndexUpdated` events emitted
    /// after cache scans and downloads. Do not bypass this sync mechanism.
    pub fn data_index(&self) -> &Arc<Mutex<DataIndex>> {
        &self.data_index
    }

    // ── Cost estimation ─────────────────────────────────────────────

    /// Estimate the Databento API cost (in USD) for downloading the given
    /// symbol / schema / date range.  Returns `Err` if the Databento
    /// adapter is not connected.
    #[cfg(feature = "databento")]
    pub async fn estimate_cost(
        &mut self,
        symbol: &str,
        schema: crate::stream::DownloadSchema,
        date_range: &DateRange,
    ) -> Result<f64, crate::Error> {
        let adapter = self
            .databento
            .as_mut()
            .ok_or_else(|| crate::Error::Config("Databento adapter not connected".to_string()))?;

        let start = date_range
            .start
            .and_hms_opt(0, 0, 0)
            .expect("midnight is always valid")
            .and_utc();
        let end = date_range
            .end
            .and_hms_opt(23, 59, 59)
            .expect("23:59:59 is always valid")
            .and_utc();

        let db_schema = schema.to_databento_schema();
        adapter
            .get_cost(symbol, db_schema, start, end)
            .await
            .map_err(crate::Error::from)
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

        // Scan Rithmic cache (only if connected — prevents orphaned entries
        // with random feed IDs from accumulating)
        #[cfg(feature = "rithmic")]
        if self.rithmic.is_some() {
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

    /// Returns a cloned reference to the internal CacheStore.
    ///
    /// Prefer using the specific DataEngine methods (e.g.,
    /// [`list_cached_dates`], [`cache_stats`]) where possible.
    /// This accessor exists for the cache-management UI that needs
    /// full provider/symbol/schema enumeration.
    pub fn cache_store(&self) -> Arc<CacheStore> {
        self.cache.clone()
    }

    /// Lists all symbols found in the cache across all providers.
    pub async fn list_cached_symbols(&self) -> Vec<(CacheProvider, String)> {
        let mut result = Vec::new();
        for provider in [CacheProvider::Databento, CacheProvider::Rithmic] {
            for symbol in self.cache.list_symbols(provider).await {
                result.push((provider, symbol));
            }
        }
        result
    }

    // ── Event access ───────────────────────────────────────────────

    /// Set a progress callback for multi-day fetches.
    ///
    /// The callback receives `(days_loaded_frac, days_total)`.
    pub fn set_progress_callback(&mut self, cb: Arc<dyn Fn(f32, usize) + Send + Sync>) {
        self.progress_callback = Some(cb);
    }

    /// Clear the progress callback.
    pub fn clear_progress_callback(&mut self) {
        self.progress_callback = None;
    }

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
            let start = date_range
                .start
                .and_hms_opt(0, 0, 0)
                .expect("midnight is always valid")
                .and_utc();
            let end = date_range
                .end
                .and_hms_opt(23, 59, 59)
                .expect("23:59:59 is always valid")
                .and_utc();

            let event_tx = self.event_tx.clone();
            adapter
                .get_trades_with_progress(
                    ticker.as_str(),
                    (start, end),
                    |current_day, total_days, _date, _cached| {
                        let _ = event_tx.send(DataEvent::DownloadProgress {
                            request_id,
                            current_day,
                            total_days,
                            sub_day_fraction: 0.0,
                        });
                    },
                )
                .await
                .map_err(crate::Error::from)?;

            let days_cached = date_range.num_days() as usize;

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

            return Ok(days_cached);
        }

        // Rithmic download path: wire progress callback to emit
        // DataEvent::DownloadProgress, then delegate to get_trades
        // which handles parallel fetching and caching.
        #[cfg(feature = "rithmic")]
        if self.rithmic.is_some() {
            let event_tx = self.event_tx.clone();
            self.progress_callback = Some(Arc::new(move |current: f32, total: usize| {
                let _ = event_tx.send(DataEvent::DownloadProgress {
                    request_id,
                    current_day: current.floor() as usize,
                    total_days: total,
                    sub_day_fraction: current.fract(),
                });
            }));

            self.get_trades(ticker, date_range, None).await?;
            self.progress_callback = None;

            // Re-scan cache to update index
            let index = self.scan_cache().await;
            self.data_index.lock().await.merge(index.clone());
            let _ = self.event_tx.send(DataEvent::DataIndexUpdated(index));

            let days_cached = date_range.num_days() as usize;
            let _ = self.event_tx.send(DataEvent::DownloadComplete {
                request_id,
                days_cached,
            });

            return Ok(days_cached);
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
            DataIndex,
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

        // Scan Rithmic cache and merge previously cached data into
        // the engine's internal index.  The cache index is also returned
        // so the app layer can merge it synchronously — avoids a race
        // where chart ranges are resolved before the async
        // DataIndexUpdated event arrives.
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
        }

        log::info!("DataEngine: Rithmic connected (feed_id: {})", feed_id);

        Ok((feed_id, client, rithmic_cache_index))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        ChartBasis, FuturesTicker, FuturesTickerInfo, FuturesVenue, Quantity, Side, Timeframe,
        Timestamp,
    };

    fn es_ticker_info() -> FuturesTickerInfo {
        FuturesTickerInfo::new(
            FuturesTicker::new("ES.c.0", FuturesVenue::CMEGlobex),
            0.25,
            1.0,
            50.0,
        )
    }

    #[test]
    fn test_rebuild_chart_data() {
        let trades = vec![
            Trade::new(
                Timestamp(1000),
                Price::from_f32(100.0),
                Quantity(10.0),
                Side::Buy,
            ),
            Trade::new(
                Timestamp(61000),
                Price::from_f32(101.0),
                Quantity(5.0),
                Side::Sell,
            ),
        ];

        let result = crate::engine::chart::rebuild_chart_data(
            &trades,
            ChartBasis::Time(Timeframe::M1),
            &es_ticker_info(),
        );

        assert!(result.is_ok());
        let chart_data = result.unwrap();
        assert_eq!(chart_data.trades.len(), 2);
        assert_eq!(chart_data.candles.len(), 2);

        // Verify OHLCV values of first candle (single trade at 100.0)
        let c0 = &chart_data.candles[0];
        assert_eq!(c0.open.to_f32(), 100.0);
        assert_eq!(c0.high.to_f32(), 100.0);
        assert_eq!(c0.low.to_f32(), 100.0);
        assert_eq!(c0.close.to_f32(), 100.0);
        assert!((c0.buy_volume.value() - 10.0).abs() < 0.01);
        assert!((c0.sell_volume.value()).abs() < 0.01);

        // Verify second candle
        let c1 = &chart_data.candles[1];
        assert_eq!(c1.open.to_f32(), 101.0);
        assert_eq!(c1.close.to_f32(), 101.0);
        assert!((c1.sell_volume.value() - 5.0).abs() < 0.01);
    }

    #[test]
    fn rebuild_chart_data_empty_trades() {
        let result = crate::engine::chart::rebuild_chart_data(
            &[],
            ChartBasis::Time(Timeframe::M1),
            &es_ticker_info(),
        );
        assert!(result.is_ok());
        let chart_data = result.unwrap();
        assert!(chart_data.trades.is_empty());
        assert!(chart_data.candles.is_empty());
    }

    #[test]
    fn rebuild_chart_data_tick_basis_with_ohlcv_check() {
        let trades = vec![
            Trade::new(
                Timestamp(1000),
                Price::from_f32(100.0),
                Quantity(5.0),
                Side::Buy,
            ),
            Trade::new(
                Timestamp(2000),
                Price::from_f32(102.0),
                Quantity(3.0),
                Side::Sell,
            ),
            Trade::new(
                Timestamp(3000),
                Price::from_f32(98.0),
                Quantity(7.0),
                Side::Sell,
            ),
            Trade::new(
                Timestamp(4000),
                Price::from_f32(101.0),
                Quantity(2.0),
                Side::Buy,
            ),
        ];

        let result = crate::engine::chart::rebuild_chart_data(
            &trades,
            ChartBasis::Tick(2),
            &es_ticker_info(),
        );
        assert!(result.is_ok());
        let chart_data = result.unwrap();
        assert_eq!(chart_data.candles.len(), 2);

        // Candle 0: trades at 100.0, 102.0
        let c0 = &chart_data.candles[0];
        assert_eq!(c0.open.to_f32(), 100.0);
        assert_eq!(c0.high.to_f32(), 102.0);
        assert_eq!(c0.low.to_f32(), 100.0);
        assert_eq!(c0.close.to_f32(), 102.0);
        assert!((c0.buy_volume.value() - 5.0).abs() < 0.01);
        assert!((c0.sell_volume.value() - 3.0).abs() < 0.01);

        // Candle 1: trades at 98.0, 101.0
        let c1 = &chart_data.candles[1];
        assert_eq!(c1.open.to_f32(), 98.0);
        assert_eq!(c1.high.to_f32(), 101.0);
        assert_eq!(c1.low.to_f32(), 98.0);
        assert_eq!(c1.close.to_f32(), 101.0);
        assert!((c1.buy_volume.value() - 2.0).abs() < 0.01);
        assert!((c1.sell_volume.value() - 7.0).abs() < 0.01);
    }

    #[test]
    fn aggregate_to_basis_time_groups_by_interval() {
        let trades = vec![
            Trade::new(
                Timestamp(0),
                Price::from_f32(50.0),
                Quantity(1.0),
                Side::Buy,
            ),
            Trade::new(
                Timestamp(30_000),
                Price::from_f32(52.0),
                Quantity(2.0),
                Side::Sell,
            ),
            Trade::new(
                Timestamp(60_000),
                Price::from_f32(51.0),
                Quantity(3.0),
                Side::Buy,
            ),
            Trade::new(
                Timestamp(90_000),
                Price::from_f32(53.0),
                Quantity(4.0),
                Side::Buy,
            ),
            Trade::new(
                Timestamp(120_000),
                Price::from_f32(54.0),
                Quantity(5.0),
                Side::Sell,
            ),
        ];

        let tick_size = Price::from_f32(0.25);
        let candles =
            chart::aggregate_to_basis(&trades, ChartBasis::Time(Timeframe::M1), tick_size).unwrap();

        // 0-59999 = minute 0, 60000-119999 = minute 1, 120000+ = minute 2
        assert_eq!(candles.len(), 3);

        // Minute 0: 50, 52 -> O=50, H=52, L=50, C=52
        assert_eq!(candles[0].open.to_f32(), 50.0);
        assert_eq!(candles[0].high.to_f32(), 52.0);
        assert_eq!(candles[0].low.to_f32(), 50.0);
        assert_eq!(candles[0].close.to_f32(), 52.0);
    }

    #[test]
    fn single_trade_produces_single_candle_time_basis() {
        let trades = vec![Trade::new(
            Timestamp(5000),
            Price::from_f32(4500.25),
            Quantity(10.0),
            Side::Buy,
        )];
        let result = crate::engine::chart::rebuild_chart_data(
            &trades,
            ChartBasis::Time(Timeframe::M1),
            &es_ticker_info(),
        );
        assert!(result.is_ok());
        let chart_data = result.unwrap();
        assert_eq!(chart_data.candles.len(), 1);
        let c = &chart_data.candles[0];
        assert_eq!(c.open.to_f32(), 4500.25);
        assert_eq!(c.high.to_f32(), 4500.25);
        assert_eq!(c.low.to_f32(), 4500.25);
        assert_eq!(c.close.to_f32(), 4500.25);
    }
}
