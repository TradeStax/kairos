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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::instrument::InstrumentSpec;
    use crate::order::types::OrderSide;
    use kairos_data::{FuturesTicker, FuturesVenue, Price, Timestamp};

    fn es_ticker() -> FuturesTicker {
        FuturesTicker::new("ES.c.0", FuturesVenue::CMEGlobex)
    }

    fn es_spec() -> InstrumentSpec {
        InstrumentSpec::new(es_ticker(), Price::from_f64(0.25), 50.0)
    }

    fn make_instruments() -> HashMap<FuturesTicker, InstrumentSpec> {
        let mut m = HashMap::new();
        m.insert(es_ticker(), es_spec());
        m
    }

    fn make_portfolio(initial: f64, commission: f64) -> Portfolio {
        Portfolio::new(initial, make_instruments(), commission, None)
    }

    fn ts(ms: u64) -> Timestamp {
        Timestamp(ms)
    }

    // ── Initial state ────────────────────────────────────────────

    #[test]
    fn test_initial_state() {
        let p = make_portfolio(100_000.0, 2.50);
        assert!((p.cash() - 100_000.0).abs() < 1e-10);
        assert!((p.realized_pnl() - 0.0).abs() < 1e-10);
        assert!((p.total_equity() - 100_000.0).abs() < 1e-10);
        assert!(p.positions().is_empty());
    }

    // ── Open long position ───────────────────────────────────────

    #[test]
    fn test_open_long_position() {
        let mut p = make_portfolio(100_000.0, 2.50);
        let result = p.process_fill(
            es_ticker(),
            OrderSide::Buy,
            Price::from_f64(5000.0),
            1.0,
            ts(1000),
            None,
            None,
        );

        assert!(result.is_none()); // Opening fill, no trade record
        assert!(p.has_position(&es_ticker()));
        // Commission deducted on entry: 2.50 * 1 = 2.50
        assert!((p.cash() - (100_000.0 - 2.50)).abs() < 1e-10);
    }

    // ── Close long position (profit) ─────────────────────────────

    #[test]
    fn test_close_long_with_profit() {
        let mut p = make_portfolio(100_000.0, 2.50);
        // Buy 1 ES at 5000.00
        p.process_fill(
            es_ticker(),
            OrderSide::Buy,
            Price::from_f64(5000.0),
            1.0,
            ts(1000),
            None,
            None,
        );
        // Sell 1 ES at 5010.00 (10 points = 40 ticks * $12.50/tick = $500 gross)
        let record = p.process_fill(
            es_ticker(),
            OrderSide::Sell,
            Price::from_f64(5010.0),
            1.0,
            ts(2000),
            Some(crate::output::trade_record::ExitReason::Manual),
            None,
        );

        let record = record.expect("Should produce a trade record");
        // 10 points / 0.25 tick_size = 40 ticks
        assert_eq!(record.pnl_ticks, 40);
        // 40 ticks * $12.50/tick * 1 contract = $500
        assert!((record.pnl_gross_usd - 500.0).abs() < 1e-10);
        // Commission: 2 sides * 2.50 * 1 = 5.00
        assert!((record.commission_usd - 5.0).abs() < 1e-10);
        // Net: 500 - 5 = 495
        assert!((record.pnl_net_usd - 495.0).abs() < 1e-10);

        assert!(!p.has_position(&es_ticker()));
    }

    // ── Close long position (loss) ───────────────────────────────

    #[test]
    fn test_close_long_with_loss() {
        let mut p = make_portfolio(100_000.0, 2.50);
        p.process_fill(
            es_ticker(),
            OrderSide::Buy,
            Price::from_f64(5000.0),
            1.0,
            ts(1000),
            None,
            None,
        );
        let record = p.process_fill(
            es_ticker(),
            OrderSide::Sell,
            Price::from_f64(4990.0),
            1.0,
            ts(2000),
            Some(crate::output::trade_record::ExitReason::StopLoss),
            None,
        );

        let record = record.unwrap();
        // -10 points / 0.25 = -40 ticks
        assert_eq!(record.pnl_ticks, -40);
        assert!((record.pnl_gross_usd - (-500.0)).abs() < 1e-10);
    }

    // ── Short position ───────────────────────────────────────────

    #[test]
    fn test_short_position_profit() {
        let mut p = make_portfolio(100_000.0, 2.50);
        // Sell 1 ES at 5000.00
        p.process_fill(
            es_ticker(),
            OrderSide::Sell,
            Price::from_f64(5000.0),
            1.0,
            ts(1000),
            None,
            None,
        );
        // Buy to cover at 4990.00 (10 point profit for short)
        let record = p.process_fill(
            es_ticker(),
            OrderSide::Buy,
            Price::from_f64(4990.0),
            1.0,
            ts(2000),
            Some(crate::output::trade_record::ExitReason::TakeProfit),
            None,
        );

        let record = record.unwrap();
        // Short: (entry - exit) / tick = (5000 - 4990) / 0.25 = 40 ticks
        assert_eq!(record.pnl_ticks, 40);
        assert!((record.pnl_gross_usd - 500.0).abs() < 1e-10);
    }

    // ── Scale into position ──────────────────────────────────────

    #[test]
    fn test_scale_into_long() {
        let mut p = make_portfolio(100_000.0, 0.0); // no commission for simplicity
        // Buy 2 at 5000
        p.process_fill(
            es_ticker(),
            OrderSide::Buy,
            Price::from_f64(5000.0),
            2.0,
            ts(1000),
            None,
            None,
        );
        // Buy 1 more at 5010 => VWAP = (5000*2 + 5010*1)/3 = 5003.333...
        p.process_fill(
            es_ticker(),
            OrderSide::Buy,
            Price::from_f64(5010.0),
            1.0,
            ts(1001),
            None,
            None,
        );

        let pos = p.positions().get(&es_ticker()).unwrap();
        assert!((pos.quantity - 3.0).abs() < 1e-9);
        let expected_avg = (5000.0 * 2.0 + 5010.0 * 1.0) / 3.0;
        assert!((pos.avg_entry_price.to_f64() - expected_avg).abs() < 0.01);
    }

    // ── Multiple instruments ─────────────────────────────────────

    #[test]
    fn test_multiple_instruments() {
        let nq = FuturesTicker::new("NQ.c.0", FuturesVenue::CMEGlobex);
        let mut instruments = make_instruments();
        instruments.insert(nq, InstrumentSpec::new(nq, Price::from_f64(0.25), 20.0));
        let mut p = Portfolio::new(100_000.0, instruments, 0.0, None);

        p.process_fill(
            es_ticker(),
            OrderSide::Buy,
            Price::from_f64(5000.0),
            1.0,
            ts(1000),
            None,
            None,
        );
        p.process_fill(
            nq,
            OrderSide::Sell,
            Price::from_f64(20000.0),
            1.0,
            ts(1001),
            None,
            None,
        );

        assert!(p.has_position(&es_ticker()));
        assert!(p.has_position(&nq));
        assert_eq!(p.positions().len(), 2);
    }

    // ── Realized PnL accumulates ─────────────────────────────────

    #[test]
    fn test_realized_pnl_accumulates() {
        let mut p = make_portfolio(100_000.0, 0.0);
        // Trade 1: buy 5000, sell 5010 => +$500
        p.process_fill(
            es_ticker(),
            OrderSide::Buy,
            Price::from_f64(5000.0),
            1.0,
            ts(1000),
            None,
            None,
        );
        p.process_fill(
            es_ticker(),
            OrderSide::Sell,
            Price::from_f64(5010.0),
            1.0,
            ts(2000),
            Some(crate::output::trade_record::ExitReason::Manual),
            None,
        );
        // Trade 2: buy 5020, sell 5015 => -$250
        p.process_fill(
            es_ticker(),
            OrderSide::Buy,
            Price::from_f64(5020.0),
            1.0,
            ts(3000),
            None,
            None,
        );
        p.process_fill(
            es_ticker(),
            OrderSide::Sell,
            Price::from_f64(5015.0),
            1.0,
            ts(4000),
            Some(crate::output::trade_record::ExitReason::Manual),
            None,
        );

        // Net realized = 500 - 250 = 250
        assert!((p.realized_pnl() - 250.0).abs() < 1e-10);
    }

    // ── Commission deduction ─────────────────────────────────────

    #[test]
    fn test_commission_reduces_cash() {
        let mut p = make_portfolio(100_000.0, 2.50);
        // Buy 2 contracts => 2.50 * 2 = 5.00 entry commission
        p.process_fill(
            es_ticker(),
            OrderSide::Buy,
            Price::from_f64(5000.0),
            2.0,
            ts(1000),
            None,
            None,
        );

        assert!((p.cash() - (100_000.0 - 5.0)).abs() < 1e-10);
    }

    // ── Mark to market ───────────────────────────────────────────

    #[test]
    fn test_mark_to_market_updates_equity() {
        let mut p = make_portfolio(100_000.0, 0.0);
        p.process_fill(
            es_ticker(),
            OrderSide::Buy,
            Price::from_f64(5000.0),
            1.0,
            ts(1000),
            None,
            None,
        );

        let mut prices = HashMap::new();
        prices.insert(es_ticker(), Price::from_f64(5010.0));
        p.mark_to_market(&prices);

        // Unrealized: (5010-5000)/0.25 * 12.50 * 1 = 40 * 12.50 = 500
        assert!((p.total_equity() - 100_500.0).abs() < 1e-10);
    }

    // ── Drawdown ─────────────────────────────────────────────────

    #[test]
    fn test_drawdown_calculation() {
        let mut p = make_portfolio(100_000.0, 0.0);
        p.process_fill(
            es_ticker(),
            OrderSide::Buy,
            Price::from_f64(5000.0),
            1.0,
            ts(1000),
            None,
            None,
        );

        // Mark up to 5050 => equity = 103,125 (peak)
        let mut prices = HashMap::new();
        prices.insert(es_ticker(), Price::from_f64(5050.0));
        p.mark_to_market(&prices);

        // Mark down to 5020 => equity = 101,250
        prices.insert(es_ticker(), Price::from_f64(5020.0));
        p.mark_to_market(&prices);

        let dd = p.current_drawdown_pct();
        // peak = 103125, current = 101250
        // dd = (103125 - 101250) / 103125 * 100 = 1875/103125*100 ~ 1.818%
        assert!(dd > 1.0);
        assert!(dd < 3.0);
    }

    // ── Margin check (no margin calculator) ──────────────────────

    #[test]
    fn test_no_margin_always_passes() {
        let p = make_portfolio(100_000.0, 0.0);
        assert!(p.check_margin(&es_ticker(), 100.0));
    }

    // ── Margin check (with calculator) ───────────────────────────

    #[test]
    fn test_margin_check_with_calculator() {
        let mc = MarginCalculator::new(Some(15_000.0), Some(14_000.0));
        let p = Portfolio::new(100_000.0, make_instruments(), 0.0, Some(mc));

        // 6 contracts * 15000 = 90000 < 100000 => pass
        assert!(p.check_margin(&es_ticker(), 6.0));
        // 7 contracts * 15000 = 105000 > 100000 => fail
        assert!(!p.check_margin(&es_ticker(), 7.0));
    }

    // ── Reset ────────────────────────────────────────────────────

    #[test]
    fn test_reset() {
        let mut p = make_portfolio(100_000.0, 0.0);
        p.process_fill(
            es_ticker(),
            OrderSide::Buy,
            Price::from_f64(5000.0),
            1.0,
            ts(1000),
            None,
            None,
        );

        p.reset();

        assert!((p.cash() - 100_000.0).abs() < 1e-10);
        assert!((p.realized_pnl() - 0.0).abs() < 1e-10);
        assert!(p.positions().is_empty());
    }

    // ── Buying power with margin ─────────────────────────────────

    #[test]
    fn test_buying_power_with_positions() {
        let mc = MarginCalculator::new(Some(15_000.0), Some(14_000.0));
        let mut p = Portfolio::new(100_000.0, make_instruments(), 0.0, Some(mc));

        p.process_fill(
            es_ticker(),
            OrderSide::Buy,
            Price::from_f64(5000.0),
            2.0,
            ts(1000),
            None,
            None,
        );

        // Buying power = cash - maintenance_margin_used
        // = 100000 - (2 * 14000) = 100000 - 28000 = 72000
        assert!((p.buying_power() - 72_000.0).abs() < 1e-10);
    }

    // ── Trade index increments ───────────────────────────────────

    #[test]
    fn test_trade_index_increments() {
        let mut p = make_portfolio(100_000.0, 0.0);

        // Trade 1
        p.process_fill(
            es_ticker(),
            OrderSide::Buy,
            Price::from_f64(5000.0),
            1.0,
            ts(1000),
            None,
            None,
        );
        let r1 = p
            .process_fill(
                es_ticker(),
                OrderSide::Sell,
                Price::from_f64(5010.0),
                1.0,
                ts(2000),
                Some(crate::output::trade_record::ExitReason::Manual),
                None,
            )
            .unwrap();

        // Trade 2
        p.process_fill(
            es_ticker(),
            OrderSide::Buy,
            Price::from_f64(5020.0),
            1.0,
            ts(3000),
            None,
            None,
        );
        let r2 = p
            .process_fill(
                es_ticker(),
                OrderSide::Sell,
                Price::from_f64(5030.0),
                1.0,
                ts(4000),
                Some(crate::output::trade_record::ExitReason::Manual),
                None,
            )
            .unwrap();

        assert_eq!(r1.index, 1);
        assert_eq!(r2.index, 2);
    }
}
