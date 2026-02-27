//! Chart data aggregation operations.
//!
//! Pure functions for building [`ChartData`] from trades. No I/O — these
//! operate on in-memory trade slices and return aggregated candles.

use crate::aggregation::{
    AggregationError, aggregate_trades_to_candles, aggregate_trades_to_ticks,
};
use crate::domain::{Candle, ChartBasis, ChartData, FuturesTickerInfo, Price, Trade};

/// Rebuilds chart data from existing trades (instant, no I/O).
///
/// Aggregates `trades` into candles according to `basis` (time or tick),
/// using the tick size from `ticker_info` for price rounding.
pub fn rebuild_chart_data(
    trades: &[Trade],
    basis: ChartBasis,
    ticker_info: &FuturesTickerInfo,
) -> Result<ChartData, AggregationError> {
    let tick_size = Price::from_f32(ticker_info.tick_size);
    let candles = aggregate_to_basis(trades, basis, tick_size)?;
    Ok(ChartData::from_trades(trades.to_vec(), candles))
}

/// Aggregates trades into candles for the specified chart basis.
///
/// Dispatches to time-based or tick-based aggregation depending on `basis`.
pub fn aggregate_to_basis(
    trades: &[Trade],
    basis: ChartBasis,
    tick_size: Price,
) -> Result<Vec<Candle>, AggregationError> {
    match basis {
        ChartBasis::Time(timeframe) => {
            aggregate_trades_to_candles(trades, timeframe.to_milliseconds(), tick_size)
        }
        ChartBasis::Tick(tick_count) => aggregate_trades_to_ticks(trades, tick_count, tick_size),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        ChartBasis, FuturesTicker, FuturesTickerInfo, FuturesVenue, Quantity, Side, Timestamp,
        Trade,
    };

    #[test]
    fn test_rebuild_chart_data_time_basis() {
        let trades = vec![
            Trade::new(
                Timestamp(1000),
                Price::from_f32(100.0),
                Quantity(10.0),
                Side::Buy,
            ),
            Trade::new(
                Timestamp(2000),
                Price::from_f32(101.0),
                Quantity(5.0),
                Side::Sell,
            ),
            Trade::new(
                Timestamp(61000),
                Price::from_f32(102.0),
                Quantity(8.0),
                Side::Buy,
            ),
        ];

        let ticker_info = FuturesTickerInfo::new(
            FuturesTicker::new("ES.c.0", FuturesVenue::CMEGlobex),
            0.25,
            1.0,
            50.0,
        );

        let result = rebuild_chart_data(
            &trades,
            ChartBasis::Time(crate::domain::Timeframe::M1),
            &ticker_info,
        );

        assert!(result.is_ok());
        let chart_data = result.unwrap();
        assert_eq!(chart_data.trades.len(), 3);
        assert_eq!(chart_data.candles.len(), 2); // 2 minutes
    }

    #[test]
    fn test_rebuild_chart_data_tick_basis() {
        let trades = vec![
            Trade::new(
                Timestamp(1000),
                Price::from_f32(100.0),
                Quantity(10.0),
                Side::Buy,
            ),
            Trade::new(
                Timestamp(2000),
                Price::from_f32(101.0),
                Quantity(5.0),
                Side::Sell,
            ),
            Trade::new(
                Timestamp(3000),
                Price::from_f32(99.5),
                Quantity(8.0),
                Side::Sell,
            ),
        ];

        let ticker_info = FuturesTickerInfo::new(
            FuturesTicker::new("ES.c.0", FuturesVenue::CMEGlobex),
            0.25,
            1.0,
            50.0,
        );

        let result = rebuild_chart_data(&trades, ChartBasis::Tick(2), &ticker_info);

        assert!(result.is_ok());
        let chart_data = result.unwrap();
        assert_eq!(chart_data.trades.len(), 3);
        assert_eq!(chart_data.candles.len(), 2); // 3 trades / 2 per candle = 2
    }
}
