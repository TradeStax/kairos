//! Top-level portfolio manager for backtest execution.
//!
//! [`Portfolio`] is the central financial state machine. It owns all
//! open positions, tracks cash and realized PnL, enforces margin
//! constraints, and produces [`TradeRecord`]s when positions close.

use crate::config::instrument::InstrumentSpec;
use crate::order::types::OrderSide;
use crate::output::trade_record::{ExitReason, TradeRecord};
use crate::portfolio::margin::MarginCalculator;
use crate::portfolio::position::Position;
use kairos_data::{FuturesTicker, Price, Timestamp};
use std::collections::HashMap;

/// Manages positions, cash, margin, and PnL for a backtest run.
///
/// The portfolio processes fill events from the matching engine,
/// maintains a cash ledger with commission accounting, and emits
/// completed [`TradeRecord`]s when positions are fully closed.
pub struct Portfolio {
    /// Starting capital for this run (used by [`reset`](Self::reset)).
    initial_equity: f64,
    /// Current cash balance (initial equity + realized PnL
    /// - commissions).
    cash: f64,
    /// Cumulative net realized PnL across all closed trades.
    realized_pnl: f64,
    /// Currently open positions, keyed by instrument.
    positions: HashMap<FuturesTicker, Position>,
    /// Contract specifications for tradeable instruments.
    instruments: HashMap<FuturesTicker, InstrumentSpec>,
    /// Optional margin enforcement (None = unlimited buying power).
    margin_calculator: Option<MarginCalculator>,
    /// Commission charged per side per contract (USD).
    commission_per_side: f64,
    /// High-water mark of total equity for drawdown calculation.
    peak_equity: f64,
    /// Monotonically increasing trade counter for [`TradeRecord`]
    /// indexing.
    trade_index: usize,
}

impl Portfolio {
    /// Create a new portfolio with the given starting capital.
    ///
    /// # Arguments
    ///
    /// * `initial_equity` -- starting cash balance in USD.
    /// * `instruments` -- contract specs for all tradeable symbols.
    /// * `commission_per_side` -- fee per contract per side (USD).
    /// * `margin_calculator` -- optional margin enforcement; pass
    ///   `None` for unlimited buying power.
    #[must_use]
    pub fn new(
        initial_equity: f64,
        instruments: HashMap<FuturesTicker, InstrumentSpec>,
        commission_per_side: f64,
        margin_calculator: Option<MarginCalculator>,
    ) -> Self {
        Self {
            initial_equity,
            cash: initial_equity,
            realized_pnl: 0.0,
            positions: HashMap::new(),
            instruments,
            margin_calculator,
            commission_per_side,
            peak_equity: initial_equity,
            trade_index: 0,
        }
    }

    /// Current cash balance (initial equity + closed-trade PnL
    /// - commissions).
    #[must_use]
    pub fn cash(&self) -> f64 {
        self.cash
    }

    /// Cumulative net realized PnL from all closed trades.
    #[must_use]
    pub fn realized_pnl(&self) -> f64 {
        self.realized_pnl
    }

    /// Read-only view of all open positions.
    #[must_use]
    pub fn positions(&self) -> &HashMap<FuturesTicker, Position> {
        &self.positions
    }

    /// Mutable access to open positions (for engine-level
    /// operations like forced liquidation).
    pub fn positions_mut(&mut self) -> &mut HashMap<FuturesTicker, Position> {
        &mut self.positions
    }

    /// Total equity = cash + unrealized PnL across all open
    /// positions.
    ///
    /// This is the portfolio's mark-to-market value.
    #[must_use]
    pub fn total_equity(&self) -> f64 {
        let unrealized: f64 = self
            .positions
            .values()
            .map(|pos| {
                self.instruments
                    .get(&pos.instrument)
                    .map(|i| pos.unrealized_pnl(i.tick_size, i.tick_value))
                    .unwrap_or(0.0)
            })
            .sum();
        self.cash + unrealized
    }

    /// Available buying power = cash - maintenance margin used by
    /// open positions.
    ///
    /// Returns the full cash balance when no margin calculator is
    /// configured.
    #[must_use]
    pub fn buying_power(&self) -> f64 {
        if let Some(ref mc) = self.margin_calculator {
            let margin_used: f64 = self
                .positions
                .values()
                .map(|pos| mc.position_margin(pos.quantity, &pos.instrument, &self.instruments))
                .sum();
            self.cash - margin_used
        } else {
            self.cash
        }
    }

    /// Check whether margin allows a new order of the given size.
    ///
    /// Returns `true` if buying power exceeds the initial margin
    /// requirement, or if no margin calculator is configured.
    #[must_use]
    pub fn check_margin(&self, instrument: &FuturesTicker, quantity: f64) -> bool {
        if let Some(ref mc) = self.margin_calculator {
            let required = mc.order_margin(quantity, instrument, &self.instruments);
            self.buying_power() >= required
        } else {
            true
        }
    }

