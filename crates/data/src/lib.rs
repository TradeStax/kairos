//! Data infrastructure for the Kairos charting platform.
//!
//! Domain types, exchange adapters, per-day caching, and the `DataEngine` facade.
//! No GUI dependencies.
//!
//! # Modules
//!
//! - [`domain`] — pure value objects and entities (no I/O, no async)
//! - [`adapter`] — exchange adapters: Databento (historical), Rithmic (real-time)
//! - [`aggregation`] — trade-to-candle aggregation (time-based and tick-based)
//! - [`cache`] — per-day bincode+zstd file storage with atomic writes
//! - [`connection`] — connection configuration and lifecycle
//! - [`engine`] — `DataEngine` facade: routes requests, manages adapters, emits events
//! - [`stream`] — stream subscription types (serializable and runtime-resolved)
//! - [`event`] — `DataEvent` enum delivered via mpsc channel
//! - [`error`] — error hierarchy with `AppError` trait
//! - [`util`] — formatting, math, time, serde helpers
//!
//! # Feature flags
//!
//! - `databento` (default) — Databento adapter for CME Globex historical data
//! - `rithmic` (default) — Rithmic adapter for CME real-time streaming
//! - `heatmap` — depth snapshots and heatmap chart types

pub mod adapter;
pub mod aggregation;
pub mod cache;
pub mod connection;
pub mod domain;
pub mod engine;
pub mod error;
pub mod event;
pub mod stream;
pub mod util;

// ── Re-exports ──────────────────────────────────────────────────────────

pub use error::Error;

pub use aggregation::{
    AggregationError, aggregate_candles_to_timeframe, aggregate_trades_to_candles,
    aggregate_trades_to_ticks,
};

#[cfg(feature = "heatmap")]
pub use domain::HeatmapIndicator;
pub use domain::{
    AppError, Autoscale, Candle, ChartBasis, ChartConfig, ChartData, ChartType, ContractSize,
    ContractSpec, ContractType, DataGap, DataGapKind, DataIndex, DataKey, DataSchema, DataSegment,
    DateRange, Depth, DownloadedTickersRegistry, ErrorSeverity, FeedContribution, FeedId,
    FuturesTicker, FuturesTickerInfo, FuturesVenue, KlineDataPoint, KlineTrades, LoadingStatus,
    MarketData, MergeResult, MinQtySize, MinTicksize, NPoc, PlaybackStatus, PointOfControl,
    Power10, Price, PriceExt, PriceStep, Quantity, ReplayData, ReplayState, Rgba,
    SerializableColor, Side, SpeedPreset, TickerStats, TimeRange, Timeframe, Timestamp, Trade,
    ViewConfig, Volume, hex_to_rgba, ms_to_datetime, rgba_to_hex,
};

pub use connection::{
    Connection, ConnectionCapability, ConnectionConfig, ConnectionKind, ConnectionManager,
    ConnectionProvider, ConnectionStatus, DatabentoConnectionConfig, HistoricalDatasetInfo,
    ResolvedConnection, RithmicConnectionConfig, RithmicEnvironment, RithmicServer,
};

pub use event::DataEvent;

#[cfg(feature = "heatmap")]
pub use stream::PersistDepth;
pub use stream::{
    DownloadSchema, PersistKline, PersistStreamKind, PushFrequency, ResolvedStream, StreamConfig,
    StreamKind, StreamSpecs, StreamTicksize, UniqueStreams,
};

#[cfg(feature = "rithmic")]
pub use adapter::rithmic::client::probe_system_names;
#[cfg(feature = "rithmic")]
pub use adapter::rithmic::{
    RithmicClient, RithmicConfig, RithmicError, build_rithmic_contribution,
};

#[cfg(feature = "databento")]
pub use adapter::databento::DatabentoConfig;

pub use util::logging as log;

/// Scans the Databento cache directory and builds a [`DataIndex`].
///
/// Free-function wrapper around `CacheStore::scan_to_index` for use by the
/// application layer after a Databento feed connects or data download completes.
#[cfg(feature = "databento")]
pub async fn scan_databento_cache(
    cache_root: &std::path::Path,
    feed_id: domain::types::FeedId,
) -> Result<domain::index::DataIndex, String> {
    let store = cache::store::CacheStore::new(cache_root.to_path_buf());
    let index = store
        .scan_to_index(cache::store::CacheProvider::Databento, feed_id)
        .await;
    Ok(index)
}

/// No-op fallback when the `databento` feature is disabled.
#[cfg(not(feature = "databento"))]
pub async fn scan_databento_cache(
    _cache_root: &std::path::Path,
    _feed_id: domain::types::FeedId,
) -> Result<domain::index::DataIndex, String> {
    Ok(domain::index::DataIndex::new())
}

/// Safely locks a mutex, recovering from poisoned locks.
///
/// Logs an error and returns the inner value if the mutex was poisoned.
/// Use when the alternative to recovery is a full application crash.
pub fn lock_or_recover<T>(
    mutex: &std::sync::Arc<std::sync::Mutex<T>>,
) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| {
        ::log::error!(
            "Mutex poisoned: recovering from panicked lock holder. \
             The application may be in an inconsistent state."
        );
        e.into_inner()
    })
}
