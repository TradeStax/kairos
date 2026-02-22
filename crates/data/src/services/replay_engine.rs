//! Replay Engine Service
//!
//! Core service that orchestrates historical data replay.
//! Manages loading, buffering, and controlled emission of market data.

use crate::domain::aggregation::{aggregate_trades_to_candles, aggregate_trades_to_ticks};
use crate::domain::chart::ChartBasis;
use crate::domain::futures::{FuturesTicker, FuturesTickerInfo};
use crate::domain::{Candle, DateRange, DepthSnapshot, Price, Side, TimeRange, Trade};
use crate::repository::{DepthRepository, TradeRepository};
use crate::state::replay::{PlaybackStatus, ReplayData, ReplayState, SpeedPreset};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::sync::mpsc;

/// A single time bucket of aggregated volume for the trackbar histogram.
#[derive(Debug, Clone, Default)]
pub struct VolumeBucket {
    pub buy_volume: f32,
    pub sell_volume: f32,
}

/// Events emitted by the replay engine
#[derive(Debug, Clone)]
pub enum ReplayEvent {
    /// Data loaded and ready for playback
    DataLoaded {
        ticker: FuturesTicker,
        trade_count: usize,
        depth_count: usize,
        time_range: TimeRange,
    },

    /// Loading progress update
    LoadingProgress { progress: f32, message: String },

    /// Error during loading or playback
    Error(String),

    /// Playback position updated
    PositionUpdate { timestamp: u64, progress: f32 },

    /// Market data for current time window
    MarketData {
        timestamp: u64,
        trades: Vec<Trade>,
        depth: Option<DepthSnapshot>,
    },

    /// Playback status changed
    StatusChanged(PlaybackStatus),

    /// Playback reached end
    PlaybackComplete,

    /// Playback started
    PlaybackStarted,

    /// Playback paused
    PlaybackPaused,

    /// Playback stopped
    PlaybackStopped,

    /// Seek completed
    SeekCompleted { timestamp: u64, progress: f32 },

    /// Full chart rebuild — contains ALL trades from [start, current_position].
    /// The UI should clear the chart and rebuild from these trades.
    ChartRebuild { trades: Vec<Trade> },

    /// Speed changed
    SpeedChanged(SpeedPreset),

    /// Cache hit (data loaded from cache)
    CacheHit {
        symbol: String,
        date_range: DateRange,
    },

    /// Memory usage update
    MemoryUsage {
        bytes: usize,
        trades: usize,
        depth_snapshots: usize,
    },
}

/// Configuration for replay engine
#[derive(Debug, Clone)]
pub struct ReplayEngineConfig {
    /// Buffer size for pre-loading data (milliseconds)
    pub buffer_window_ms: u64,

    /// Emit interval for market data (milliseconds)
    pub emit_interval_ms: u64,

    /// Maximum trades per emission
    pub max_trades_per_emit: usize,

    /// Enable depth data loading
    pub load_depth: bool,

    /// Pre-aggregate common timeframes on load
    pub pre_aggregate: bool,

    /// Event channel buffer size
    pub event_buffer_size: usize,

    /// Enable detailed progress events
    pub detailed_progress: bool,

    /// Memory limit for loaded data (MB)
    pub max_memory_mb: usize,
}

impl Default for ReplayEngineConfig {
    fn default() -> Self {
        Self {
            buffer_window_ms: 60000,   // 1 minute buffer
            emit_interval_ms: 100,     // Emit every 100ms
            max_trades_per_emit: 1000, // Max 1000 trades per emit
            load_depth: true,          // Load depth by default
            pre_aggregate: true,       // Pre-aggregate common timeframes
            event_buffer_size: 1000,   // Event channel buffer
            detailed_progress: true,   // Emit detailed progress
            max_memory_mb: 500,        // 500 MB memory limit
        }
    }
}

/// Replay Engine with Complete Event System
pub struct ReplayEngine {
    /// Configuration
    config: ReplayEngineConfig,

    /// Current replay state
    state: Arc<RwLock<ReplayState>>,

    /// Loaded replay data
    data: Arc<RwLock<Option<ReplayData>>>,

    /// Trade repository
    trade_repo: Arc<dyn TradeRepository + Send + Sync>,

    /// Depth repository (optional)
    depth_repo: Option<Arc<dyn DepthRepository + Send + Sync>>,

    /// Event sender channel (bounded to prevent memory growth under heavy load)
    event_tx: mpsc::Sender<ReplayEvent>,

