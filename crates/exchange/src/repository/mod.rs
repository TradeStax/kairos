//! Repository trait implementations for each data source.
//!
//! - [`databento`] - Futures trades and depth (historical, per-day caching)
//! - [`rithmic`] - Futures trades and depth (real-time + historical)
//! - [`massive`] - Options snapshots, chains, and contracts

pub mod databento;
pub mod massive;
pub mod rithmic;

pub use databento::{DatabentoDepthRepository, DatabentoTradeRepository};
pub use massive::{MassiveChainRepository, MassiveContractRepository, MassiveSnapshotRepository};
pub use rithmic::{RithmicDepthRepository, RithmicTradeRepository};
