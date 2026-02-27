//! Backtest configuration module.
//!
//! This module defines the full configuration surface for running a
//! backtest: instrument specifications, risk/position-sizing rules,
//! margin enforcement, slippage models, and the top-level
//! [`BacktestConfig`] that ties them together.

pub mod backtest;
pub mod instrument;
pub mod margin;
pub mod risk;

pub use backtest::BacktestConfig;
pub use instrument::InstrumentSpec;
pub use margin::MarginConfig;
pub use risk::{PositionSizeMode, RiskConfig, SlippageModel};
