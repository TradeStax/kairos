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
/// Unsorted trades return an error.
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

    fn make_trade(time: u64, price: f32, qty: f64, side: Side) -> Trade {
        Trade::new(Timestamp(time), Price::from_f32(price), Quantity(qty), side)
    }

    #[test]
    fn test_aggregate_trades_to_ticks() {
        let trades = vec![
            make_trade(1000, 100.0, 10.0, Side::Buy),
            make_trade(1500, 101.0, 5.0, Side::Sell),
            make_trade(2000, 99.5, 8.0, Side::Sell),
            make_trade(61000, 102.0, 12.0, Side::Buy),
            make_trade(62000, 101.5, 7.0, Side::Buy),
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

    #[test]
    fn tick_count_zero_returns_error() {
        let trades = vec![make_trade(1000, 100.0, 10.0, Side::Buy)];
        let tick_size = Price::from_f32(0.25);
        let result = aggregate_trades_to_ticks(&trades, 0, tick_size);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            AggregationError::InvalidTickCount(0)
        ));
    }

    #[test]
    fn empty_input_returns_empty_vec() {
        let trades: Vec<Trade> = vec![];
        let tick_size = Price::from_f32(0.25);
        let result = aggregate_trades_to_ticks(&trades, 3, tick_size).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn single_trade_produces_one_candle() {
        let trades = vec![make_trade(1000, 50.0, 7.0, Side::Buy)];
        let tick_size = Price::from_f32(0.25);
        let candles = aggregate_trades_to_ticks(&trades, 5, tick_size).unwrap();

        assert_eq!(candles.len(), 1);
        assert_eq!(candles[0].open.to_f32(), 50.0);
        assert_eq!(candles[0].high.to_f32(), 50.0);
        assert_eq!(candles[0].low.to_f32(), 50.0);
        assert_eq!(candles[0].close.to_f32(), 50.0);
        assert!((candles[0].buy_volume.value() - 7.0).abs() < 0.01);
        assert!((candles[0].sell_volume.value()).abs() < 0.01);
    }

    #[test]
    fn non_even_divisor_produces_partial_last_candle() {
        // 7 trades with tick_count=3 -> 3 candles (3 + 3 + 1)
        let trades = vec![
            make_trade(100, 10.0, 1.0, Side::Buy),
            make_trade(200, 12.0, 2.0, Side::Sell),
            make_trade(300, 11.0, 3.0, Side::Buy),
            make_trade(400, 13.0, 4.0, Side::Buy),
            make_trade(500, 9.0, 5.0, Side::Sell),
            make_trade(600, 14.0, 6.0, Side::Buy),
            make_trade(700, 15.0, 7.0, Side::Buy),
        ];
        let tick_size = Price::from_f32(0.01);
        let candles = aggregate_trades_to_ticks(&trades, 3, tick_size).unwrap();

        assert_eq!(candles.len(), 3);

        // Candle 0: trades at 10, 12, 11 -> O=10, H=12, L=10, C=11
        assert_eq!(candles[0].open.to_f32(), 10.0);
        assert_eq!(candles[0].high.to_f32(), 12.0);
        assert_eq!(candles[0].low.to_f32(), 10.0);
        assert_eq!(candles[0].close.to_f32(), 11.0);
        // buy_vol = 1+3 = 4, sell_vol = 2
        assert!((candles[0].buy_volume.value() - 4.0).abs() < 0.01);
        assert!((candles[0].sell_volume.value() - 2.0).abs() < 0.01);

        // Candle 1: trades at 13, 9, 14 -> O=13, H=14, L=9, C=14
        assert_eq!(candles[1].open.to_f32(), 13.0);
        assert_eq!(candles[1].high.to_f32(), 14.0);
        assert_eq!(candles[1].low.to_f32(), 9.0);
        assert_eq!(candles[1].close.to_f32(), 14.0);
        // buy_vol = 4+6 = 10, sell_vol = 5
        assert!((candles[1].buy_volume.value() - 10.0).abs() < 0.01);
        assert!((candles[1].sell_volume.value() - 5.0).abs() < 0.01);

        // Candle 2: last trade only at 15 -> O=H=L=C=15
        assert_eq!(candles[2].open.to_f32(), 15.0);
        assert_eq!(candles[2].high.to_f32(), 15.0);
        assert_eq!(candles[2].low.to_f32(), 15.0);
        assert_eq!(candles[2].close.to_f32(), 15.0);
        assert!((candles[2].buy_volume.value() - 7.0).abs() < 0.01);
        assert!((candles[2].sell_volume.value()).abs() < 0.01);
    }

    #[test]
    fn ohlcv_values_verified_for_exact_chunk() {
        // tick_count=2, 4 trades -> exactly 2 candles
        let trades = vec![
            make_trade(1000, 100.0, 5.0, Side::Buy),
            make_trade(2000, 98.0, 3.0, Side::Sell),
            make_trade(3000, 99.0, 2.0, Side::Sell),
            make_trade(4000, 105.0, 8.0, Side::Buy),
        ];
        let tick_size = Price::from_f32(0.25);
        let candles = aggregate_trades_to_ticks(&trades, 2, tick_size).unwrap();

        assert_eq!(candles.len(), 2);

        // Candle 0: 100, 98 -> O=100, H=100, L=98, C=98
        assert_eq!(candles[0].open.to_f32(), 100.0);
        assert_eq!(candles[0].high.to_f32(), 100.0);
        assert_eq!(candles[0].low.to_f32(), 98.0);
        assert_eq!(candles[0].close.to_f32(), 98.0);
        assert!((candles[0].buy_volume.value() - 5.0).abs() < 0.01);
        assert!((candles[0].sell_volume.value() - 3.0).abs() < 0.01);
        assert!((candles[0].volume() - 8.0).abs() < 0.01);

        // Candle 1: 99, 105 -> O=99, H=105, L=99, C=105
        assert_eq!(candles[1].open.to_f32(), 99.0);
        assert_eq!(candles[1].high.to_f32(), 105.0);
        assert_eq!(candles[1].low.to_f32(), 99.0);
        assert_eq!(candles[1].close.to_f32(), 105.0);
        assert!((candles[1].buy_volume.value() - 8.0).abs() < 0.01);
        assert!((candles[1].sell_volume.value() - 2.0).abs() < 0.01);
    }

    #[test]
    fn all_sell_volume_in_candle() {
        let trades = vec![
            make_trade(100, 50.0, 10.0, Side::Sell),
            make_trade(200, 49.0, 20.0, Side::Sell),
        ];
        let tick_size = Price::from_f32(0.25);
        let candles = aggregate_trades_to_ticks(&trades, 2, tick_size).unwrap();

        assert_eq!(candles.len(), 1);
        assert!((candles[0].buy_volume.value()).abs() < 0.01);
        assert!((candles[0].sell_volume.value() - 30.0).abs() < 0.01);
    }

    #[test]
    fn tick_count_one_produces_one_candle_per_trade() {
        let trades = vec![
            make_trade(100, 10.0, 1.0, Side::Buy),
            make_trade(200, 20.0, 2.0, Side::Sell),
            make_trade(300, 15.0, 3.0, Side::Buy),
        ];
        let tick_size = Price::from_f32(0.01);
        let candles = aggregate_trades_to_ticks(&trades, 1, tick_size).unwrap();

        assert_eq!(candles.len(), 3);
        for (i, candle) in candles.iter().enumerate() {
            // Each candle has O=H=L=C
            assert_eq!(candle.open, candle.high);
            assert_eq!(candle.open, candle.low);
            assert_eq!(candle.open, candle.close);
            assert_eq!(candle.open.to_f32(), trades[i].price.to_f32());
        }
    }
}
