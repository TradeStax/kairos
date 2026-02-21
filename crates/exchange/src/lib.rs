//! Kairos Exchange Layer
//!
//! Adapter layer providing market data from external sources.
//!
//! ## Data Sources
//! - **Databento**: CME Globex futures (trades, depth, OHLCV)
//! - **Massive (Polygon)**: US options (chains, Greeks, IV)
//! - **Rithmic**: Real-time CME futures streaming
//!
//! ## Modules
//! - [`types`] - Exchange-specific type definitions (Trade, Kline, Depth)
//! - [`adapter`] - Adapter implementations (Databento, Massive, Rithmic)
//! - [`repository`] - Repository trait implementations per data source
//! - [`error`] - Error types with [`AppError`] trait
//! - [`util`] - Fixed-point Price type and helpers

pub mod adapter;
pub mod error;
pub mod ext;
pub mod repository;
pub mod types;
pub mod util;

// Re-export error types
pub use error::{Error, ExchangeResult};
pub use kairos_data::domain::error::{AppError, ErrorSeverity};

// Re-export domain types from data layer (futures, timeframe, etc.)
pub use kairos_data::domain::{
    ContractSpec, ContractType, FuturesTicker, FuturesTickerInfo, FuturesVenue, TickerStats,
    Timeframe,
};

// Re-export exchange-specific types
pub use types::{Depth, Kline, OpenInterest, TickerInfo, Trade, TradeSide};

// Re-export adapter event
pub use adapter::Event;

// Re-export repository implementations
// Futures (Databento)
pub use repository::{DatabentoDepthRepository, DatabentoTradeRepository};

// Futures (Rithmic)
pub use repository::{RithmicDepthRepository, RithmicTradeRepository};

// Options (Massive)
pub use repository::{
    MassiveChainRepository, MassiveContractRepository, MassiveSnapshotRepository,
};

// Re-export Massive adapter
pub use adapter::massive::{HistoricalOptionsManager, MassiveConfig, MassiveError, MassiveResult};

// Re-export Rithmic adapter
pub use adapter::rithmic::{RithmicClient, RithmicConfig, RithmicError, RithmicStream};

// Re-export download schema wrapper (replaces direct databento::dbn::Schema re-export)
pub use types::DownloadSchema;

// Re-export PushFrequency from its canonical location in adapter::stream
pub use adapter::stream::PushFrequency;

// Re-export DataIndex builder functions
pub use ext::{build_rithmic_contribution, scan_databento_cache};
