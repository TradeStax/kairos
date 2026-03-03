//! Pure accounting functions for PnL and commission calculations.
//!
//! All functions in this module are stateless and operate on
//! fixed-point [`Price`] values to avoid floating-point rounding
//! in tick arithmetic.

use crate::config::instrument::InstrumentSpec;
use crate::order::types::OrderSide;
use kairos_data::Price;

/// Compute PnL in ticks for a completed trade.
///
/// # Formula
///
/// - **Long**: `(exit_price - entry_price) / tick_size`
/// - **Short**: `(entry_price - exit_price) / tick_size`
///
/// The result is signed: positive means profit, negative means loss.
/// Division is performed in fixed-point price units to avoid
/// floating-point drift.
#[must_use]
pub fn pnl_ticks(side: OrderSide, entry_price: Price, exit_price: Price, tick_size: Price) -> i64 {
    let tick_units = tick_size.units().max(1);
    let diff = match side {
        OrderSide::Buy => exit_price.units() - entry_price.units(),
        OrderSide::Sell => entry_price.units() - exit_price.units(),
    };
    diff / tick_units
}

/// Compute gross PnL in USD for a completed trade.
///
/// # Formula
///
/// `pnl_ticks(side, entry, exit, tick_size) * tick_value * quantity`
///
/// This is the raw dollar profit/loss **before** commissions.
#[must_use]
pub fn pnl_gross_usd(
    side: OrderSide,
    entry_price: Price,
    exit_price: Price,
    quantity: f64,
    instrument: &InstrumentSpec,
) -> f64 {
    let ticks = pnl_ticks(side, entry_price, exit_price, instrument.tick_size);
    ticks as f64 * instrument.tick_value * quantity
}

/// Compute total commission for a round-trip trade (entry + exit).
///
/// # Formula
///
/// `commission_per_side * 2 * quantity`
///
/// Each side (entry and exit) is charged independently, so the total
/// is always double the per-side rate times the number of contracts.
#[must_use]
pub fn round_trip_commission(quantity: f64, commission_per_side: f64) -> f64 {
    commission_per_side * 2.0 * quantity
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::order::types::OrderSide;
    use kairos_data::Price;

    // ── pnl_ticks ────────────────────────────────────────────────

    #[test]
    fn test_pnl_ticks_long_profit() {
        // Buy at 5000, sell at 5010, tick_size = 0.25
        // (5010 - 5000) / 0.25 = 40 ticks
        let entry = Price::from_f64(5000.0);
        let exit = Price::from_f64(5010.0);
        let tick = Price::from_f64(0.25);

        assert_eq!(pnl_ticks(OrderSide::Buy, entry, exit, tick), 40);
    }

    #[test]
    fn test_pnl_ticks_long_loss() {
        let entry = Price::from_f64(5000.0);
        let exit = Price::from_f64(4990.0);
        let tick = Price::from_f64(0.25);

        assert_eq!(pnl_ticks(OrderSide::Buy, entry, exit, tick), -40);
    }

    #[test]
    fn test_pnl_ticks_short_profit() {
        // Sell at 5000, cover at 4990
        let entry = Price::from_f64(5000.0);
        let exit = Price::from_f64(4990.0);
        let tick = Price::from_f64(0.25);

        assert_eq!(pnl_ticks(OrderSide::Sell, entry, exit, tick), 40);
    }

    #[test]
    fn test_pnl_ticks_short_loss() {
        let entry = Price::from_f64(5000.0);
        let exit = Price::from_f64(5010.0);
        let tick = Price::from_f64(0.25);

        assert_eq!(pnl_ticks(OrderSide::Sell, entry, exit, tick), -40);
    }

    #[test]
    fn test_pnl_ticks_zero_movement() {
        let price = Price::from_f64(5000.0);
        let tick = Price::from_f64(0.25);

        assert_eq!(pnl_ticks(OrderSide::Buy, price, price, tick), 0);
        assert_eq!(pnl_ticks(OrderSide::Sell, price, price, tick), 0);
    }

    #[test]
    fn test_pnl_ticks_different_tick_sizes() {
        // NQ: tick_size = 0.25
        let entry = Price::from_f64(20000.0);
        let exit = Price::from_f64(20001.0);
        let tick = Price::from_f64(0.25);
        assert_eq!(pnl_ticks(OrderSide::Buy, entry, exit, tick), 4);

        // CL: tick_size = 0.01
        let entry = Price::from_f64(80.00);
        let exit = Price::from_f64(80.05);
        let tick = Price::from_f64(0.01);
        assert_eq!(pnl_ticks(OrderSide::Buy, entry, exit, tick), 5);
    }

    // ── pnl_gross_usd ────────────────────────────────────────────

    #[test]
    fn test_pnl_gross_usd_es() {
        let spec = InstrumentSpec::new(
            kairos_data::FuturesTicker::new("ES.c.0", kairos_data::FuturesVenue::CMEGlobex),
            Price::from_f64(0.25),
            50.0,
        );
        // 40 ticks * $12.50/tick * 2 contracts = $1000
        let gross = pnl_gross_usd(
            OrderSide::Buy,
            Price::from_f64(5000.0),
            Price::from_f64(5010.0),
            2.0,
            &spec,
        );
        assert!((gross - 1000.0).abs() < 1e-10);
    }

    // ── round_trip_commission ─────────────────────────────────────

    #[test]
    fn test_round_trip_commission_single_contract() {
        // 2.50 per side * 2 sides * 1 contract = 5.00
        assert!((round_trip_commission(1.0, 2.50) - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_round_trip_commission_multiple_contracts() {
        // 2.50 per side * 2 sides * 3 contracts = 15.00
        assert!((round_trip_commission(3.0, 2.50) - 15.0).abs() < 1e-10);
    }

    #[test]
    fn test_round_trip_commission_zero() {
        assert!((round_trip_commission(1.0, 0.0) - 0.0).abs() < 1e-10);
    }
}
