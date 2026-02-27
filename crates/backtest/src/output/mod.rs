//! Backtest output types and performance analytics.
//!
//! This module defines the result types produced by a completed backtest run:
//!
//! - [`BacktestResult`] — the complete output of a backtest including trades,
//!   metrics, and equity curve.
//! - [`PerformanceMetrics`] — aggregated performance statistics computed from
//!   completed trades.
//! - [`TradeRecord`] — a single round-trip trade with entry/exit details,
//!   P&L, and excursion data.
//! - [`BacktestProgressEvent`] — streaming events emitted during a running
//!   backtest for live progress tracking.

pub mod metrics;
pub mod progress;
pub mod result;
pub mod trade_record;

pub use metrics::PerformanceMetrics;
pub use progress::BacktestProgressEvent;
pub use result::BacktestResult;
pub use trade_record::{ExitReason, TradeRecord};
