//! Strategy execution context.
//!
//! [`StrategyContext`] is the read-only snapshot of the simulation
//! state passed to every strategy callback. It provides access to
//! market data, portfolio state, study outputs, instrument specs,
//! and timing information.

use crate::config::instrument::InstrumentSpec;
use crate::feed::aggregation::candle::PartialCandle;
use crate::order::entity::Order;
use crate::portfolio::position::Position;
use crate::strategy::study_bank::StudyBank;
use kairos_data::{Candle, Depth, FuturesTicker, Price, Timeframe, Timestamp, Trade};
use std::collections::HashMap;

/// Current state of the RTH trading session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// Before the regular trading hours session opens.
    PreMarket,
    /// The RTH session is active.
    Open,
    /// The RTH session has closed for the day.
    Closed,
}

/// Read-only snapshot of the simulation state passed to strategy
/// callbacks.
///
/// Contains everything a strategy needs to make decisions: market
/// data across instruments and timeframes, portfolio state, computed
/// study outputs, and session timing information.
///
/// The lifetime `'a` borrows from the engine's internal buffers for
/// the duration of a single callback invocation.
pub struct StrategyContext<'a> {
    // ── Market data ─────────────────────────────────────────────
    /// The most recent trade tick.
    pub trade: &'a Trade,
    /// Completed candles per (instrument, timeframe) pair.
    pub candles: &'a HashMap<(FuturesTicker, Timeframe), Vec<Candle>>,
    /// In-progress (partial) candles per (instrument, timeframe).
    pub partial_candles: &'a HashMap<(FuturesTicker, Timeframe), PartialCandle>,
    /// Latest depth snapshot per instrument.
    pub depth: &'a HashMap<FuturesTicker, Depth>,

    // ── Studies ─────────────────────────────────────────────────
    /// Computed study (indicator) outputs, keyed by the
    /// [`StudyRequest::key`](super::StudyRequest::key) declared by
    /// the strategy.
    pub studies: &'a StudyBank,

    // ── Portfolio ───────────────────────────────────────────────
    /// Open positions indexed by instrument.
    pub positions: &'a HashMap<FuturesTicker, Position>,
    /// Currently active (unfilled) orders.
    pub active_orders: Vec<&'a Order>,
    /// Total account equity (cash + unrealized PnL) in USD.
    pub equity: f64,
    /// Cash balance in USD.
    pub cash: f64,
    /// Available buying power after margin requirements.
    pub buying_power: f64,
    /// Current drawdown as a percentage from peak equity.
    pub drawdown_pct: f64,
    /// Cumulative realized profit and loss in USD.
    pub realized_pnl: f64,

    // ── Time ────────────────────────────────────────────────────
    /// Current simulation timestamp (exchange time).
    pub timestamp: Timestamp,
    /// Local wall-clock time as HHMM (e.g. `930` for 9:30 AM,
    /// `1600` for 4:00 PM).
    pub local_hhmm: u32,
    /// Current session state (pre-market, open, or closed).
    pub session_state: SessionState,
    /// Number of trade ticks processed in the current session.
    pub session_tick_count: u32,
    /// Whether the engine is still in the warm-up period.
    pub is_warmup: bool,

    // ── Instruments ─────────────────────────────────────────────
    /// Instrument specifications indexed by ticker.
    pub instruments: &'a HashMap<FuturesTicker, InstrumentSpec>,
    /// The primary instrument for this strategy run.
    pub primary_instrument: FuturesTicker,
}

impl<'a> StrategyContext<'a> {
    /// Returns completed candles for the primary instrument at the
    /// given timeframe.
    #[must_use]
    pub fn primary_candles(&self, timeframe: Timeframe) -> &[Candle] {
        self.candles
            .get(&(self.primary_instrument, timeframe))
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Returns the open position for the primary instrument, if any.
    #[must_use]
    pub fn primary_position(&self) -> Option<&Position> {
        self.positions.get(&self.primary_instrument)
    }

    /// Returns the instrument spec for the primary instrument.
    #[must_use]
    pub fn primary_spec(&self) -> Option<&InstrumentSpec> {
        self.instruments.get(&self.primary_instrument)
    }

    /// Returns completed candles for a specific instrument at the
    /// given timeframe.
    #[must_use]
    pub fn candles(&self, instrument: FuturesTicker, timeframe: Timeframe) -> &[Candle] {
        self.candles
            .get(&(instrument, timeframe))
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Returns the open position for a specific instrument, if any.
    #[must_use]
    pub fn position(&self, instrument: &FuturesTicker) -> Option<&Position> {
        self.positions.get(instrument)
    }

    /// Returns the instrument spec for a specific instrument.
    #[must_use]
    pub fn spec(&self, instrument: &FuturesTicker) -> Option<&InstrumentSpec> {
        self.instruments.get(instrument)
    }

    /// Returns `true` if there is an open position with non-zero
    /// quantity for the given instrument.
    #[must_use]
    pub fn has_position(&self, instrument: &FuturesTicker) -> bool {
        self.positions
            .get(instrument)
            .is_some_and(|p| p.quantity > 0.0)
    }

    /// Returns the tick size for the primary instrument.
    ///
    /// Falls back to 0.25 (ES default) if the instrument spec is
    /// unavailable.
    #[must_use]
    pub fn tick_size(&self) -> Price {
        self.primary_spec()
            .map(|s| s.tick_size)
            .unwrap_or(Price::from_f64(0.25))
    }

    /// Returns the contract multiplier for the primary instrument.
    ///
    /// Falls back to 50.0 (ES default) if the instrument spec is
    /// unavailable.
    #[must_use]
    pub fn contract_size(&self) -> f32 {
        self.primary_spec()
            .map(|s| s.multiplier as f32)
            .unwrap_or(50.0)
    }

    /// Returns unrealized PnL for the primary instrument in USD.
    ///
    /// Returns 0.0 if there is no open position or the instrument
    /// spec is unavailable.
    #[must_use]
    pub fn unrealized_pnl(&self) -> f64 {
        if let (Some(pos), Some(spec)) = (self.primary_position(), self.primary_spec()) {
            pos.unrealized_pnl(spec.tick_size, spec.tick_value)
        } else {
            0.0
        }
    }
}
