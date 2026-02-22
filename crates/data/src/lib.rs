//! Kairos Data Layer
//!
//! Layered architecture with strict separation of concerns:
//!
//! - **domain**: Pure business logic (types, entities, aggregation)
//! - **repository**: Data access abstraction (traits, cache, composite)
//! - **services**: Business orchestration (market data, cache management)
//! - **state**: Application state management (app state, persistence)
//! - **config**: Configuration (theme, timezone, sidebar)
//! - **util**: Utilities (formatting, math, time)

pub mod config; // Configuration
pub mod domain; // Pure domain logic - THE source of truth
pub mod drawing; // Drawing types for chart annotations
pub mod error; // Crate-level error types
pub mod feed; // Data feed connection model
pub mod repository; // Data access abstraction
pub mod services; // Business logic orchestration
pub mod state; // State management with persistence
pub mod util; // Utilities

// Re-export error types
pub use domain::error::{AppError, ErrorSeverity};

// Re-export commonly used types for convenience
pub use domain::{
    Autoscale, Candle, ChartBasis, ChartConfig, ChartData, ChartType, DataGap, DataGapKind,
    DataIndex, DataKey, DataSchema, DataSegment, DateRange, DepthSnapshot, FuturesTicker,
    FuturesTickerInfo, FuturesVenue, HeatmapIndicator, KlineDataPoint, KlineTrades, LoadingStatus,
    MergeResult, NPoc, PointOfControl, Price, Quantity, Side, Timeframe, Timestamp, Trade,
    ViewConfig, Volume, aggregate_trades_to_candles, aggregate_trades_to_ticks,
};

pub use repository::{
    CompositeTradeRepository, DepthRepository, DownloadRepository, FeedRepo, RepositoryError,
    RepositoryResult, RepositoryStats, TradeRepository,
};

pub use services::{
    CacheManagerService, DataRequestEstimate, MarketDataService, ServiceError, merge_segments,
};

pub use state::{
    AppState, Axis, ChartState, ComparisonConfig, ContentKind, Dashboard,
    DownloadedTickersRegistry, HeatmapConfig, KlineConfig, LadderConfig, Layout, LayoutManager,
    Layouts, LinkGroup, Pane, ProfileConfig, ProfileDisplayType, ProfileLengthUnit, ProfilePeriod,
    Settings, StateVersion, StudyInstanceConfig, TimeAndSalesConfig,
    VisualConfig, WindowSpec, load_state, save_state,
};

// Re-export config types
pub use config::color::Rgba;
pub use config::ScaleFactor;
pub use config::sidebar;
pub use config::sidebar::Sidebar;
pub use config::theme::Theme;
pub use config::timezone::UserTimezone;

// Re-export config secrets (domain types only; SecretsManager lives in GUI crate)
pub use config::secrets::{ApiKeyStatus, ApiProvider, SecretsError};

// Re-export drawing types
pub use drawing::{
    CalcMode, DrawingId, DrawingStyle, DrawingTool, FibLevel, FibonacciConfig, LabelAlignment,
    LineStyle, PositionCalcConfig, SerializableColor, SerializableDrawing, SerializablePoint,
    VbpDrawingConfig,
};

// Re-export feed types
pub use feed::{
    DataFeed, DataFeedManager, DatabentoFeedConfig, FeedCapability, FeedConfig, FeedId, FeedKind,
    FeedProvider, FeedStatus, HistoricalDatasetInfo, RithmicEnvironment, RithmicFeedConfig,
};

// Re-export logging util for convenience
pub use util::logging as log;

// Re-export crate-level error type
pub use error::DataError;

/// Safely lock a mutex and recover from poisoned locks
///
/// This is a utility function to handle mutex locks safely by recovering
/// from poisoned locks using the `into_inner()` method.
pub fn lock_or_recover<T>(
    mutex: &std::sync::Arc<std::sync::Mutex<T>>,
) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}
