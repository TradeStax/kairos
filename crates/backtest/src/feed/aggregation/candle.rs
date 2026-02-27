//! Single-timeframe candle aggregation from trade ticks.
//!
//! [`CandleAggregator`] buckets trades into fixed-duration time
//! windows and produces OHLCV [`Candle`]s. Each trade is assigned
//! to a bucket by truncating its timestamp to the nearest multiple
//! of the timeframe duration. When a trade falls into a new bucket,
//! the previous bucket is closed and emitted as a completed candle.

use kairos_data::{Candle, Price, Side, Timestamp, Trade, Volume};

/// An in-progress candle that has not yet closed.
///
/// Exposed so callers can inspect the current bar state for
/// real-time display during a backtest replay.
#[derive(Debug, Clone)]
pub struct PartialCandle {
    /// Start of the time bucket (milliseconds since epoch).
    pub bucket_start: u64,
    /// Opening price of the candle.
    pub open: Price,
    /// Highest price seen so far.
    pub high: Price,
    /// Lowest price seen so far.
    pub low: Price,
    /// Most recent trade price.
    pub close: Price,
    /// Cumulative buy-side volume.
    pub buy_volume: f64,
    /// Cumulative sell-side volume.
    pub sell_volume: f64,
}

/// Aggregates trade ticks into OHLCV candles at a fixed timeframe.
///
/// # Usage
///
/// 1. Create with [`CandleAggregator::new`], passing the timeframe
///    duration in milliseconds.
/// 2. Feed trades via [`update`](Self::update). Each call returns
///    `Some(Candle)` when the previous time bucket closes.
/// 3. Call [`flush`](Self::flush) at the end of data to emit any
///    remaining partial candle.
pub struct CandleAggregator {
    timeframe_ms: u64,
    partial: Option<PartialCandle>,
}

impl CandleAggregator {
    /// Creates a new aggregator for the given timeframe.
    ///
    /// `timeframe_ms` is the candle duration in milliseconds
    /// (e.g., 60_000 for 1-minute candles).
    #[must_use]
    pub fn new(timeframe_ms: u64) -> Self {
        Self {
            timeframe_ms,
            partial: None,
        }
    }

    /// Feeds a trade tick into the aggregator.
    ///
    /// Returns `Some(Candle)` if this trade crossed into a new time
    /// bucket, closing the previous candle. The trade itself is
    /// always incorporated into the current (or new) partial candle.
    pub fn update(&mut self, trade: &Trade) -> Option<Candle> {
        let bucket = (trade.time.0 / self.timeframe_ms) * self.timeframe_ms;

        match &mut self.partial {
            None => {
                self.start_new_bar(trade, bucket);
                None
            }
            Some(bar) if bar.bucket_start == bucket => {
                Self::update_bar(bar, trade);
                None
            }
            Some(_) => {
                let closed = self.close_bar();
                self.start_new_bar(trade, bucket);
                closed
            }
        }
    }

    /// Flushes the current partial candle, if any.
    ///
    /// Call this at the end of a data stream to emit the final
    /// in-progress candle as a completed candle.
    pub fn flush(&mut self) -> Option<Candle> {
        self.close_bar()
    }

    /// Returns a reference to the in-progress partial candle.
    #[must_use]
    pub fn partial(&self) -> Option<&PartialCandle> {
        self.partial.as_ref()
    }

    /// Starts a new partial candle from the given trade.
    fn start_new_bar(&mut self, trade: &Trade, bucket: u64) {
        let (buy_vol, sell_vol) = match trade.side {
            Side::Buy | Side::Bid => (trade.quantity.0, 0.0),
            Side::Sell | Side::Ask => (0.0, trade.quantity.0),
        };
        self.partial = Some(PartialCandle {
            bucket_start: bucket,
            open: trade.price,
            high: trade.price,
            low: trade.price,
            close: trade.price,
            buy_volume: buy_vol,
            sell_volume: sell_vol,
        });
    }

    /// Updates an existing partial candle with a new trade.
    fn update_bar(bar: &mut PartialCandle, trade: &Trade) {
        if trade.price > bar.high {
            bar.high = trade.price;
        }
        if trade.price < bar.low {
            bar.low = trade.price;
        }
        bar.close = trade.price;
        match trade.side {
            Side::Buy | Side::Bid => {
                bar.buy_volume += trade.quantity.0;
            }
            Side::Sell | Side::Ask => {
                bar.sell_volume += trade.quantity.0;
            }
        }
    }

    /// Closes the current partial candle and returns it.
    fn close_bar(&mut self) -> Option<Candle> {
        self.partial.take().and_then(|bar| {
            Candle::new(
                Timestamp(bar.bucket_start),
                bar.open,
                bar.high,
                bar.low,
                bar.close,
                Volume(bar.buy_volume),
                Volume(bar.sell_volume),
            )
            .ok()
        })
    }
}
