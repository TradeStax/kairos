//! Time-based aggregation: trades to candles, candles to higher timeframes.
//!
//! Trades are bucketed by `timeframe_millis` intervals. Each bucket produces
//! one OHLCV candle with separate buy/sell volume tracking.

use super::AggregationError;
use crate::domain::{Candle, Price, Timestamp, Trade, Volume};
use std::collections::BTreeMap;

/// Aggregates tick-by-tick trades into time-based OHLCV candles.
///
/// The `timeframe_millis` must be at least 1000ms. Prices are rounded to
/// the nearest `tick_size`. In debug builds, unsorted trades return an error.
pub fn aggregate_trades_to_candles(
    trades: &[Trade],
    timeframe_millis: u64,
    tick_size: Price,
) -> Result<Vec<Candle>, AggregationError> {
    if trades.is_empty() {
        return Ok(Vec::new());
    }

    if timeframe_millis == 0 {
        return Err(AggregationError::InvalidTimeframe(timeframe_millis));
    }

    const MIN_TIMEFRAME_MS: u64 = 1_000;
    if timeframe_millis < MIN_TIMEFRAME_MS {
        return Err(AggregationError::InvalidTimeframe(timeframe_millis));
    }

    #[cfg(debug_assertions)]
    for window in trades.windows(2) {
        if window[0].time > window[1].time {
            return Err(AggregationError::UnsortedTrades);
        }
    }

    let mut buckets: BTreeMap<u64, Vec<&Trade>> = BTreeMap::new();

    for trade in trades {
        let bucket_time = (trade.time.to_millis() / timeframe_millis) * timeframe_millis;
        buckets.entry(bucket_time).or_default().push(trade);
    }

    let mut candles = Vec::with_capacity(buckets.len());

    for (bucket_time, bucket_trades) in buckets {
        let candle = build_candle_from_trades(
            Timestamp::from_millis(bucket_time),
            bucket_trades,
            tick_size,
        );
        candles.push(candle);
    }

    Ok(candles)
}

/// Aggregates candles from one timeframe to a higher timeframe.
///
/// Candles are bucketed by `target_timeframe_millis`. OHLC is computed from
/// the first/last/max/min candle in each bucket; volumes are summed.
pub fn aggregate_candles_to_timeframe(
    candles: &[Candle],
    target_timeframe_millis: u64,
) -> Result<Vec<Candle>, AggregationError> {
    if candles.is_empty() {
        return Ok(Vec::new());
    }

    if target_timeframe_millis == 0 {
        return Err(AggregationError::InvalidTimeframe(target_timeframe_millis));
    }

    let mut buckets: BTreeMap<u64, Vec<&Candle>> = BTreeMap::new();

    for candle in candles {
        let bucket_time =
            (candle.time.to_millis() / target_timeframe_millis) * target_timeframe_millis;
        buckets.entry(bucket_time).or_default().push(candle);
    }

    let mut result = Vec::with_capacity(buckets.len());

    for (bucket_time, bucket_candles) in buckets {
        let aggregated =
            aggregate_candle_bucket(Timestamp::from_millis(bucket_time), bucket_candles);
        result.push(aggregated);
    }

    Ok(result)
}

/// Builds a single candle from a bucket of trades, rounding prices to tick size.
fn build_candle_from_trades(time: Timestamp, trades: Vec<&Trade>, tick_size: Price) -> Candle {
    if trades.is_empty() {
        return Candle::new(
            time,
            Price::from_f32(0.0),
            Price::from_f32(0.0),
            Price::from_f32(0.0),
            Price::from_f32(0.0),
            Volume(0.0),
            Volume(0.0),
        )
        .expect("invariant: zero price for all fields satisfies OHLC constraints");
    }

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
    .expect("invariant: high = max price, low = min price guarantees OHLC constraints")
}

