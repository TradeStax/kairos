//! Market data entities.
//!
//! - `Trade` — single execution with timestamp, price, quantity, side
//! - `Candle` — OHLCV bar with buy/sell volume split
//! - `Depth` — order book snapshot as `BTreeMap<i64, f32>` (price units to quantity)
//! - `MarketData` — tagged union of Trade, Candle, Depth

pub mod entities;

// Re-export commonly used types
pub use entities::{Candle, Depth, MarketData, Trade};
