//! Input data passed to [`Study::compute()`](super::Study::compute).

use data::{Candle, ChartBasis, Price, Trade};

/// Market data and chart context provided to a study for computation.
pub struct StudyInput<'a> {
    /// OHLCV candle data. Non-empty; studies may return `Empty` output if
    /// fewer candles than required are present (e.g. period not yet warm).
    pub candles: &'a [Candle],
    /// Optional raw trades for order flow studies (Big Trades, Footprint).
    /// `None` for chart-level studies that only use candle data.
    pub trades: Option<&'a [Trade]>,
    /// Chart basis (time-based or tick-based aggregation).
    pub basis: ChartBasis,
    /// Minimum price increment for the instrument.
    /// Fixed-point `Price` (10^-8 precision); guaranteed non-zero.
    pub tick_size: Price,
    /// Visible chart range as `(start, end)` interval values (both inclusive).
    /// `None` for studies that operate on the full candle history.
    /// When `Some((start, end))`, callers guarantee `start <= end`.
    pub visible_range: Option<(u64, u64)>,
}
