//! Margin enforcement configuration.
//!
//! [`MarginConfig`] controls whether the backtest engine enforces
//! margin checks before accepting orders, and allows overriding
//! the per-contract margin amounts from
//! [`InstrumentSpec`](super::instrument::InstrumentSpec).

use serde::{Deserialize, Serialize};

/// Margin enforcement configuration for backtest order validation.
///
/// When `enforce` is `true`, the engine rejects orders that would
/// exceed available buying power based on initial margin
/// requirements. During a position, maintenance margin is checked
/// to trigger margin calls.
///
/// # Defaults
///
/// Margin enforcement is **disabled** by default (all fields zero
/// or `None`), matching the common case of a simple backtest that
/// does not model margin constraints.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct MarginConfig {
    /// Whether to enforce margin checks on order submission.
    ///
    /// When `false`, orders are never rejected for insufficient
    /// margin.
    #[serde(default)]
    pub enforce: bool,
    /// Override for initial margin per contract in USD.
    ///
    /// When `None`, the value from [`InstrumentSpec`] is used.
    ///
    /// [`InstrumentSpec`]: super::InstrumentSpec
    pub initial_margin_override: Option<f64>,
    /// Override for maintenance margin per contract in USD.
    ///
    /// When `None`, the value from [`InstrumentSpec`] is used.
    ///
    /// [`InstrumentSpec`]: super::InstrumentSpec
    pub maintenance_margin_override: Option<f64>,
}
