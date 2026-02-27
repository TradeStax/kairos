//! Tick-count-based aggregation: groups of N trades produce one candle.
//!
//! Each chunk of `tick_count` consecutive trades is aggregated into a single
//! OHLCV candle. The last chunk may contain fewer trades if the total is not
//! evenly divisible.

use super::AggregationError;
use crate::domain::{Candle, Price, Trade, Volume};

/// Aggregates trades into tick-count-based candles (N trades per candle).
///
/// The `tick_count` must be non-zero. Prices are rounded to `tick_size`.
/// In debug builds, unsorted trades return an error.
pub fn aggregate_trades_to_ticks(
    trades: &[Trade],
    tick_count: u32,
    tick_size: Price,
) -> Result<Vec<Candle>, AggregationError> {
    if trades.is_empty() {
        return Ok(Vec::new());
    }

    if tick_count == 0 {
        return Err(AggregationError::InvalidTickCount(tick_count));
    }

    #[cfg(debug_assertions)]
    for window in trades.windows(2) {
        if window[0].time > window[1].time {
            return Err(AggregationError::UnsortedTrades);
        }
    }

    let mut candles = Vec::new();
    let tick_count = tick_count as usize;

    for chunk in trades.chunks(tick_count) {
        let time = chunk[0].time;
        let candle = build_candle_from_chunk(time, chunk, tick_size);
        candles.push(candle);
    }

    Ok(candles)
}

/// Builds a single candle from a chunk of trades.
fn build_candle_from_chunk(
    time: crate::domain::Timestamp,
    trades: &[Trade],
    tick_size: Price,
) -> Candle {
    let open = trades[0].price.round_to_tick(tick_size);
    let high = trades
        .iter()
        .map(|t| t.price)
        .max()
        .unwrap()
        .round_to_tick(tick_size);
    let low = trades
        .iter()
        .map(|t| t.price)
        .min()
        .unwrap()
        .round_to_tick(tick_size);
    let close = trades.last().unwrap().price.round_to_tick(tick_size);

    let mut buy_volume = 0.0;
    let mut sell_volume = 0.0;

    for trade in trades {
        if trade.is_buy() {
            buy_volume += trade.quantity.value();
        } else {
            sell_volume += trade.quantity.value();
        }
    }

    Candle::new(
        time,
        open,
        high,
        low,
        close,
        Volume(buy_volume),
        Volume(sell_volume),
    )
    .expect("invariant: high = max, low = min guarantees OHLC constraints")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Price, Quantity, Side, Timestamp, Trade};

    #[test]
    fn test_aggregate_trades_to_ticks() {
        let trades = vec![
            Trade::new(
                Timestamp(1000),
                Price::from_f32(100.0),
                Quantity(10.0),
                Side::Buy,
            ),
            Trade::new(
                Timestamp(1500),
                Price::from_f32(101.0),
                Quantity(5.0),
                Side::Sell,
            ),
            Trade::new(
                Timestamp(2000),
                Price::from_f32(99.5),
                Quantity(8.0),
                Side::Sell,
            ),
            Trade::new(
                Timestamp(61000),
                Price::from_f32(102.0),
                Quantity(12.0),
                Side::Buy,
            ),
            Trade::new(
                Timestamp(62000),
                Price::from_f32(101.5),
                Quantity(7.0),
                Side::Buy,
            ),
        ];

        let tick_size = Price::from_f32(0.01);
        let candles = aggregate_trades_to_ticks(&trades, 2, tick_size).unwrap();

        assert_eq!(candles.len(), 3);
        assert_eq!(candles[0].open.to_f32(), 100.0);
        assert_eq!(candles[0].close.to_f32(), 101.0);
        assert_eq!(candles[1].open.to_f32(), 99.5);
        assert_eq!(candles[1].close.to_f32(), 102.0);
        assert_eq!(candles[2].open.to_f32(), 101.5);
        assert_eq!(candles[2].close.to_f32(), 101.5);
    }
}
