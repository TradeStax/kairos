//! Replay Engine Service
//!
//! Core service that orchestrates historical data replay.
//! Manages loading, buffering, and controlled emission of market data.

use data::{
    Candle, ChartBasis, DateRange, Depth, FuturesTicker, FuturesTickerInfo, PlaybackStatus, Price,
    ReplayData, ReplayState, Side, SpeedPreset, TimeRange, Trade, aggregate_trades_to_candles,
    aggregate_trades_to_ticks,
};
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
        depth: Option<Depth>,
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

    /// Full chart rebuild -- contains ALL trades from [start, current_position].
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
            buffer_window_ms: 60000,
            emit_interval_ms: 100,
            max_trades_per_emit: 1000,
            load_depth: cfg!(feature = "heatmap"),
            pre_aggregate: true,
            event_buffer_size: 1000,
            detailed_progress: true,
            max_memory_mb: 500,
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

    /// Data engine for fetching trades/depth
    engine: Arc<tokio::sync::Mutex<data::engine::DataEngine>>,

    /// Event sender channel (unbounded, shared with global subscription)
    event_tx: mpsc::UnboundedSender<ReplayEvent>,

    /// Background playback task handle
    playback_handle: Option<tokio::task::JoinHandle<()>>,
}

impl ReplayEngine {
    /// Create new Replay engine
    pub fn new(
        config: ReplayEngineConfig,
        engine: Arc<tokio::sync::Mutex<data::engine::DataEngine>>,
        event_tx: mpsc::UnboundedSender<ReplayEvent>,
    ) -> Self {
        Self {
            config,
            state: Arc::new(RwLock::new(ReplayState::new())),
            data: Arc::new(RwLock::new(None)),
            engine,
            event_tx,
            playback_handle: None,
        }
    }

    /// Create with default configuration
    pub fn with_default_config(
        engine: Arc<tokio::sync::Mutex<data::engine::DataEngine>>,
        event_tx: mpsc::UnboundedSender<ReplayEvent>,
    ) -> Self {
        Self::new(ReplayEngineConfig::default(), engine, event_tx)
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

        self.emit_event(ReplayEvent::LoadingProgress {
            progress: 0.0,
            message: format!(
                "Loading {} from {} to {}",
                ticker_info.ticker, date_range.start, date_range.end
            ),
        });

        // Fetch trades via DataEngine
        self.emit_event(ReplayEvent::LoadingProgress {
            progress: 0.2,
            message: "Fetching trades...".to_string(),
        });

        let trades = {
            let mut eng = self.engine.lock().await;
            eng.get_trades(&ticker_info.ticker, &date_range, None)
                .await
                .map_err(|e| {
                    let error = format!("Failed to load trades: {:?}", e);
                    self.emit_event(ReplayEvent::Error(error.clone()));
                    error
                })?
        };

        if trades.is_empty() {
            let error = "No trades found for the specified date range".to_string();
            self.emit_event(ReplayEvent::Error(error.clone()));
            return Err(error);
        }

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

            let mut eng = self.engine.lock().await;
            eng.get_depth(&ticker_info.ticker, &date_range)
                .await
                .unwrap_or_default()
        } else {
            vec![]
        };

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

    /// Pre-aggregate common timeframes
    fn pre_aggregate_candles(&self, data: &mut ReplayData, tick_size: f32) {
        let timeframes = vec![
            ("1m", 60_000),
            ("5m", 300_000),
            ("15m", 900_000),
            ("1h", 3_600_000),
        ];

        let all_trades: Vec<Trade> = data.trades.values().flatten().cloned().collect();

        let tick_size_price = Price::from_f32(tick_size);

        for (name, millis) in timeframes {
            if let Ok(candles) = aggregate_trades_to_candles(&all_trades, millis, tick_size_price) {
                data.cache_candles(name.to_string(), candles);

                if self.config.detailed_progress
                    && let Err(e) = self.event_tx.send(ReplayEvent::LoadingProgress {
                        progress: 0.9,
                        message: format!("Aggregated {} candles", name),
                    })
                {
                    log::warn!("Dropped replay event: {}", e);
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
                tokio::time::sleep(tokio::time::Duration::from_millis(emit_interval)).await;

                let (status, speed) = {
                    let state = state.read().await;
                    (state.status, state.speed)
                };

                if status != PlaybackStatus::Playing {
                    continue;
                }

                let elapsed_ms = last_emit.elapsed().as_millis() as u64;
                last_emit = std::time::Instant::now();

                let advance_ms = (elapsed_ms as f32 * speed.to_multiplier()) as u64;

                let (old_position, new_position, time_range) = {
                    let mut state = state.write().await;
                    let old_pos = state.position;
                    state.position =
                        (state.position + advance_ms).min(state.time_range.end.to_millis());
                    (old_pos, state.position, state.time_range)
                };

                if let Some(replay_data) = &*data.read().await {
                    let trades = replay_data.trades_after(old_position, new_position);
                    let depth = replay_data.depth_at(new_position);

                    if (!trades.is_empty() || depth.is_some())
                        && let Err(e) = event_tx.send(ReplayEvent::MarketData {
                            timestamp: new_position,
                            trades,
                            depth: depth.cloned(),
                        })
                    {
                        log::warn!("Dropped replay event: {}", e);
                    }
                }

                let progress = {
                    let state = state.read().await;
                    state.progress()
                };

                if let Err(e) = event_tx.send(ReplayEvent::PositionUpdate {
                    timestamp: new_position,
                    progress,
                }) {
                    log::warn!("Dropped replay event: {}", e);
                }

                if new_position >= time_range.end.to_millis() {
                    {
                        let mut state = state.write().await;
                        state.status = PlaybackStatus::Stopped;
                    }

                    if let Err(e) =
                        event_tx.send(ReplayEvent::StatusChanged(PlaybackStatus::Stopped))
                    {
                        log::warn!("Dropped replay event: {}", e);
                    }
                    if let Err(e) = event_tx.send(ReplayEvent::PlaybackComplete) {
                        log::warn!("Dropped replay event: {}", e);
                    }
                    break;
                }
            }
        }));
    }

    /// Emit an event to the global replay channel
    fn emit_event(&self, event: ReplayEvent) {
        if let Err(e) = self.event_tx.send(event) {
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
            let cache_key = match basis {
                ChartBasis::Time(tf) => format!("{:?}", tf),
                ChartBasis::Tick(count) => format!("tick_{}", count),
            };

            if let Some(cached) = data.get_cached_candles(&cache_key) {
                return cached.clone();
            }

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

    /// Get all trades from [start, current_position] for syncing a new
    /// pane. Returns None if no data is loaded.
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
    pub async fn get_depth(&self, timestamp: u64) -> Option<Depth> {
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

        self.emit_event(ReplayEvent::MemoryUsage {
            bytes: 0,
            trades: 0,
            depth_snapshots: 0,
        });
    }

    /// Compute a volume histogram from loaded trade data.
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
