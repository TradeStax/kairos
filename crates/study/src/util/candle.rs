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

#[cfg(test)]
mod tests {
    use super::*;
    use data::{Price, Timeframe, Timestamp, Volume};

    fn make_ohlc_candle(open: f32, high: f32, low: f32, close: f32) -> Candle {
        Candle::new(
            Timestamp(1_000_000),
            Price::from_f32(open),
            Price::from_f32(high),
            Price::from_f32(low),
            Price::from_f32(close),
            Volume(50.0),
            Volume(30.0),
        )
        .expect("test candle")
    }

    // ── source_value ────────────────────────────────────────

    #[test]
    fn source_value_close_default() {
        let c = make_ohlc_candle(100.0, 110.0, 90.0, 105.0);
        let v = source_value(&c, "Close");
        assert!((v - 105.0).abs() < 0.01);
    }

    #[test]
    fn source_value_open() {
        let c = make_ohlc_candle(100.0, 110.0, 90.0, 105.0);
        assert!((source_value(&c, "Open") - 100.0).abs() < 0.01);
    }

    #[test]
    fn source_value_high() {
        let c = make_ohlc_candle(100.0, 110.0, 90.0, 105.0);
        assert!((source_value(&c, "High") - 110.0).abs() < 0.01);
    }

    #[test]
    fn source_value_low() {
        let c = make_ohlc_candle(100.0, 110.0, 90.0, 105.0);
        assert!((source_value(&c, "Low") - 90.0).abs() < 0.01);
    }

    #[test]
    fn source_value_hl2() {
        let c = make_ohlc_candle(100.0, 110.0, 90.0, 105.0);
        // HL2 = (110 + 90) / 2 = 100
        assert!((source_value(&c, "HL2") - 100.0).abs() < 0.01);
    }

    #[test]
    fn source_value_hlc3() {
        let c = make_ohlc_candle(100.0, 110.0, 90.0, 105.0);
        // HLC3 = (110 + 90 + 105) / 3 ≈ 101.67
        let expected = (110.0 + 90.0 + 105.0) / 3.0;
        assert!((source_value(&c, "HLC3") - expected).abs() < 0.1);
    }

    #[test]
    fn source_value_ohlc4() {
        let c = make_ohlc_candle(100.0, 110.0, 90.0, 105.0);
        // OHLC4 = (100 + 110 + 90 + 105) / 4 = 101.25
        let expected = (100.0 + 110.0 + 90.0 + 105.0) / 4.0;
        assert!((source_value(&c, "OHLC4") - expected).abs() < 0.1);
    }

    #[test]
    fn source_value_unknown_falls_back_to_close() {
        let c = make_ohlc_candle(100.0, 110.0, 90.0, 105.0);
        assert!((source_value(&c, "Unknown") - 105.0).abs() < 0.01);
        assert!((source_value(&c, "") - 105.0).abs() < 0.01);
    }

    // ── candle_key ──────────────────────────────────────────

    #[test]
    fn candle_key_time_based_returns_timestamp() {
        let c = Candle::new(
            Timestamp(1_700_000_000),
            Price::from_f32(100.0),
            Price::from_f32(100.0),
            Price::from_f32(100.0),
            Price::from_f32(100.0),
            Volume(0.0),
            Volume(0.0),
        )
        .unwrap();
        let basis = ChartBasis::Time(Timeframe::M1);
        assert_eq!(candle_key(&c, 0, 10, &basis), 1_700_000_000);
    }

    #[test]
    fn candle_key_tick_based_returns_reverse_index() {
        let c = make_ohlc_candle(100.0, 100.0, 100.0, 100.0);
        let basis = ChartBasis::Tick(100);
        // total=10, index=0 => 10-1-0 = 9 (oldest has highest key)
        assert_eq!(candle_key(&c, 0, 10, &basis), 9);
        // total=10, index=9 => 10-1-9 = 0 (newest has key 0)
        assert_eq!(candle_key(&c, 9, 10, &basis), 0);
        // total=10, index=5 => 10-1-5 = 4
        assert_eq!(candle_key(&c, 5, 10, &basis), 4);
    }

    #[test]
    fn candle_key_tick_based_single_candle() {
        let c = make_ohlc_candle(100.0, 100.0, 100.0, 100.0);
        let basis = ChartBasis::Tick(50);
        // total=1, index=0 => 1-1-0 = 0
        assert_eq!(candle_key(&c, 0, 1, &basis), 0);
    }

    #[test]
    fn candle_key_tick_based_saturates_on_overflow() {
        let c = make_ohlc_candle(100.0, 100.0, 100.0, 100.0);
        let basis = ChartBasis::Tick(50);
        // total=0, index=0 => saturating_sub(1) = 0, then sub(0) = 0
        assert_eq!(candle_key(&c, 0, 0, &basis), 0);
    }
}
