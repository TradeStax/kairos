pub mod built_in;
pub mod context;
pub mod metadata;
pub mod registry;
pub mod study_bank;

pub use context::StrategyContext;
pub use study_bank::StudyBank;

use crate::order::request::OrderRequest;
use crate::order::types::OrderId;
use crate::strategy::context::StrategyContext as StrategyCtx;
use crate::strategy::metadata::StrategyMetadata;
use kairos_data::{Candle, FuturesTicker, Price, Timeframe};
use kairos_study::{ParameterDef, ParameterKind, ParameterValue, StudyConfig};
use thiserror::Error;

/// Errors produced by the backtest engine or strategy validation.
#[derive(Debug, Error)]
pub enum BacktestError {
    #[error("invalid parameter '{key}': {reason}")]
    InvalidParameter { key: String, reason: String },
    #[error("unknown strategy: {0}")]
    UnknownStrategy(String),
    #[error("data error: {0}")]
    Data(String),
    #[error("config validation: {0}")]
    Validation(String),
    #[error("engine: {0}")]
    Engine(String),
    #[error("margin rejected: required={required:.2}, available={available:.2}")]
    MarginRejected { required: f64, available: f64 },
    #[error("order rejected: {0}")]
    OrderRejected(String),
}

impl BacktestError {
    pub fn invalid_param(key: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::InvalidParameter {
            key: key.into(),
            reason: reason.into(),
        }
    }
}

/// Request for a study (indicator) that the strategy needs.
#[derive(Debug, Clone)]
pub struct StudyRequest {
    /// Unique key to retrieve this study's output (e.g. "sma_20").
    pub key: String,
    /// Study ID in the StudyRegistry (e.g. "sma").
    pub study_id: String,
    /// Parameter overrides for this study instance.
    pub params: Vec<(String, ParameterValue)>,
}

/// Core trait for v2 backtest strategies.
///
/// Strategies return `Vec<OrderRequest>` instead of `Vec<Signal>`.
/// They receive a rich `StrategyContext` with multi-instrument,
/// multi-timeframe, study outputs, and portfolio state.
pub trait Strategy: Send + Sync {
    fn id(&self) -> &str;
    fn metadata(&self) -> StrategyMetadata;
    fn parameters(&self) -> &[ParameterDef];
    fn config(&self) -> &StudyConfig;
    fn config_mut(&mut self) -> &mut StudyConfig;

    /// Declare which studies (indicators) this strategy needs.
    /// Called once before the run starts.
    fn required_studies(&self) -> Vec<StudyRequest> {
        vec![]
    }

    /// Declare additional timeframes beyond the primary config
    /// timeframe.
    fn required_timeframes(&self) -> Vec<Timeframe> {
        vec![]
    }

    /// Update a single parameter with validation.
    fn set_parameter(&mut self, key: &str, value: ParameterValue) -> Result<(), BacktestError> {
        let def = self
            .parameters()
            .iter()
            .find(|p| p.key == key)
            .ok_or_else(|| BacktestError::invalid_param(key, "not found"))?;

        match (&def.kind, &value) {
            (ParameterKind::Integer { min, max }, ParameterValue::Integer(v)) => {
                if *v < *min || *v > *max {
                    return Err(BacktestError::invalid_param(
                        key,
                        format!(
                            "value {v} out of range \
                             [{min}, {max}]"
                        ),
                    ));
                }
            }
            (ParameterKind::Float { min, max, .. }, ParameterValue::Float(v)) => {
                if !v.is_finite() || *v < *min || *v > *max {
                    return Err(BacktestError::invalid_param(
                        key,
                        format!(
                            "value {v} out of range \
                             [{min}, {max}]"
                        ),
                    ));
                }
            }
            (ParameterKind::Boolean, ParameterValue::Boolean(_)) => {}
            (ParameterKind::Color, ParameterValue::Color(_)) => {}
            (ParameterKind::Choice { options }, ParameterValue::Choice(s)) => {
                if !options.contains(&s.as_str()) {
                    return Err(BacktestError::invalid_param(
                        key,
                        format!("invalid choice '{s}'"),
                    ));
                }
            }
            _ => {
                return Err(BacktestError::invalid_param(key, "type mismatch"));
            }
        }

        self.config_mut().set(key, value);
        Ok(())
    }

    // ─── Lifecycle callbacks ────────────────────────────────────

    /// Called once at the start of the backtest.
    fn on_init(&mut self, _ctx: &StrategyCtx) {}

    /// Called when warm-up period completes.
    fn on_warmup_complete(&mut self, _ctx: &StrategyCtx) {}

    /// Called when the RTH session opens.
    fn on_session_open(&mut self, ctx: &StrategyCtx) -> Vec<OrderRequest>;

    /// Called after a candle closes on any tracked timeframe.
    fn on_candle(
        &mut self,
        instrument: FuturesTicker,
        timeframe: Timeframe,
        candle: &Candle,
        ctx: &StrategyCtx,
    ) -> Vec<OrderRequest>;

    /// Called on every trade tick during RTH.
    fn on_tick(&mut self, ctx: &StrategyCtx) -> Vec<OrderRequest>;

    /// Called when the RTH session closes.
    fn on_session_close(&mut self, ctx: &StrategyCtx) -> Vec<OrderRequest>;

    /// Called when an order event occurs (fill, cancel, reject).
    fn on_order_event(&mut self, _event: OrderEvent, _ctx: &StrategyCtx) -> Vec<OrderRequest> {
        vec![]
    }

    /// Reset all internal state between runs.
    fn reset(&mut self);

    /// Deep-clone this strategy.
    fn clone_strategy(&self) -> Box<dyn Strategy>;
}

/// Order lifecycle events delivered to the strategy.
#[derive(Debug, Clone)]
pub enum OrderEvent {
    Filled {
        order_id: OrderId,
        fill_price: Price,
        fill_quantity: f64,
    },
    Cancelled {
        order_id: OrderId,
    },
    Rejected {
        order_id: OrderId,
        reason: String,
    },
}
