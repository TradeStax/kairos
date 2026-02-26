//! Futures instrument specifications.
//!
//! - `FuturesTicker` — inline 28-byte symbol with venue and expiration parsing
//! - `FuturesTickerInfo` — ticker with tick size, min quantity, contract size
//! - `Timeframe` — bar duration (1s through 1d)
//! - `ContractType` — continuous roll (`ES.c.0`) or specific (`ESH26`)
//! - `FuturesVenue` — exchange venue (CME Globex)

pub mod futures;

// Re-export commonly used types
pub use futures::{
    ContractSpec, ContractType, FuturesTicker, FuturesTickerInfo, FuturesVenue, TickerStats,
    Timeframe,
};
