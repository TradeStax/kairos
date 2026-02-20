//! Aggregation Logic - Single Source of Truth
//!
//! This module contains ALL aggregation logic for converting raw tick-by-tick
//! trades into higher-level data structures (candles, tick bars, etc.).
//!
//! ## Architecture
//!
//! ```text
//! Raw Trades (sorted by time)
//!     ↓
//! aggregate_trades_to_candles() → Time-based (M1, M5, H1, etc.)
//!     OR
//! aggregate_trades_to_ticks() → Count-based (50T, 100T, etc.)
//!     ↓
//! Candles (OHLCV with buy/sell volume)
//!     ↓
//! aggregate_candles_to_timeframe() → Higher timeframes (M1→M5, etc.)
//! ```
//!

use super::entities::{Candle, Trade};
use super::types::{Price, Timestamp, Volume};
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AggregationError {
    #[error("No trades provided")]
    NoTrades,
    #[error("Invalid timeframe: {0}")]
    InvalidTimeframe(u64),
    #[error("Invalid tick count: {0}")]
    InvalidTickCount(u32),
    #[error("Trades not sorted by time")]
    UnsortedTrades,
}

/// Aggregate tick-by-tick trades into time-based OHLCV candles
///
/// This is the CORRECT way to build candles - from actual trade data, not
/// from a separate OHLCV API endpoint.
///
/// # Arguments
/// * `trades` - Slice of trades (MUST be sorted by time)
/// * `timeframe_millis` - Candle period in milliseconds (e.g., 60000 for 1M)
/// * `tick_size` - Minimum price increment for rounding
///
/// # Returns
/// Vector of candles, one per time bucket
///
/// # Algorithm
/// 1. Group trades by time bucket (floor(trade.time / timeframe) * timeframe)
/// 2. For each bucket:
///    - Open: first trade price
///    - High: max trade price
///    - Low: min trade price
///    - Close: last trade price
///    - Buy Volume: sum of buy trades
///    - Sell Volume: sum of sell trades
///
/// # Example
/// ```
/// use flowsurface_data::domain::entities::Trade;
/// use flowsurface_data::domain::types::{Price, Quantity, Timestamp, Side};
/// use flowsurface_data::domain::aggregation::aggregate_trades_to_candles;
///
/// let trades = vec![
///     Trade::new(Timestamp(1000), Price::from_f32(100.0), Quantity(10.0), Side::Buy),
///     Trade::new(Timestamp(1500), Price::from_f32(101.0), Quantity(5.0), Side::Sell),
///     Trade::new(Timestamp(61000), Price::from_f32(102.0), Quantity(8.0), Side::Buy),
/// ];
///
/// let candles = aggregate_trades_to_candles(&trades, 60000, Price::from_f32(0.01)).unwrap();
/// assert_eq!(candles.len(), 2); // Two 1-minute candles
/// ```
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

    // Verify trades are sorted by time
    for window in trades.windows(2) {
        if window[0].time > window[1].time {
            return Err(AggregationError::UnsortedTrades);
        }
    }

    // Group trades by time bucket
    // Using BTreeMap ensures buckets are ordered chronologically
    let mut buckets: BTreeMap<u64, Vec<&Trade>> = BTreeMap::new();

    for trade in trades {
        // Floor division: bucket_time = floor(trade_time / interval) * interval
        // Example: trade at 12:34:56.789 with 5min (300000ms) interval
        //   → (754,456,789 / 300,000) * 300,000 = 754,200,000 (12:30:00.000)
        // This ensures all trades within the same interval are bucketed together
        let bucket_time = (trade.time.to_millis() / timeframe_millis) * timeframe_millis;
        buckets.entry(bucket_time).or_default().push(trade);
    }

    // Build candle for each bucket
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

/// Build a single candle from a group of trades
fn build_candle_from_trades(time: Timestamp, trades: Vec<&Trade>, tick_size: Price) -> Candle {
    assert!(!trades.is_empty(), "Cannot build candle from empty trades");

    // Open: first trade
    let open = trades[0].price.round_to_tick(tick_size);

    // High: maximum price
    let high = trades
        .iter()
        .map(|t| t.price)
        .max()
        .unwrap()
        .round_to_tick(tick_size);

    // Low: minimum price
    let low = trades
        .iter()
        .map(|t| t.price)
        .min()
        .unwrap()
        .round_to_tick(tick_size);

    // Close: last trade
    let close = trades.last().unwrap().price.round_to_tick(tick_size);

    // Volume: separate buy and sell
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
}

/// Aggregate tick-by-tick trades into tick-count-based candles
///
/// Groups trades by count (e.g., every 50 trades = 1 candle), useful for
/// tick charts and range bars.
///
/// # Arguments
/// * `trades` - Slice of trades (MUST be sorted by time)
/// * `tick_count` - Number of trades per candle
/// * `tick_size` - Minimum price increment for rounding
///
/// # Returns
/// Vector of tick candles
///
/// # Example
/// ```rust,ignore
/// let candles = aggregate_trades_to_ticks(&trades, 50, tick_size).unwrap();
/// // Each candle contains exactly 50 trades (except possibly the last one)
/// ```
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

    // Verify trades are sorted by time
    for window in trades.windows(2) {
        if window[0].time > window[1].time {
            return Err(AggregationError::UnsortedTrades);
        }
    }

    let mut candles = Vec::new();
    let tick_count = tick_count as usize;

    // Process trades in chunks of tick_count
    // chunks() splits the slice into fixed-size groups
    // Example: 100 trades with tick_count=50 → 2 candles (50 trades each)
    // Last chunk may be smaller if trade count not divisible by tick_count
    for chunk in trades.chunks(tick_count) {
        let time = chunk[0].time; // Use first trade time as candle timestamp
        let candle = build_candle_from_trades(time, chunk.iter().collect(), tick_size);
        candles.push(candle);
    }

    Ok(candles)
}