    /// Event receiver channel (private; use take_event_rx() for external access)
    event_rx: Option<mpsc::Receiver<ReplayEvent>>,

    /// Background playback task handle
    playback_handle: Option<tokio::task::JoinHandle<()>>,
}

impl ReplayEngine {
    /// Create new Replay engine
    pub fn new(
        config: ReplayEngineConfig,
        trade_repo: Arc<dyn TradeRepository + Send + Sync>,
        depth_repo: Option<Arc<dyn DepthRepository + Send + Sync>>,
    ) -> Self {
        // Bounded channel prevents unbounded memory growth during heavy replay
        let (event_tx, event_rx) = mpsc::channel(1024);

        Self {
            config,
            state: Arc::new(RwLock::new(ReplayState::new())),
            data: Arc::new(RwLock::new(None)),
            trade_repo,
            depth_repo,
            event_tx,
            event_rx: Some(event_rx),
            playback_handle: None,
        }
    }

    /// Create with default configuration
    pub fn with_default_config(
        trade_repo: Arc<dyn TradeRepository + Send + Sync>,
        depth_repo: Option<Arc<dyn DepthRepository + Send + Sync>>,
    ) -> Self {
        Self::new(ReplayEngineConfig::default(), trade_repo, depth_repo)
    }

    /// Load data for replay
    pub async fn load_data(
        &mut self,
        ticker_info: FuturesTickerInfo,
        date_range: DateRange,
    ) -> Result<(), String> {
        // Clear any existing data
        {
            let mut state = self.state.write().await;
            state.clear();
        }
        {
            let mut data = self.data.write().await;
            *data = None;
        }

        // Emit initial loading progress
        self.emit_event(ReplayEvent::LoadingProgress {
            progress: 0.0,
            message: format!(
                "Loading {} from {} to {}",
                ticker_info.ticker, date_range.start, date_range.end
            ),
        });

        // Check cache first
        if self.check_cache(&ticker_info.ticker, &date_range).await {
            self.emit_event(ReplayEvent::CacheHit {
                symbol: ticker_info.ticker.to_string(),
                date_range,
            });
        }

        // Fetch trades
        self.emit_event(ReplayEvent::LoadingProgress {
            progress: 0.2,
            message: "Fetching trades...".to_string(),
        });

        let trades = self
            .trade_repo
            .get_trades(&ticker_info.ticker, &date_range)
            .await
            .map_err(|e| {
                let error = format!("Failed to load trades: {:?}", e);
                self.emit_event(ReplayEvent::Error(error.clone()));
                error
            })?;

        if trades.is_empty() {
            let error = "No trades found for the specified date range".to_string();
            self.emit_event(ReplayEvent::Error(error.clone()));
            return Err(error);
        }

        // Emit progress
        self.emit_event(ReplayEvent::LoadingProgress {
            progress: 0.5,
            message: format!("Loaded {} trades", trades.len()),
        });

        // Fetch depth if enabled
        let depth_snapshots = if self.config.load_depth {
            self.emit_event(ReplayEvent::LoadingProgress {
                progress: 0.6,
                message: "Fetching depth data...".to_string(),
            });

            if let Some(depth_repo) = &self.depth_repo {
                depth_repo
                    .get_depth(&ticker_info.ticker, &date_range)
                    .await
                    .unwrap_or_default()
            } else {
                vec![]
            }
        } else {
            vec![]
        };

        // Emit progress
        self.emit_event(ReplayEvent::LoadingProgress {
            progress: 0.8,
            message: format!("Loaded {} depth snapshots", depth_snapshots.len()),
        });

        // Create replay data
        let mut replay_data = ReplayData::new(trades, depth_snapshots, ticker_info);

        // Pre-aggregate common timeframes if configured
        if self.config.pre_aggregate {
            self.emit_event(ReplayEvent::LoadingProgress {
                progress: 0.9,
                message: "Pre-aggregating candles...".to_string(),
            });

            self.pre_aggregate_candles(&mut replay_data, ticker_info.tick_size);
        }

        // Check memory usage
        let memory_usage = replay_data.memory_usage();
        let memory_limit = self.config.max_memory_mb * 1024 * 1024;

        if memory_usage > memory_limit {
            let error = format!(
                "Data too large: {} MB exceeds limit of {} MB",
                memory_usage / (1024 * 1024),
                self.config.max_memory_mb
            );
            self.emit_event(ReplayEvent::Error(error.clone()));
            return Err(error);
        }

        // Emit memory usage
        self.emit_event(ReplayEvent::MemoryUsage {
            bytes: memory_usage,
            trades: replay_data.stats().trade_count,
            depth_snapshots: replay_data.stats().depth_count,
        });

        // Update state
        {
            let mut state = self.state.write().await;
            *state = ReplayState::with_data(
                replay_data.time_range,
                ticker_info,
                replay_data.stats().trade_count,
                replay_data.stats().depth_count,
            );
        }

        // Store data
        {
            let mut data = self.data.write().await;
            *data = Some(replay_data);
        }

        // Emit completion event
        self.emit_event(ReplayEvent::LoadingProgress {
            progress: 1.0,
            message: "Loading complete".to_string(),
        });

        let (trade_count, depth_count, time_range) = {
            let st = self.state.read().await;
            (st.trade_count, st.depth_count, st.time_range)
        };
        self.emit_event(ReplayEvent::DataLoaded {
            ticker: ticker_info.ticker,
            trade_count,
            depth_count,
            time_range,
        });

        Ok(())
    }

