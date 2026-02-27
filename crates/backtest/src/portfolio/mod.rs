//! Portfolio management for backtesting.
//!
//! This module tracks positions, cash balances, margin requirements,
//! and profit/loss across one or more instruments during a backtest
//! run. It is the financial core of the backtesting engine.
//!
//! # Submodules
//!
//! - [`manager`] -- top-level [`Portfolio`] that orchestrates fills,
//!   margin checks, and equity tracking.
//! - [`position`] -- individual [`Position`] state with scale-in
//!   support and MAE/MFE tracking.
//! - [`equity`] -- [`EquityCurve`] and [`DailyEquityTracker`] for
//!   time-series equity recording.
//! - [`margin`] -- [`MarginCalculator`] for initial and maintenance
//!   margin enforcement.
//! - [`accounting`] -- pure functions for PnL and commission math.

pub mod accounting;
pub mod equity;
pub mod manager;
pub mod margin;
pub mod position;

pub use equity::{DailyEquityTracker, DailySnapshot, EquityCurve, EquityPoint};
pub use manager::Portfolio;
pub use margin::MarginCalculator;
pub use position::Position;
