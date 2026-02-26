use crate::config::instrument::InstrumentSpec;
use crate::order::types::OrderSide;
use kairos_data::Price;

/// Compute PnL in ticks for a completed trade.
pub fn pnl_ticks(side: OrderSide, entry_price: Price, exit_price: Price, tick_size: Price) -> i64 {
    let tick_units = tick_size.units().max(1);
    let diff = match side {
        OrderSide::Buy => exit_price.units() - entry_price.units(),
        OrderSide::Sell => entry_price.units() - exit_price.units(),
    };
    diff / tick_units
}

/// Compute gross PnL in USD for a completed trade.
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

/// Compute total commission for a round trip (entry + exit).
pub fn round_trip_commission(quantity: f64, commission_per_side: f64) -> f64 {
    commission_per_side * 2.0 * quantity
}