    /// Check if data is in cache (simplified)
    async fn check_cache(&self, ticker: &FuturesTicker, date_range: &DateRange) -> bool {
        self.trade_repo
            .has_trades(ticker, date_range.start)
            .await
            .unwrap_or_default()
    }

    /// Pre-aggregate common timeframes
    fn pre_aggregate_candles(&self, data: &mut ReplayData, tick_size: f32) {
        // Common timeframes to pre-aggregate
        let timeframes = vec![
            ("1m", 60_000),
            ("5m", 300_000),
            ("15m", 900_000),
            ("1h", 3_600_000),
        ];

        // Collect all trades
        let all_trades: Vec<Trade> = data.trades.values().flatten().cloned().collect();

        let tick_size_price = Price::from_f32(tick_size);

        for (name, millis) in timeframes {
            if let Ok(candles) = aggregate_trades_to_candles(&all_trades, millis, tick_size_price) {
                data.cache_candles(name.to_string(), candles);

                // Emit detailed progress if enabled
                if self.config.detailed_progress {
                    if let Err(e) = self.event_tx.try_send(ReplayEvent::LoadingProgress {
                        progress: 0.9,
                        message: format!("Aggregated {} candles", name),
                    }) {
                        log::warn!("Dropped replay event: {}", e);
                    }
                }
            }
        }
    }

    /// Start playback
    pub async fn play(&mut self) -> Result<(), String> {
        let is_loaded = {
            let state = self.state.read().await;
            state.is_loaded
        };

        if !is_loaded {
            let error = "No data loaded".to_string();
            self.emit_event(ReplayEvent::Error(error.clone()));
            return Err(error);
        }

        // Emit ChartRebuild with all trades from [start, current_position]
        {
            let state = self.state.read().await;
            let start = state.time_range.start.to_millis();
            let position = state.position;
            if let Some(replay_data) = &*self.data.read().await {
                let trades = replay_data.trades_in_window(start, position);
                self.emit_event(ReplayEvent::ChartRebuild { trades });
            }
        }

        // Update status
        {
            let mut state = self.state.write().await;
            state.status = PlaybackStatus::Playing;
        }

        self.emit_event(ReplayEvent::StatusChanged(PlaybackStatus::Playing));
        self.emit_event(ReplayEvent::PlaybackStarted);

        // Start background playback task if not running
        if self.playback_handle.is_none() {
            self.start_playback_task();
        }

        Ok(())
    }

    /// Pause playback
    pub async fn pause(&mut self) -> Result<(), String> {
        {
            let mut state = self.state.write().await;
            state.status = PlaybackStatus::Paused;
        }

        self.emit_event(ReplayEvent::StatusChanged(PlaybackStatus::Paused));
        self.emit_event(ReplayEvent::PlaybackPaused);

        Ok(())
    }

    /// Stop playback and reset
    pub async fn stop(&mut self) -> Result<(), String> {
        // Stop background task
        if let Some(handle) = self.playback_handle.take() {
            handle.abort();
        }

        {
            let mut state = self.state.write().await;
            state.reset();
        }

        self.emit_event(ReplayEvent::StatusChanged(PlaybackStatus::Stopped));
        self.emit_event(ReplayEvent::PlaybackStopped);

        Ok(())
    }

