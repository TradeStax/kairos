//! Flowsurface Data Layer - Modern Architecture
//!
//! Clean layered architecture with strict separation of concerns:
//!
//! - **domain**: Pure business logic (types, entities, aggregation)
//! - **repository**: Data access abstraction (traits, cache, composite)
//! - **services**: Business orchestration (market data, cache management)
//! - **state**: Application state management (app state, persistence)
//! - **config**: Configuration (theme, timezone, sidebar)
//! - **util**: Utilities (formatting, math, time)

// ============================================================================
// MODERN ARCHITECTURE - Clean Exports Only
// ============================================================================

pub mod audio_config; // Audio configuration
pub mod chart; // Chart types re-exports
pub mod config; // Configuration
pub mod domain; // Pure domain logic - THE source of truth
pub mod drawing; // Drawing types for chart annotations
pub mod error_types; // Error types
pub mod feed; // Data feed connection model
pub mod log_config; // Logging configuration
pub mod panel_config; // Panel configuration types
pub mod repository; // Data access abstraction
pub mod secrets; // Secure API key management
pub mod services; // Business logic orchestration
pub mod state; // State management with persistence
pub mod util; // Utilities

// Module re-exports
pub use audio_config as audio;
pub use log_config as log;
pub use panel_config as panel;
pub use state as layout;

// Re-export audio types
pub use audio_config::{AudioStream, StreamCfg, Threshold};

// Re-export error types
pub use error_types::InternalError;

// Re-export commonly used types for convenience
pub use domain::{
    Autoscale, CandlePosition, Candle, ChartBasis, ChartConfig, ChartData, ChartType,
    ClusterScaling, DataGap, DataGapKind, DataSchema, DataSegment, DateRange, DepthSnapshot,
    FootprintMode, FootprintStudyConfig, FootprintType, FuturesTicker, FuturesTickerInfo,
    FuturesVenue, HeatmapIndicator, Indicator, KlineDataPoint, KlineIndicator, KlineTrades,
    LoadingStatus, MergeResult, NPoc, PointOfControl, Price, Quantity, Side, Timeframe, Timestamp,
    Trade, UiIndicator, ViewConfig, Volume, aggregate_trades_to_candles,
    aggregate_trades_to_ticks,
};

pub use repository::{
    CompositeTradeRepository, DepthRepository, FeedRepo, RepositoryError, RepositoryResult,
    RepositoryStats, TradeRepository,
};

pub use services::{CacheManagerService, MarketDataService, ServiceError, merge_segments};

pub use state::{
    AppState, Axis, ChartState, ComparisonConfig, ContentKind, Dashboard,
    DownloadedTickersRegistry, HeatmapConfig, KlineConfig, LadderConfig, Layout, LayoutManager,
    Layouts, LinkGroup, Pane, Settings, StateVersion, TimeAndSalesConfig, VisualConfig, WindowSpec,
    load_state, save_state,
};

// Re-export config types
pub use config::ScaleFactor;
pub use config::sidebar;
pub use config::sidebar::Sidebar;
pub use config::theme::Theme;
pub use config::timezone::UserTimezone;

// Re-export secrets types
pub use secrets::{ApiKeyStatus, ApiProvider, SecretsError, SecretsManager};

// Re-export drawing types
pub use drawing::{
    DrawingId, DrawingStyle, DrawingTool, LineStyle, SerializableColor, SerializableDrawing,
    SerializablePoint,
};

// Re-export feed types
pub use feed::{
    DataFeed, DataFeedManager, DatabentoFeedConfig, FeedCapability, FeedConfig, FeedId, FeedKind,
    FeedProvider, FeedStatus, HistoricalDatasetInfo, RithmicEnvironment, RithmicFeedConfig,
};

// Error types
use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum DataError {
    #[error("Service error: {0}")]
    Service(String),
    #[error("Repository error: {0}")]
    Repository(String),
    #[error("State error: {0}")]
    State(String),
}

// Implement From<RepositoryError> for DataError
impl From<RepositoryError> for DataError {
    fn from(err: RepositoryError) -> Self {
        DataError::Repository(err.to_string())
    }
}

// Implement From<ServiceError> for DataError
impl From<ServiceError> for DataError {
    fn from(err: ServiceError) -> Self {
        DataError::Service(err.to_string())
    }
}

// Utility functions
use std::path::PathBuf;

/// Get data directory path
pub fn data_path(path_name: Option<&str>) -> PathBuf {
    let base = if let Ok(path) = std::env::var("FLOWSURFACE_DATA_PATH") {
        PathBuf::from(path)
    } else {
        let data_dir = dirs_next::data_dir().unwrap_or_else(|| PathBuf::from("."));
        data_dir.join("flowsurface")
    };

    if let Some(path_name) = path_name {
        base.join(path_name)
    } else {
        base
    }
}

/// Open data folder in system file browser
pub fn open_data_folder() -> Result<(), DataError> {
    let pathbuf = data_path(None);

    if pathbuf.exists() {
        open::that(&pathbuf)
            .map_err(|e| DataError::State(format!("Failed to open folder: {}", e)))?;
        ::log::info!("Opened data folder: {:?}", pathbuf);
        Ok(())
    } else {
        Err(DataError::State(format!(
            "Data folder does not exist: {:?}",
            pathbuf
        )))
    }
}

/// Safely lock a mutex and recover from poisoned locks
///
/// This is a utility function to handle mutex locks safely by recovering
/// from poisoned locks using the `into_inner()` method.
pub fn lock_or_recover<T>(
    mutex: &std::sync::Arc<std::sync::Mutex<T>>,
) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}
