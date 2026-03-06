//! Shared test constructors for study unit tests.
//!
//! Provides standardized `make_candle`, `make_input`, and `make_trade`
//! helpers so that individual study test modules don't need to
//! duplicate these constructors.

use data::{Candle, ChartBasis, Price, Quantity, Side, Timeframe, Timestamp, Trade, Volume};

use crate::core::StudyInput;

/// Build a simple test candle with OHLC all equal to `close`.
pub fn make_candle(time: u64, close: f32) -> Candle {
    Candle::new(
        Timestamp(time),
        Price::from_f32(close),
        Price::from_f32(close),
        Price::from_f32(close),
        Price::from_f32(close),
        Volume(0.0),
        Volume(0.0),
    )
    .expect("test candle")
}

/// Build a test candle with explicit OHLC and buy/sell volume.
pub fn make_candle_ohlcv(
    time: u64,
    o: f32,
    h: f32,
    l: f32,
    c: f32,
    buy_vol: f64,
    sell_vol: f64,
) -> Candle {
    Candle::new(
        Timestamp::from_millis(time),
        Price::from_f32(o),
        Price::from_f32(h),
        Price::from_f32(l),
        Price::from_f32(c),
        Volume(buy_vol),
        Volume(sell_vol),
    )
    .expect("test candle")
}

/// Build a `StudyInput` from candles only (no trades, M1 timeframe).
pub fn make_input(candles: &[Candle]) -> StudyInput<'_> {
    StudyInput {
        candles,
        trades: None,
        basis: ChartBasis::Time(Timeframe::M1),
        tick_size: Price::from_f32(0.25),
        visible_range: None,
    }
}

/// Build a `StudyInput` with both candles and trades.
pub fn make_input_with_trades<'a>(candles: &'a [Candle], trades: &'a [Trade]) -> StudyInput<'a> {
    StudyInput {
        candles,
        trades: Some(trades),
        basis: ChartBasis::Time(Timeframe::M1),
        tick_size: Price::from_f32(0.25),
        visible_range: None,
    }
}

/// Build a test trade at the given time, price, quantity, and side.
pub fn make_trade(time: u64, price: f32, qty: f64, side: Side) -> Trade {
    Trade::new(
        Timestamp::from_millis(time),
        Price::from_f32(price),
        Quantity(qty),
        side,
    )
}