    /// Seek to position
    pub async fn seek(&mut self, timestamp: u64) -> Result<(), String> {
        // Abort current playback task
        if let Some(handle) = self.playback_handle.take() {
            handle.abort();
        }

        let (start, end, was_playing) = {
            let state = self.state.read().await;
            (
                state.time_range.start.to_millis(),
                state.time_range.end.to_millis(),
                state.status == PlaybackStatus::Playing,
            )
        };

        let clamped = timestamp.clamp(start, end);

        {
            let mut state = self.state.write().await;
            state.position = clamped;
        }

        // Emit ChartRebuild with all trades from [start, new_position]
        if let Some(replay_data) = &*self.data.read().await {
            let trades = replay_data.trades_in_window(start, clamped);
            self.emit_event(ReplayEvent::ChartRebuild { trades });
        }

        let progress = {
            let state = self.state.read().await;
            state.progress()
        };

        self.emit_event(ReplayEvent::PositionUpdate {
            timestamp: clamped,
            progress,
        });

        self.emit_event(ReplayEvent::SeekCompleted {
            timestamp: clamped,
            progress,
        });

        // Restart playback task if was playing
        if was_playing {
            self.start_playback_task();
        }

        Ok(())
    }

    /// Set playback speed
    pub async fn set_speed(&mut self, speed: SpeedPreset) -> Result<(), String> {
        {
            let mut state = self.state.write().await;
            state.speed = speed;
        }

        self.emit_event(ReplayEvent::SpeedChanged(speed));

        Ok(())
    }

    /// Jump forward/backward
    pub async fn jump(&mut self, delta_ms: i64) -> Result<(), String> {
        let new_position = {
            let state = self.state.read().await;
            if delta_ms >= 0 {
                state.position.saturating_add(delta_ms as u64)
            } else {
                state.position.saturating_sub(delta_ms.unsigned_abs())
            }
        };

        self.seek(new_position).await
    }

    /// Start background playback task
    fn start_playback_task(&mut self) {
        let state = Arc::clone(&self.state);
        let data = Arc::clone(&self.data);
        let event_tx = self.event_tx.clone();
        let emit_interval = self.config.emit_interval_ms;

        self.playback_handle = Some(tokio::spawn(async move {
            let mut last_emit = std::time::Instant::now();

            loop {
                // Sleep for emit interval
                tokio::time::sleep(tokio::time::Duration::from_millis(emit_interval)).await;

                // Check if playing
                let (status, speed) = {
                    let state = state.read().await;
                    (state.status, state.speed)
                };

                if status != PlaybackStatus::Playing {
                    continue;
                }

                // Calculate elapsed time since last emit
                let elapsed_ms = last_emit.elapsed().as_millis() as u64;
                last_emit = std::time::Instant::now();

                // Calculate new position based on speed
                let advance_ms = (elapsed_ms as f32 * speed.to_multiplier()) as u64;

                let (old_position, new_position, time_range) = {
                    let mut state = state.write().await;
                    let old_pos = state.position;
                    state.position =
                        (state.position + advance_ms).min(state.time_range.end.to_millis());
                    (old_pos, state.position, state.time_range)
                };

                // Get data for the time window (exclusive start to avoid
                // double-counting trades already covered by ChartRebuild)
                if let Some(replay_data) = &*data.read().await {
                    let trades = replay_data.trades_after(old_position, new_position);
                    let depth = replay_data.depth_at(new_position);

                    if !trades.is_empty() || depth.is_some() {
                        if let Err(e) = event_tx.try_send(ReplayEvent::MarketData {
                            timestamp: new_position,
                            trades,
                            depth: depth.cloned(),
                        }) {
                            log::warn!("Dropped replay event: {}", e);
                        }
                    }
                }

                // Update position
                let progress = {
                    let state = state.read().await;
                    state.progress()
                };

                if let Err(e) = event_tx.try_send(ReplayEvent::PositionUpdate {
                    timestamp: new_position,
                    progress,
                }) {
                    log::warn!("Dropped replay event: {}", e);
                }

                // Check if reached end
                if new_position >= time_range.end.to_millis() {
                    {
                        let mut state = state.write().await;
                        state.status = PlaybackStatus::Stopped;
                    }

                    if let Err(e) = event_tx
                        .try_send(ReplayEvent::StatusChanged(PlaybackStatus::Stopped))
                    {
                        log::warn!("Dropped replay event: {}", e);
                    }
                    if let Err(e) = event_tx.try_send(ReplayEvent::PlaybackComplete) {
                        log::warn!("Dropped replay event: {}", e);
                    }
                    break;
                }
            }
        }));
    }

    /// Take the event receiver (can only be called once; subsequent calls return None)
    pub fn take_event_rx(&mut self) -> Option<mpsc::Receiver<ReplayEvent>> {
        self.event_rx.take()
    }

