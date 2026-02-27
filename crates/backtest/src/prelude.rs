//! Convenience re-exports for strategy authors.
//!
//! Import `use kairos_backtest::prelude::*` to bring all commonly
//! needed types into scope when implementing a [`Strategy`] or
//! working with backtest results.

// ─── Configuration ──────────────────────────────────────────────

pub use crate::config::backtest::BacktestConfig;
pub use crate::config::risk::{PositionSizeMode, RiskConfig, SlippageModel};

// ─── Data feed ──────────────────────────────────────────────────

pub use crate::feed::provider::TradeProvider;

// ─── Orders ─────────────────────────────────────────────────────

pub use crate::order::request::{BracketOrder, NewOrder, OrderRequest};
pub use crate::order::types::{OrderSide, OrderType, TimeInForce};

// ─── Output ─────────────────────────────────────────────────────

pub use crate::output::metrics::PerformanceMetrics;
pub use crate::output::result::BacktestResult;
pub use crate::output::trade_record::{ExitReason, TradeRecord};

// ─── Portfolio ──────────────────────────────────────────────────

pub use crate::portfolio::equity::{EquityCurve, EquityPoint};

// ─── Strategy ───────────────────────────────────────────────────

pub use crate::strategy::metadata::{StrategyCategory, StrategyMetadata};
pub use crate::strategy::registry::{StrategyInfo, StrategyRegistry};
pub use crate::strategy::{
    BacktestError, OrderEvent, Strategy, StrategyContext, StudyBank, StudyRequest,
};

// ─── Re-exports from dependency crates ──────────────────────────

pub use kairos_data::{Candle, FuturesTickerInfo, Price, Side, Timestamp, Trade};
pub use kairos_study::{
    DisplayFormat, ParameterDef, ParameterKind, ParameterValue, StudyConfig, Visibility,
};
