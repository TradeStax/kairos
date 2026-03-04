//! Trade snapshot types for post-hoc analysis.
//!
//! A [`TradeSnapshot`] captures the surrounding candle data and
//! strategy-specific context at the time a trade was completed.
//! This enables detailed drill-down views in the backtest manager.

use std::collections::BTreeMap;

use kairos_data::{Candle, Price, Timestamp};
use serde::{Deserialize, Serialize};

/// A snapshot of market data and strategy state surrounding a
/// completed trade.
///
/// Stored alongside each [`TradeRecord`](super::TradeRecord) to
/// enable mini-chart rendering and strategy-context display in the
/// backtest manager's trade detail view.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeSnapshot {
    /// All session candles for the primary instrument/timeframe.
    pub candles: Vec<Candle>,
    /// Index into `candles` of the candle containing the entry
    /// fill.
    pub entry_candle_idx: Option<usize>,
    /// Index into `candles` of the candle containing the exit fill.
    pub exit_candle_idx: Option<usize>,
    /// Strategy-specific context values captured at trade close.
    ///
    /// Keys are strategy-defined labels (e.g. `"or_high"`,
    /// `"vwap"`). Uses `BTreeMap` for deterministic serialization
    /// order.
    pub context: BTreeMap<String, ContextValue>,
}

/// A typed value for strategy-specific context data.
///
/// Mirrors the [`ParameterValue`](kairos_study::ParameterValue)
/// pattern from the study crate but is tailored for read-only
/// display of trade-time state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContextValue {
    /// A fixed-point price value.
    Price(Price),
    /// A floating-point numeric value.
    Float(f64),
    /// An integer value.
    Integer(i64),
    /// A boolean flag.
    Bool(bool),
    /// A free-form text value.
    Text(String),
    /// A timestamp value.
    Timestamp(Timestamp),
}

impl ContextValue {
    /// Returns the value as an `f64`, converting prices and
    /// integers as needed.
    ///
    /// Returns `None` for non-numeric variants.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Price(p) => Some(p.to_f64()),
            Self::Float(f) => Some(*f),
            Self::Integer(i) => Some(*i as f64),
            _ => None,
        }
    }

    /// Returns the value as a [`Price`], converting floats and
    /// integers as needed.
    ///
    /// Returns `None` for non-numeric variants.
    pub fn as_price(&self) -> Option<Price> {
        match self {
            Self::Price(p) => Some(*p),
            Self::Float(f) => Some(Price::from_f64(*f)),
            Self::Integer(i) => Some(Price::from_f64(*i as f64)),
            _ => None,
        }
    }
}

impl std::fmt::Display for ContextValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Price(p) => write!(f, "{}", p.to_f64()),
            Self::Float(v) => write!(f, "{v:.4}"),
            Self::Integer(v) => write!(f, "{v}"),
            Self::Bool(v) => write!(f, "{v}"),
            Self::Text(s) => write!(f, "{s}"),
            Self::Timestamp(ts) => write!(f, "{}", ts.0),
        }
    }
}