    /// Emit an event (non-blocking, drops event if channel is full)
    fn emit_event(&self, event: ReplayEvent) {
        if let Err(e) = self.event_tx.try_send(event) {
            log::error!("Failed to emit replay event: {}", e);
        }
    }

    /// Get current state (read-only)
    pub async fn state(&self) -> ReplayState {
        self.state.read().await.clone()
    }

    /// Get loaded data statistics
    pub async fn data_stats(&self) -> Option<(usize, usize)> {
        if let Some(data) = &*self.data.read().await {
            let stats = data.stats();
            Some((stats.trade_count, stats.depth_count))
        } else {
            None
        }
    }

    /// Get aggregated candles for current data
    pub async fn get_candles(&self, basis: ChartBasis, tick_size: f32) -> Vec<Candle> {
        if let Some(data) = &*self.data.read().await {
            // Check cache first
            let cache_key = match basis {
                ChartBasis::Time(tf) => format!("{:?}", tf),
                ChartBasis::Tick(count) => format!("tick_{}", count),
            };

            if let Some(cached) = data.get_cached_candles(&cache_key) {
                return cached.clone();
            }

            // Aggregate on demand
            let all_trades: Vec<Trade> = data.trades.values().flatten().cloned().collect();

            let tick_size_price = Price::from_f32(tick_size);

            match basis {
                ChartBasis::Time(tf) => {
                    aggregate_trades_to_candles(&all_trades, tf.to_milliseconds(), tick_size_price)
                        .unwrap_or_default()
                }
                ChartBasis::Tick(count) => {
                    aggregate_trades_to_ticks(&all_trades, count, tick_size_price)
                        .unwrap_or_default()
                }
            }
        } else {
            vec![]
        }
    }

    /// Get all trades from [start, current_position] for syncing a new pane.
    /// Returns None if no data is loaded.
    pub async fn get_rebuild_trades(&self) -> Option<Vec<Trade>> {
        let state = self.state.read().await;
        if !state.is_loaded {
            return None;
        }
        let start = state.time_range.start.to_millis();
        let position = state.position;
        drop(state);
        let data = self.data.read().await;
        data.as_ref().map(|d| d.trades_in_window(start, position))
    }

    /// Get trades for a time window
    pub async fn get_trades(&self, start: u64, end: u64) -> Vec<Trade> {
        if let Some(data) = &*self.data.read().await {
            data.trades_in_window(start, end)
        } else {
            vec![]
        }
    }

    /// Get depth at timestamp
    pub async fn get_depth(&self, timestamp: u64) -> Option<DepthSnapshot> {
        if let Some(data) = &*self.data.read().await {
            data.depth_at(timestamp).cloned()
        } else {
            None
        }
    }

    /// Clear all data and stop playback
    pub async fn clear(&mut self) {
        self.stop().await.ok();

        {
            let mut state = self.state.write().await;
            state.clear();
        }

        {
            let mut data = self.data.write().await;
            *data = None;
        }

        // Emit memory cleared
        self.emit_event(ReplayEvent::MemoryUsage {
            bytes: 0,
            trades: 0,
            depth_snapshots: 0,
        });
    }

    /// Compute a volume histogram from loaded trade data.
    /// Divides the time range into `num_buckets` equal slices and
    /// sums buy/sell volume in each.
    pub async fn compute_volume_histogram(&self, num_buckets: usize) -> Vec<VolumeBucket> {
        let num_buckets = num_buckets.max(1);
        let data_guard = self.data.read().await;
        let Some(data) = data_guard.as_ref() else {
            return vec![VolumeBucket::default(); num_buckets];
        };

        let start = data.time_range.start.to_millis();
        let end = data.time_range.end.to_millis();
        if end <= start {
            return vec![VolumeBucket::default(); num_buckets];
        }

        let bucket_width = (end - start) as f64 / num_buckets as f64;
        let mut buckets = vec![VolumeBucket::default(); num_buckets];

        for (&ts, trades) in &data.trades {
            let idx = ((ts - start) as f64 / bucket_width) as usize;
            let idx = idx.min(num_buckets - 1);
            for trade in trades {
                match trade.side {
                    Side::Buy | Side::Bid => {
                        buckets[idx].buy_volume += trade.quantity.0 as f32;
                    }
                    Side::Sell | Side::Ask => {
                        buckets[idx].sell_volume += trade.quantity.0 as f32;
                    }
                }
            }
        }

        buckets
    }

