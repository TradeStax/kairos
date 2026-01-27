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
pub mod error_types; // Error types
pub mod log_config; // Logging configuration
pub mod panel_config; // Panel configuration types
pub mod repository; // Data access abstraction
pub mod services; // Business logic orchestration
pub mod state; // State management with persistence
pub mod tickers_table_config; // Tickers table configuration
pub mod util; // Utilities

// Module re-exports
pub use audio_config as audio;
pub use log_config as log;
pub use panel_config as panel;
pub use state as layout;
pub use tickers_table_config as tickers_table;

// Re-export audio types
pub use audio_config::{AudioStream, StreamCfg, Threshold};

// Re-export error types
pub use error_types::InternalError;

// Re-export commonly used types for convenience
pub use domain::{
    Autoscale, Candle, ChartBasis, ChartConfig, ChartData, ChartType, ClusterKind, ClusterScaling,
    DataSchema, DateRange, DepthSnapshot, FootprintStudy, FuturesTicker, FuturesTickerInfo, FuturesVenue,
    HeatmapIndicator, Indicator, KlineChartKind, KlineDataPoint, KlineIndicator, KlineTrades,
    LoadingStatus, NPoc, PointOfControl, Price, Quantity, Side, Timeframe, Timestamp, Trade,
    UiIndicator, ViewConfig, Volume, aggregate_trades_to_candles, aggregate_trades_to_ticks,
};

pub use repository::{
    DepthRepository, RepositoryError, RepositoryResult, RepositoryStats, TradeRepository,
};

pub use services::{CacheManagerService, MarketDataService, ServiceError};

pub use state::{
    AppState, Axis, ChartState, ComparisonConfig, ContentKind, Dashboard, DownloadedTickersRegistry,
    HeatmapConfig, KlineConfig, LadderConfig, Layout, LayoutManager, Layouts, LinkGroup, Pane,
    Settings, StateVersion, TimeAndSalesConfig, VisualConfig, WindowSpec, load_state, save_state,
};

// Re-export config types
pub use config::ScaleFactor;
pub use config::sidebar;
pub use config::sidebar::Sidebar;
pub use config::theme::Theme;
pub use config::timezone::UserTimezone;

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

// ============================================================================
// BACKWARD COMPATIBILITY - Deprecated exports for old code
// ============================================================================

// State management backward compatibility
pub use state::AppState as State;

// Persistence helpers
pub const SAVED_STATE_PATH: &str = "saved-state.json";

pub fn write_json_to_file(json_str: &str, file_name: &str) -> Result<(), DataError> {
    use std::fs;
    let path = data_path(None).join(file_name);
    fs::write(&path, json_str)
        .map_err(|e| DataError::State(format!("Failed to write file: {}", e)))?;
    Ok(())
}

// Layout types exported from state module (no duplicates)

/// Stub for removed function - caching now handled by CacheManagerService
pub fn cleanup_old_market_data() {
    ::log::warn!("cleanup_old_market_data is deprecated - use CacheManagerService");
}

// Utility functions
use std::path::PathBuf;

/// Get data directory path
pub fn data_path(path_name: Option<&str>) -> PathBuf {
    if let Ok(path) = std::env::var("FLOWSURFACE_DATA_PATH") {
        PathBuf::from(path)
    } else {
        let data_dir = dirs_next::data_dir().unwrap_or_else(|| PathBuf::from("."));
        if let Some(path_name) = path_name {
            data_dir.join("flowsurface").join(path_name)
        } else {
            data_dir.join("flowsurface")
        }
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
