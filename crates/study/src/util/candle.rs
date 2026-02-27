//! Candle data extraction helpers shared across studies.
//!
//! [`source_value`] maps a price-source name (`"Close"`, `"HL2"`, etc.)
//! to the corresponding `f32` value from a candle. [`candle_key`]
//! produces the x-coordinate used to place study points on the chart,
//! adapting for both time-based and tick-based aggregation modes.

use data::{Candle, ChartBasis};

/// Extract a price value from a candle by source name.
///
/// Supported sources:
/// - `"Open"`, `"High"`, `"Low"` — single OHLC field.
/// - `"HL2"` — midpoint of high and low.
/// - `"HLC3"` — typical price (high + low + close) / 3.
/// - `"OHLC4"` — average of all four OHLC fields.
/// - Any other string — falls back to `"Close"`.
pub fn source_value(candle: &Candle, source: &str) -> f32 {
    match source {
        "Open" => candle.open.to_f32(),
        "High" => candle.high.to_f32(),
        "Low" => candle.low.to_f32(),
        "HL2" => (candle.high.to_f32() + candle.low.to_f32()) / 2.0,
        "HLC3" => (candle.high.to_f32() + candle.low.to_f32() + candle.close.to_f32()) / 3.0,
        "OHLC4" => {
            (candle.open.to_f32()
                + candle.high.to_f32()
                + candle.low.to_f32()
                + candle.close.to_f32())
                / 4.0
        }
        _ => candle.close.to_f32(),
    }
}

/// Compute the x-coordinate for a candle on the chart.
///
/// - **Time-based** charts: returns the candle's timestamp directly.
/// - **Tick-based** charts: returns the reverse index (`0` = newest)
///   to match the chart rendering coordinate system.
pub fn candle_key(candle: &Candle, index: usize, total_candles: usize, basis: &ChartBasis) -> u64 {
    match basis {
        ChartBasis::Time(_) => candle.time.0,
        ChartBasis::Tick(_) => total_candles.saturating_sub(1).saturating_sub(index) as u64,
    }
}