/// Aggregate candles from one timeframe to a higher timeframe
///
/// Example: M1 candles → M5 candles
///
/// # Arguments
/// * `candles` - Slice of candles (MUST be sorted by time)
/// * `target_timeframe_millis` - Target timeframe in milliseconds
///
/// # Returns
/// Vector of aggregated candles
///
/// # Note
/// This is useful for aggregating cached M1 data to any higher timeframe
/// without re-fetching trades.
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

    // Group candles by target time bucket
    let mut buckets: BTreeMap<u64, Vec<&Candle>> = BTreeMap::new();

    for candle in candles {
        let bucket_time =
            (candle.time.to_millis() / target_timeframe_millis) * target_timeframe_millis;
        buckets.entry(bucket_time).or_default().push(candle);
    }

    // Aggregate each bucket
    let mut result = Vec::with_capacity(buckets.len());

    for (bucket_time, bucket_candles) in buckets {
        let aggregated =
            aggregate_candle_bucket(Timestamp::from_millis(bucket_time), bucket_candles);
        result.push(aggregated);
    }

    Ok(result)
}

/// Aggregate a bucket of candles into a single candle
fn aggregate_candle_bucket(time: Timestamp, candles: Vec<&Candle>) -> Candle {
    assert!(!candles.is_empty(), "Cannot aggregate empty candle bucket");

    // Open: first candle's open
    let open = candles[0].open;

    // High: maximum high
    let high = candles.iter().map(|c| c.high).max().unwrap();

    // Low: minimum low
    let low = candles.iter().map(|c| c.low).min().unwrap();

    // Close: last candle's close
    let close = candles.last().unwrap().close;

    // Volume: sum of all volumes
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::types::{Quantity, Side};

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
        let timeframe = 60_000; // 1 minute
        let tick_size = Price::from_f32(0.01);

        let candles = aggregate_trades_to_candles(&trades, timeframe, tick_size).unwrap();

        assert_eq!(candles.len(), 2); // Two 1-minute candles

        // First candle (0-60s)
        assert_eq!(candles[0].open.to_f32(), 100.0);
        assert_eq!(candles[0].high.to_f32(), 101.0);
        assert_eq!(candles[0].low.to_f32(), 99.5);
        assert_eq!(candles[0].close.to_f32(), 99.5);
        assert_eq!(candles[0].buy_volume.value(), 10.0);
        assert_eq!(candles[0].sell_volume.value(), 13.0);

        // Second candle (60-120s)
        assert_eq!(candles[1].open.to_f32(), 102.0);
        assert_eq!(candles[1].close.to_f32(), 101.5);
        assert_eq!(candles[1].buy_volume.value(), 19.0);
    }

    #[test]
    fn test_aggregate_trades_to_ticks() {
        let trades = create_test_trades();
        let tick_count = 2;
        let tick_size = Price::from_f32(0.01);

        let candles = aggregate_trades_to_ticks(&trades, tick_count, tick_size).unwrap();

        assert_eq!(candles.len(), 3); // 5 trades / 2 = 3 candles (2,2,1)

        // First tick candle (trades 0-1)
        assert_eq!(candles[0].open.to_f32(), 100.0);
        assert_eq!(candles[0].close.to_f32(), 101.0);

        // Second tick candle (trades 2-3)
        assert_eq!(candles[1].open.to_f32(), 99.5);
        assert_eq!(candles[1].close.to_f32(), 102.0);

        // Third tick candle (trade 4)
        assert_eq!(candles[2].open.to_f32(), 101.5);
        assert_eq!(candles[2].close.to_f32(), 101.5);
    }

    #[test]
    fn test_aggregate_candles_to_timeframe() {
        // Create 3 one-minute candles
        let candles = vec![
            Candle::new(
                Timestamp(0),
                Price::from_f32(100.0),
                Price::from_f32(101.0),
                Price::from_f32(99.0),
                Price::from_f32(100.5),
                Volume(10.0),
                Volume(5.0),
            ),
            Candle::new(
                Timestamp(60_000),
                Price::from_f32(100.5),
                Price::from_f32(102.0),
                Price::from_f32(100.0),
                Price::from_f32(101.5),
                Volume(8.0),
                Volume(6.0),
            ),
            Candle::new(
                Timestamp(120_000),
                Price::from_f32(101.5),
                Price::from_f32(103.0),
                Price::from_f32(101.0),
                Price::from_f32(102.0),
                Volume(12.0),
                Volume(7.0),
            ),
        ];

        // Aggregate to 3-minute candles
        let aggregated = aggregate_candles_to_timeframe(&candles, 180_000).unwrap();

        assert_eq!(aggregated.len(), 1);
        assert_eq!(aggregated[0].open.to_f32(), 100.0); // First candle's open
        assert_eq!(aggregated[0].high.to_f32(), 103.0); // Max of all highs
        assert_eq!(aggregated[0].low.to_f32(), 99.0); // Min of all lows
        assert_eq!(aggregated[0].close.to_f32(), 102.0); // Last candle's close
        assert_eq!(aggregated[0].buy_volume.value(), 30.0); // Sum
        assert_eq!(aggregated[0].sell_volume.value(), 18.0); // Sum
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
                Timestamp(1000), // Out of order!
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