    /// Memory usage estimate
    pub async fn memory_usage(&self) -> usize {
        if let Some(data) = &*self.data.read().await {
            data.memory_usage()
        } else {
            0
        }
    }
}

/// Clean shutdown
impl Drop for ReplayEngine {
    fn drop(&mut self) {
        if let Some(handle) = self.playback_handle.take() {
            handle.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Price as DomainPrice, Quantity, Side, Timestamp};
    use crate::repository::RepositoryResult;
    use chrono::NaiveDate;

    // Mock repository that returns actual test trade data
    struct MockTradeRepository;

    #[async_trait::async_trait]
    impl TradeRepository for MockTradeRepository {
        async fn get_trades(
            &self,
            _ticker: &FuturesTicker,
            _date_range: &DateRange,
        ) -> RepositoryResult<Vec<Trade>> {
            Ok(vec![
                Trade {
                    time: Timestamp::from_millis(1000),
                    price: DomainPrice::from_f32(100.0),
                    quantity: Quantity(10.0),
                    side: Side::Buy,
                },
                Trade {
                    time: Timestamp::from_millis(2000),
                    price: DomainPrice::from_f32(101.0),
                    quantity: Quantity(20.0),
                    side: Side::Sell,
                },
                Trade {
                    time: Timestamp::from_millis(3000),
                    price: DomainPrice::from_f32(100.5),
                    quantity: Quantity(15.0),
                    side: Side::Buy,
                },
            ])
        }

        async fn has_trades(
            &self,
            _ticker: &FuturesTicker,
            _date: NaiveDate,
        ) -> RepositoryResult<bool> {
            Ok(true) // Simulate cache hit
        }

        async fn get_trades_for_date(
            &self,
            _ticker: &FuturesTicker,
            _date: NaiveDate,
        ) -> RepositoryResult<Vec<Trade>> {
            Ok(vec![])
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
            Ok(vec![])
        }

        async fn stats(&self) -> RepositoryResult<crate::repository::RepositoryStats> {
            Ok(crate::repository::RepositoryStats::default())
        }
    }

    fn test_ticker_info() -> FuturesTickerInfo {
        FuturesTickerInfo {
            ticker: FuturesTicker::new("ES.c.0", crate::domain::futures::FuturesVenue::CMEGlobex),
            tick_size: 0.25,
            min_qty: 1.0,
            contract_size: 50.0,
        }
    }

    #[tokio::test]
    async fn test_enhanced_replay_engine() {
        let mock_repo = Arc::new(MockTradeRepository);
        let mut engine = ReplayEngine::with_default_config(mock_repo, None);

        // Test that engine starts in stopped state with no data loaded
        let state = engine.state().await;
        assert_eq!(state.status, PlaybackStatus::Stopped);
        assert!(!state.is_loaded);
        assert_eq!(state.speed, SpeedPreset::Normal);

        // Load test data via mock repository
        let ticker_info = test_ticker_info();
        let date_range = DateRange::new(
            NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2026, 1, 2).unwrap(),
        )
        .expect("invariant: start <= end for test date range");
        engine
            .load_data(ticker_info, date_range)
            .await
            .expect("load_data should succeed with mock trades");

        // After loading, state should reflect loaded data
        let state = engine.state().await;
        assert!(state.is_loaded, "State should be loaded after load_data");
        assert_eq!(state.status, PlaybackStatus::Stopped);
        assert_eq!(state.trade_count, 3, "Mock returns 3 trades");
        assert_eq!(state.depth_count, 0, "No depth repo provided");

        // Verify data stats through the engine API
        let stats = engine
            .data_stats()
            .await
            .expect("data_stats should return Some after loading");
        assert_eq!(stats.0, 3, "Should have 3 trades");
        assert_eq!(stats.1, 0, "Should have 0 depth snapshots");

        // Test play transition
        engine.play().await.expect("play should succeed");
        let state = engine.state().await;
        assert_eq!(state.status, PlaybackStatus::Playing);

        // Test pause transition
        engine.pause().await.expect("pause should succeed");
        let state = engine.state().await;
        assert_eq!(state.status, PlaybackStatus::Paused);

        // Test stop transition (resets position)
        engine.stop().await.expect("stop should succeed");
        let state = engine.state().await;
        assert_eq!(state.status, PlaybackStatus::Stopped);

        // Test speed change
        engine
            .set_speed(SpeedPreset::Double)
            .await
            .expect("set_speed should succeed");
        let state = engine.state().await;
        assert_eq!(state.speed, SpeedPreset::Double);
    }
}
