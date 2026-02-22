//! Candle utility functions shared across studies.

use data::{ChartBasis, Candle};

/// Extract price value from a candle based on the source parameter.
pub fn source_value(candle: &Candle, source: &str) -> f32 {
    match source {
        "Open" => candle.open.to_f32(),
        "High" => candle.high.to_f32(),
        "Low" => candle.low.to_f32(),
        "HL2" => (candle.high.to_f32() + candle.low.to_f32()) / 2.0,
        "HLC3" => {
            (candle.high.to_f32()
                + candle.low.to_f32()
                + candle.close.to_f32())
                / 3.0
        }
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

/// Get the x-key for a candle based on chart basis.
///
/// For Time basis, returns the candle timestamp.
/// For Tick basis, returns the reverse index (0 = newest candle)
/// to match the chart rendering coordinate system.
pub fn candle_key(
    candle: &Candle,
    index: usize,
    total_candles: usize,
    basis: &ChartBasis,
) -> u64 {
    match basis {
        ChartBasis::Time(_) => candle.time.0,
        ChartBasis::Tick(_) => {
            (total_candles
                .saturating_sub(1)
                .saturating_sub(index)) as u64
        }
    }
}
