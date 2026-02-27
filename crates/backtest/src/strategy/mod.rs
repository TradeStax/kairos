//! Strategy abstraction layer for the backtest engine.
//!
//! This module defines the [`Strategy`] trait — the core contract that
//! all backtest strategies implement — along with supporting types for
//! parameter validation, study requests, order events, and error
//! handling.
//!
//! # Architecture
//!
//! ```text
//! StrategyRegistry ──creates──> Box<dyn Strategy>
//!                                   │
//!        ┌──────────────────────────┘
//!        ▼
//!   on_init / on_session_open / on_candle / on_tick / on_session_close
//!        │
//!        └──> Vec<OrderRequest>  ──> BacktestEngine
//! ```
//!
//! Built-in strategies live in [`built_in`]. Custom strategies
//! implement [`Strategy`] directly and register via
//! [`registry::StrategyRegistry::register`].

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

/// Errors produced by backtest engine operations or strategy
/// validation.
#[derive(Debug, Error)]
pub enum BacktestError {
    /// A parameter value failed validation (out of range, wrong type,
    /// or unknown key).
    #[error("invalid parameter '{key}': {reason}")]
    InvalidParameter {
        /// The parameter key that was rejected.
        key: String,
        /// Human-readable explanation of the failure.
        reason: String,
    },

    /// No strategy with the given ID was found in the registry.
    #[error("unknown strategy: {0}")]
    UnknownStrategy(String),

    /// An error originating from the data layer (missing data, I/O).
    #[error("data error: {0}")]
    Data(String),

    /// A configuration or pre-run validation failure.
    #[error("config validation: {0}")]
    Validation(String),

    /// A runtime engine error (processing, state machine).
    #[error("engine: {0}")]
    Engine(String),

    /// The engine rejected an order because the account lacks
    /// sufficient margin.
    #[error(
        "margin rejected: required={required:.2}, \
         available={available:.2}"
    )]
    MarginRejected {
        /// Margin required by the order.
        required: f64,
        /// Margin currently available.
        available: f64,
    },

    /// An order was rejected for a reason other than margin.
    #[error("order rejected: {0}")]
    OrderRejected(String),
}

impl BacktestError {
    /// Convenience constructor for [`BacktestError::InvalidParameter`].
    pub fn invalid_param(key: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::InvalidParameter {
            key: key.into(),
            reason: reason.into(),
        }
    }
}

/// A request for a study (indicator) that a strategy depends on.
///
/// Strategies declare their study requirements via
/// [`Strategy::required_studies`]. The engine creates and manages
/// study instances through the [`StudyBank`].
#[derive(Debug, Clone)]
pub struct StudyRequest {
    /// Unique key used to retrieve this study's output at runtime
    /// (e.g. `"sma_20"`).
    pub key: String,
    /// Study identifier in the [`StudyRegistry`](kairos_study::StudyRegistry)
    /// (e.g. `"sma"`, `"ema"`, `"rsi"`).
    pub study_id: String,
    /// Parameter overrides applied to this study instance.
    pub params: Vec<(String, ParameterValue)>,
}

/// Core trait for backtest strategies.
///
/// A strategy receives market data through lifecycle callbacks and
/// returns [`OrderRequest`]s that the engine executes against the
/// simulated order book.
///
/// # Lifecycle
///
/// The engine calls methods in the following order each session:
///
/// 1. **[`on_init`](Strategy::on_init)** — once at backtest start
/// 2. **[`on_warmup_complete`](Strategy::on_warmup_complete)** — once
///    after the warm-up period elapses
/// 3. **[`on_session_open`](Strategy::on_session_open)** — at RTH
///    open each day
/// 4. **[`on_candle`](Strategy::on_candle)** /
///    **[`on_tick`](Strategy::on_tick)** — repeatedly during RTH
/// 5. **[`on_order_event`](Strategy::on_order_event)** — when fills,
///    cancellations, or rejections occur
/// 6. **[`on_session_close`](Strategy::on_session_close)** — at RTH
///    close each day
/// 7. **[`reset`](Strategy::reset)** — between optimization runs
///
/// # Configuration
///
/// Each strategy exposes typed parameters via [`parameters`](Strategy::parameters)
/// and [`config`](Strategy::config). The engine and UI use these to
/// present parameter controls and validate user input. The default
/// [`set_parameter`](Strategy::set_parameter) implementation handles
/// range/type validation automatically.
///
/// # Cloning
///
/// Strategies must support deep cloning via [`clone_strategy`](Strategy::clone_strategy)
/// because the optimizer needs independent copies for parallel runs.
/// Object-safe `Clone` is not available on trait objects, so this
/// method returns `Box<dyn Strategy>`.
pub trait Strategy: Send + Sync {
    /// Unique identifier for this strategy (e.g. `"orb"`,
    /// `"vwap_reversion"`).
    fn id(&self) -> &str;