    /// Process a fill event from the matching engine.
    ///
    /// Applies commission, updates position state, and returns a
    /// [`TradeRecord`] if a position was fully closed.
    ///
    /// # Fill semantics
    ///
    /// - **New position**: creates a [`Position`] on the given side.
    /// - **Same-side fill**: scales into the existing position.
    /// - **Opposite-side fill**: reduces/closes the position and
    ///   realizes PnL.
    #[allow(clippy::too_many_arguments)]
    pub fn process_fill(
        &mut self,
        instrument: FuturesTicker,
        side: OrderSide,
        fill_price: Price,
        fill_qty: f64,
        timestamp: Timestamp,
        exit_reason: Option<ExitReason>,
        label: Option<String>,
    ) -> Option<TradeRecord> {
        // Charge entry commission up front
        let commission = self.commission_per_side * fill_qty;
        self.cash -= commission;

        if let Some(pos) = self.positions.get_mut(&instrument) {
            if side == pos.side {
                // Adding to existing position (scale in)
                pos.apply_fill(side, fill_price, fill_qty, timestamp);
                None
            } else {
                // Closing/reducing position (opposite side)
                let (consumed, closed) = pos.apply_fill(side, fill_price, fill_qty, timestamp);

                let (tick_size, tick_value) = self
                    .instruments
                    .get(&instrument)
                    .map(|i| (i.tick_size, i.tick_value))
                    .unwrap_or((Price::from_f64(0.25), 12.50));

                let tick_units = tick_size.units().max(1);
                let pnl_ticks = crate::portfolio::accounting::pnl_ticks(
                    pos.side,
                    pos.avg_entry_price,
                    fill_price,
                    tick_size,
                );
                let pnl_gross = pnl_ticks as f64 * tick_value * consumed;
                let total_commission = crate::portfolio::accounting::round_trip_commission(
                    consumed,
                    self.commission_per_side,
                );
                let pnl_net = pnl_gross - total_commission;

                self.realized_pnl += pnl_net;
                self.cash += pnl_gross;

                let record = if closed {
                    self.trade_index += 1;

                    let stop_dist_ticks = pos
                        .initial_stop_loss
                        .map(|sl| {
                            let dist = match pos.side {
                                OrderSide::Buy => pos.avg_entry_price.units() - sl.units(),
                                OrderSide::Sell => sl.units() - pos.avg_entry_price.units(),
                            };
                            (dist / tick_units).abs()
                        })
                        .unwrap_or(0);

                    // R:R = profit ticks / risk ticks
                    let rr = if stop_dist_ticks != 0 {
                        pnl_ticks as f64 / stop_dist_ticks as f64
                    } else {
                        0.0
                    };

                    Some(TradeRecord {
                        index: self.trade_index,
                        entry_time: pos.opened_at,
                        exit_time: timestamp,
                        side: pos.side.to_data_side(),
                        quantity: consumed,
                        entry_price: pos.avg_entry_price,
                        exit_price: fill_price,
                        initial_stop_loss: pos.initial_stop_loss.unwrap_or(pos.avg_entry_price),
                        initial_take_profit: None,
                        pnl_ticks,
                        pnl_gross_usd: pnl_gross,
                        commission_usd: total_commission,
                        pnl_net_usd: pnl_net,
                        rr_ratio: rr,
                        mae_ticks: pos.mae_ticks(tick_size),
                        mfe_ticks: pos.mfe_ticks(tick_size),
                        exit_reason: exit_reason.unwrap_or(ExitReason::Manual),
                        label: pos.label.clone(),
                        instrument: Some(instrument),
                        duration_ms: Some(timestamp.0.saturating_sub(pos.opened_at.0)),
                    })
                } else {
                    None
                };

                if closed {
                    self.positions.remove(&instrument);
                }

                record
            }
        } else {
            // No existing position -- open a new one
            let pos = Position::new(instrument, side, fill_price, fill_qty, timestamp, label);
            self.positions.insert(instrument, pos);
            None
        }
    }

    /// Mark all open positions to market with current prices.
    ///
    /// Updates each position's mark price and refreshes MAE/MFE.
    /// Also updates the peak equity high-water mark for drawdown
    /// calculation.
    pub fn mark_to_market(&mut self, prices: &HashMap<FuturesTicker, Price>) {
        for (ticker, pos) in &mut self.positions {
            if let Some(price) = prices.get(ticker) {
                pos.update_mark(*price);
            }
        }
        let eq = self.total_equity();
        if eq > self.peak_equity {
            self.peak_equity = eq;
        }
    }

    /// Current drawdown as a percentage of peak equity.
    ///
    /// # Formula
    ///
    /// `(peak_equity - current_equity) / peak_equity * 100`
    ///
    /// Returns 0.0 if peak equity is zero or negative.
    #[must_use]
    pub fn current_drawdown_pct(&self) -> f64 {
        let eq = self.total_equity();
        if self.peak_equity > 0.0 {
            (self.peak_equity - eq) / self.peak_equity * 100.0
        } else {
            0.0
        }
    }

    /// Whether the portfolio has an open position in the given
    /// instrument.
    #[must_use]
    pub fn has_position(&self, instrument: &FuturesTicker) -> bool {
        self.positions
            .get(instrument)
            .is_some_and(|p| p.quantity > 0.0)
    }

    /// Reset the portfolio to its initial state.
    ///
    /// Clears all positions, resets cash to the initial equity, and
    /// zeroes out cumulative PnL and the trade counter.
    pub fn reset(&mut self) {
        self.cash = self.initial_equity;
        self.realized_pnl = 0.0;
        self.positions.clear();
        self.peak_equity = self.initial_equity;
        self.trade_index = 0;
    }
}