/// Aggregates a bucket of candles into a single higher-timeframe candle.
fn aggregate_candle_bucket(time: Timestamp, candles: Vec<&Candle>) -> Candle {
    if candles.is_empty() {
        return Candle::new(
            time,
            Price::from_f32(0.0),
            Price::from_f32(0.0),
            Price::from_f32(0.0),
            Price::from_f32(0.0),
            Volume(0.0),
            Volume(0.0),
        )
        .expect("invariant: zero price for all fields satisfies OHLC constraints");
    }

    let open = candles[0].open;
    let high = candles.iter().map(|c| c.high).max().unwrap();
    let low = candles.iter().map(|c| c.low).min().unwrap();
    let close = candles.last().unwrap().close;
    let total_buy_volume = candles.iter().map(|c| c.buy_volume.value()).sum();
    let total_sell_volume = candles.iter().map(|c| c.sell_volume.value()).sum();

    Candle::new(
        time,
        open,
        high,
        low,
        close,
        Volume(total_buy_volume),
        Volume(total_sell_volume),
    )
    .expect("invariant: high = max of candle highs, low = min of candle lows")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Quantity, Side};

    fn create_test_trades() -> Vec<Trade> {
        vec![
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
        ]
    }

    #[test]
    fn test_aggregate_trades_to_candles() {
        let trades = create_test_trades();
        let timeframe = 60_000;
        let tick_size = Price::from_f32(0.01);

        let candles = aggregate_trades_to_candles(&trades, timeframe, tick_size).unwrap();

        assert_eq!(candles.len(), 2);

        assert_eq!(candles[0].open.to_f32(), 100.0);
        assert_eq!(candles[0].high.to_f32(), 101.0);
        assert_eq!(candles[0].low.to_f32(), 99.5);
        assert_eq!(candles[0].close.to_f32(), 99.5);
        assert_eq!(candles[0].buy_volume.value(), 10.0);
        assert_eq!(candles[0].sell_volume.value(), 13.0);

        assert_eq!(candles[1].open.to_f32(), 102.0);
        assert_eq!(candles[1].close.to_f32(), 101.5);
        assert_eq!(candles[1].buy_volume.value(), 19.0);
    }

    #[test]
    fn test_aggregate_candles_to_timeframe() {
        let candles = vec![
            Candle::new(
                Timestamp(0),
                Price::from_f32(100.0),
                Price::from_f32(101.0),
                Price::from_f32(99.0),
                Price::from_f32(100.5),
                Volume(10.0),
                Volume(5.0),
            )
            .unwrap(),
            Candle::new(
                Timestamp(60_000),
                Price::from_f32(100.5),
                Price::from_f32(102.0),
                Price::from_f32(100.0),
                Price::from_f32(101.5),
                Volume(8.0),
                Volume(6.0),
            )
            .unwrap(),
            Candle::new(
                Timestamp(120_000),
                Price::from_f32(101.5),
                Price::from_f32(103.0),
                Price::from_f32(101.0),
                Price::from_f32(102.0),
                Volume(12.0),
                Volume(7.0),
            )
            .unwrap(),
        ];

        let aggregated = aggregate_candles_to_timeframe(&candles, 180_000).unwrap();

        assert_eq!(aggregated.len(), 1);
        assert_eq!(aggregated[0].open.to_f32(), 100.0);
        assert_eq!(aggregated[0].high.to_f32(), 103.0);
        assert_eq!(aggregated[0].low.to_f32(), 99.0);
        assert_eq!(aggregated[0].close.to_f32(), 102.0);
        assert_eq!(aggregated[0].buy_volume.value(), 30.0);
        assert_eq!(aggregated[0].sell_volume.value(), 18.0);
    }

    #[test]
    fn test_empty_trades() {
        let trades: Vec<Trade> = vec![];
        let result = aggregate_trades_to_candles(&trades, 60_000, Price::from_f32(0.01)).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_unsorted_trades_error() {
        let trades = vec![
            Trade::new(
                Timestamp(2000),
                Price::from_f32(100.0),
                Quantity(10.0),
                Side::Buy,
            ),
            Trade::new(
                Timestamp(1000),
                Price::from_f32(101.0),
                Quantity(5.0),
                Side::Sell,
            ),
        ];

        let result = aggregate_trades_to_candles(&trades, 60_000, Price::from_f32(0.01));
        assert!(matches!(result, Err(AggregationError::UnsortedTrades)));
    }

    #[test]
    fn test_invalid_timeframe() {
        let trades = create_test_trades();
        let result = aggregate_trades_to_candles(&trades, 0, Price::from_f32(0.01));
        assert!(matches!(result, Err(AggregationError::InvalidTimeframe(0))));
    }
}