    /// Descriptive metadata (name, category, version) for UI
    /// display and serialization.
    fn metadata(&self) -> StrategyMetadata;

    /// Parameter definitions that describe the strategy's
    /// configurable inputs.
    fn parameters(&self) -> &[ParameterDef];

    /// Current parameter values.
    fn config(&self) -> &StudyConfig;

    /// Mutable access to parameter values, used by
    /// [`set_parameter`](Strategy::set_parameter).
    fn config_mut(&mut self) -> &mut StudyConfig;

    /// Declare which studies (indicators) this strategy needs.
    ///
    /// Called once before the run starts. The engine creates study
    /// instances and feeds them candle data automatically. Results
    /// are available through
    /// [`StrategyContext::studies`](context::StrategyContext::studies).
    fn required_studies(&self) -> Vec<StudyRequest> {
        vec![]
    }

    /// Declare additional timeframes beyond the primary config
    /// timeframe.
    ///
    /// The engine will aggregate candles for each returned timeframe
    /// and deliver them via
    /// [`on_candle`](Strategy::on_candle).
    fn required_timeframes(&self) -> Vec<Timeframe> {
        vec![]
    }

    /// Update a single parameter with type and range validation.
    ///
    /// The default implementation validates against the
    /// [`ParameterDef`] for the given key. Override only if custom
    /// cross-parameter validation is needed.
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
                        format!("value {v} out of range [{min}, {max}]"),
                    ));
                }
            }
            (ParameterKind::Float { min, max, .. }, ParameterValue::Float(v)) => {
                if !v.is_finite() || *v < *min || *v > *max {
                    return Err(BacktestError::invalid_param(
                        key,
                        format!("value {v} out of range [{min}, {max}]"),
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

    /// Called once at the very start of the backtest run.
    ///
    /// Use this to initialize any state that depends on the context
    /// (instrument specs, account size, etc.).
    fn on_init(&mut self, _ctx: &StrategyCtx) {}

    /// Called when the warm-up period completes.
    ///
    /// After this point, the strategy's orders will be executed.
    /// During warm-up, the engine feeds data but ignores order
    /// requests.
    fn on_warmup_complete(&mut self, _ctx: &StrategyCtx) {}

    /// Called when the RTH session opens.
    ///
    /// Use this to reset per-session state and optionally submit
    /// opening orders.
    fn on_session_open(&mut self, ctx: &StrategyCtx) -> Vec<OrderRequest>;

    /// Called after a candle closes on any tracked timeframe.
    ///
    /// This is the primary decision point for candle-based
    /// strategies. The `instrument` and `timeframe` identify which
    /// data stream produced the candle.
    fn on_candle(
        &mut self,
        instrument: FuturesTicker,
        timeframe: Timeframe,
        candle: &Candle,
        ctx: &StrategyCtx,
    ) -> Vec<OrderRequest>;

    /// Called on every trade tick during RTH.
    ///
    /// Use sparingly — this fires at very high frequency. Prefer
    /// [`on_candle`](Strategy::on_candle) for most logic.
    fn on_tick(&mut self, ctx: &StrategyCtx) -> Vec<OrderRequest>;

    /// Called when the RTH session closes.
    ///
    /// Strategies should flatten any open positions here unless they
    /// intend to hold overnight.
    fn on_session_close(&mut self, ctx: &StrategyCtx) -> Vec<OrderRequest>;

    /// Called when an order lifecycle event occurs (fill, cancel,
    /// reject).
    ///
    /// Returns additional order requests in response to the event
    /// (e.g. adjusting stops after a partial fill).
    fn on_order_event(&mut self, _event: OrderEvent, _ctx: &StrategyCtx) -> Vec<OrderRequest> {
        vec![]
    }

    /// Reset all internal state between optimization runs.
    ///
    /// Must restore the strategy to a clean initial state equivalent
    /// to a freshly constructed instance (but preserving current
    /// parameter values).
    fn reset(&mut self);

    /// Deep-clone this strategy into a new boxed trait object.
    ///
    /// Required because `Clone` is not object-safe. The optimizer
    /// uses this to create independent copies for parallel runs.
    fn clone_strategy(&self) -> Box<dyn Strategy>;
}

/// Order lifecycle events delivered to the strategy via
/// [`Strategy::on_order_event`].
#[derive(Debug, Clone)]
pub enum OrderEvent {
    /// An order was filled (fully or partially).
    Filled {
        /// The order that was filled.
        order_id: OrderId,
        /// The price at which the fill occurred.
        fill_price: Price,
        /// The quantity filled.
        fill_quantity: f64,
    },
    /// An order was cancelled (by the strategy or engine).
    Cancelled {
        /// The order that was cancelled.
        order_id: OrderId,
    },
    /// An order was rejected by the engine.
    Rejected {
        /// The order that was rejected.
        order_id: OrderId,
        /// Human-readable rejection reason.
        reason: String,
    },
}
