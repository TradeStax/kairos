//! Extension traits and utilities for exchange-specific functionality.
//!
//! The `DownloadRepository` trait is defined in `kairos-data` and implemented
//! by `DatabentoTradeRepository` in `repository::databento::trades`.
//!
//! This module also provides `DataIndex` builders that scan connected data
//! stores to discover available data ranges.

pub mod data_index_builder;

pub use data_index_builder::{build_rithmic_contribution, scan_databento_cache};
