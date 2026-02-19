//! Flowsurface Exchange Layer - Modern Architecture
//!
//! Exchange adapter layer providing market data from external sources.
//!
//! ## Data Sources
//! - **Databento**: CME Globex futures market data (trades, depth, OHLCV)
//! - **Massive (Polygon)**: US options market data (chains, Greeks, IV)
//!
//! ## Modules
//! - **types**: All type definitions (futures, market data, depth, timeframe)
//! - **adapter**: Adapter pattern for exchange integration
//! - **repository**: Repository implementations for each data source
//! - **config**: Exchange configuration
//! - **error**: Error handling
//! - **util**: Price utilities and helpers

// ============================================================================
// MODERN ARCHITECTURE - Exchange Adapter Layer
// ============================================================================

pub mod adapter; // Adapter pattern (Databento implementation)
pub mod config; // Exchange configuration
pub mod error; // Error types
pub mod repository; // Repository implementations (Databento)
pub mod types; // Consolidated type definitions
pub mod util; // Price utilities

// Re-export error types
pub use error::{Error, ExchangeResult};

// Re-export domain types from data layer (futures, timeframe, etc.)
pub use flowsurface_data::domain::{
    ContractSpec, ContractType, FuturesTicker, FuturesTickerInfo, FuturesVenue, TickerStats,
    Timeframe,
};

// Re-export exchange-specific types
pub use types::{Depth, Kline, OpenInterest, TickerInfo, Trade, TradeSide};

// Type alias for compatibility
pub type Ticker = FuturesTicker;

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
pub use adapter::massive::{
    HistoricalOptionsManager, MassiveConfig, MassiveError, MassiveResult,
};

// Re-export Rithmic adapter
pub use adapter::rithmic::{
    RithmicClient, RithmicConfig, RithmicError, RithmicStream,
};
pub use rithmic_rs::{self, RithmicEnv};

// Re-export Databento Schema for UI access
pub use databento::dbn::Schema as DatabentoSchema;

/// Check if symbol is supported
pub fn is_symbol_supported(
    symbol: &str,
    _venue: FuturesVenue,
    log_warn: bool,
) -> bool {
    let valid = symbol.chars().all(|c| {
        c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.'
    });
    if !valid && log_warn {
        log::warn!("Unsupported ticker symbol: '{}'", symbol);
    }
    valid
}

// TickMultiplier removed - was only needed for crypto which this project doesn't support
// Use ticker_info.tick_size directly for futures tick sizes

/// Push frequency for orderbook updates
#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize,
)]
pub enum PushFrequency {
    #[default]
    ServerDefault,
    Custom(Timeframe),
}

impl std::fmt::Display for PushFrequency {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PushFrequency::ServerDefault => write!(f, "Server Default"),
            PushFrequency::Custom(tf) => write!(f, "{}", tf),
        }
    }
}

/// Serializable ticker for map keys
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SerTicker {
    pub venue: FuturesVenue,
    pub ticker: FuturesTicker,
}

impl SerTicker {
    pub fn new(venue: FuturesVenue, ticker_str: &str) -> Self {
        let ticker = FuturesTicker::new(ticker_str, venue);
        Self { venue, ticker }
    }

    pub fn from_parts(ticker: FuturesTicker) -> Self {
        Self {
            venue: ticker.venue,
            ticker,
        }
    }
}

impl serde::Serialize for SerTicker {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let combined = format!("CMEGlobex:{}", self.ticker.as_str());
        serializer.serialize_str(&combined)
    }
}

impl<'de> serde::Deserialize<'de> for SerTicker {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let parts: Vec<&str> = s.split(':').collect();

        if parts.len() != 2 {
            return Err(serde::de::Error::custom(format!(
                "Invalid SerTicker format: expected 'Venue:Ticker', got '{}'",
                s
            )));
        }

        let venue = match parts[0] {
            "CMEGlobex" => FuturesVenue::CMEGlobex,
            _ => {
                return Err(serde::de::Error::custom(format!(
                    "Unknown venue: {}",
                    parts[0]
                )));
            }
        };

        let ticker = FuturesTicker::new(parts[1], venue);
        Ok(SerTicker { venue, ticker })
    }
}
