use crate::config::instrument::InstrumentSpec;
use crate::feed::aggregation::candle::PartialCandle;
use crate::order::entity::Order;
use crate::portfolio::position::Position;
use crate::strategy::study_bank::StudyBank;
use kairos_data::{Candle, Depth, FuturesTicker, Price, Timeframe, Timestamp, Trade};
use std::collections::HashMap;

/// Session state for strategy context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    PreMarket,
    Open,
    Closed,
}

/// Full context passed to strategy callbacks.
pub struct StrategyContext<'a> {
    // ── Market data ─────────────────────────────────────────────
    /// The current trade tick.
    pub trade: &'a Trade,
    /// Completed candles per (instrument, timeframe).
    pub candles: &'a HashMap<(FuturesTicker, Timeframe), Vec<Candle>>,
    /// Partial (in-progress) candles per (instrument, timeframe).
    pub partial_candles: &'a HashMap<(FuturesTicker, Timeframe), PartialCandle>,
    /// Latest depth snapshot per instrument.
    pub depth: &'a HashMap<FuturesTicker, Depth>,

    // ── Studies ─────────────────────────────────────────────────
    /// Study bank with computed indicators.
    pub studies: &'a StudyBank,

    // ── Portfolio ───────────────────────────────────────────────
    /// Open positions by instrument.
    pub positions: &'a HashMap<FuturesTicker, Position>,
    /// Active orders.
    pub active_orders: Vec<&'a Order>,
    /// Current equity in USD.
    pub equity: f64,
    /// Current cash balance.
    pub cash: f64,
    /// Current buying power.
    pub buying_power: f64,
    /// Current drawdown percentage from peak.
    pub drawdown_pct: f64,
    /// Cumulative realized PnL.
    pub realized_pnl: f64,

    // ── Time ────────────────────────────────────────────────────
    /// Current simulation timestamp.
    pub timestamp: Timestamp,
    /// Local time as HHMM (e.g. 930, 1600).
    pub local_hhmm: u32,
    /// Session state (PreMarket, Open, Closed).
    pub session_state: SessionState,
    /// Number of trade ticks in the current session.
    pub session_tick_count: u32,
    /// Whether we are in the warm-up period.
    pub is_warmup: bool,

    // ── Instruments ─────────────────────────────────────────────
    /// Instrument specs by ticker.
    pub instruments: &'a HashMap<FuturesTicker, InstrumentSpec>,
    /// The primary instrument ticker.
    pub primary_instrument: FuturesTicker,
}

impl<'a> StrategyContext<'a> {
    /// Get candles for the primary instrument at a given
    /// timeframe.
    pub fn primary_candles(&self, timeframe: Timeframe) -> &[Candle] {
        self.candles
            .get(&(self.primary_instrument, timeframe))
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Get the position for the primary instrument.
    pub fn primary_position(&self) -> Option<&Position> {
        self.positions.get(&self.primary_instrument)
    }

    /// Get the instrument spec for the primary instrument.
    pub fn primary_spec(&self) -> Option<&InstrumentSpec> {
        self.instruments.get(&self.primary_instrument)
    }

    /// Get candles for a specific instrument at a given timeframe.
    pub fn candles(&self, instrument: FuturesTicker, timeframe: Timeframe) -> &[Candle] {
        self.candles
            .get(&(instrument, timeframe))
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Get the position for a specific instrument.
    pub fn position(&self, instrument: &FuturesTicker) -> Option<&Position> {
        self.positions.get(instrument)
    }

    /// Get the instrument spec for a specific instrument.
    pub fn spec(&self, instrument: &FuturesTicker) -> Option<&InstrumentSpec> {
        self.instruments.get(instrument)
    }

    /// Convenience: check if there's an open position for an
    /// instrument.
    pub fn has_position(&self, instrument: &FuturesTicker) -> bool {
        self.positions
            .get(instrument)
            .is_some_and(|p| p.quantity > 0.0)
    }

    /// Get the tick size for the primary instrument.
    pub fn tick_size(&self) -> Price {
        self.primary_spec()
            .map(|s| s.tick_size)
            .unwrap_or(Price::from_f64(0.25))
    }

    /// Get the contract multiplier for the primary instrument.
    pub fn contract_size(&self) -> f32 {
        self.primary_spec()
            .map(|s| s.multiplier as f32)
            .unwrap_or(50.0)
    }

    /// Get unrealized PnL for the primary instrument.
    pub fn unrealized_pnl(&self) -> f64 {
        if let (Some(pos), Some(spec)) = (self.primary_position(), self.primary_spec()) {
            pos.unrealized_pnl(spec.tick_size, spec.tick_value)
        } else {
            0.0
        }
    }
}
