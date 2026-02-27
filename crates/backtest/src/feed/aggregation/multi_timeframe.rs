//! Multi-instrument, multi-timeframe candle aggregation.
//!
//! [`MultiTimeframeAggregator`] manages a collection of
//! [`CandleAggregator`]s, one per (instrument, timeframe) pair.
//! Trades are routed to all aggregators matching the trade's
//! instrument, enabling a single trade stream to produce candles
//! at multiple timeframes simultaneously.

use super::candle::{CandleAggregator, PartialCandle};
use kairos_data::{Candle, FuturesTicker, Timeframe, Trade};
use std::collections::HashMap;

/// Identifies a specific (instrument, timeframe) aggregation slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AggregatorKey {
    /// The futures instrument being aggregated.
    pub instrument: FuturesTicker,
    /// The candle timeframe for this slot.
    pub timeframe: Timeframe,
}

/// Manages [`CandleAggregator`]s for multiple instrument/timeframe
/// combinations.
///
/// # Usage
///
/// 1. Call [`register`](Self::register) for each (instrument,
///    timeframe) pair the strategy needs.
/// 2. Feed trades via [`update`](Self::update). Trades are
///    automatically routed to all aggregators matching the
///    instrument.
/// 3. Query completed candles with [`candles`](Self::candles) or
///    in-progress bars with [`partial_candle`](Self::partial_candle).
/// 4. Call [`flush_all`](Self::flush_all) at end-of-data to close
///    any remaining partial candles.
///
/// The [`generation`](Self::generation) counter increments each
/// time any aggregator produces a new closed candle, providing a
/// lightweight change-detection mechanism.
pub struct MultiTimeframeAggregator {
    aggregators: HashMap<AggregatorKey, CandleAggregator>,
    /// Completed candles per key, in chronological order.
    candles: HashMap<AggregatorKey, Vec<Candle>>,
    /// Monotonic counter incremented each time a candle closes.
    generation: u64,
}

impl MultiTimeframeAggregator {
    /// Creates an empty aggregator with no registered pairs.
    #[must_use]
    pub fn new() -> Self {
        Self {
            aggregators: HashMap::new(),
            candles: HashMap::new(),
            generation: 0,
        }
    }

    /// Registers a new (instrument, timeframe) pair for aggregation.
    ///
    /// If the pair is already registered, this is a no-op.
    pub fn register(&mut self, instrument: FuturesTicker, timeframe: Timeframe) {
        let key = AggregatorKey {
            instrument,
            timeframe,
        };
        self.aggregators
            .entry(key)
            .or_insert_with(|| CandleAggregator::new(timeframe.to_milliseconds()));
        self.candles.entry(key).or_default();
    }

    /// Feeds a trade to all aggregators matching the instrument.
    ///
    /// Returns a list of (key, candle) pairs for any candles that
    /// closed as a result of this trade.
    pub fn update(
        &mut self,
        instrument: FuturesTicker,
        trade: &Trade,
    ) -> Vec<(AggregatorKey, Candle)> {
        let mut closed = Vec::new();

        let keys: Vec<AggregatorKey> = self
            .aggregators
            .keys()
            .filter(|k| k.instrument == instrument)
            .copied()
            .collect();

        for key in keys {
            if let Some(agg) = self.aggregators.get_mut(&key)
                && let Some(candle) = agg.update(trade)
            {
                self.candles.entry(key).or_default().push(candle);
                closed.push((key, candle));
                self.generation += 1;
            }
        }

        closed
    }

    /// Returns completed candles for the given instrument/timeframe.
    ///
    /// Returns an empty slice if the pair is not registered or has
    /// no completed candles yet.
    #[must_use]
    pub fn candles(&self, instrument: FuturesTicker, timeframe: Timeframe) -> &[Candle] {
        let key = AggregatorKey {
            instrument,
            timeframe,
        };
        self.candles.get(&key).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Returns the in-progress partial candle for the given pair.
    ///
    /// Returns `None` if the pair is not registered or no trades
    /// have been received yet.
    #[must_use]
    pub fn partial_candle(
        &self,
        instrument: FuturesTicker,
        timeframe: Timeframe,
    ) -> Option<&PartialCandle> {
        let key = AggregatorKey {
            instrument,
            timeframe,
        };
        self.aggregators.get(&key).and_then(|a| a.partial())
    }

    /// Returns the current generation counter.
    ///
    /// Incremented each time any aggregator produces a new closed
    /// candle. Useful for lightweight change detection.
    #[must_use]
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Flushes all aggregators and returns any remaining candles.
    ///
    /// Call this at the end of a data stream to close all partial
    /// candles. Each flushed candle is also appended to the
    /// completed candles for its key.
    pub fn flush_all(&mut self) -> Vec<(AggregatorKey, Candle)> {
        let mut closed = Vec::new();
        for (key, agg) in &mut self.aggregators {
            if let Some(candle) = agg.flush() {
                self.candles.entry(*key).or_default().push(candle);
                closed.push((*key, candle));
            }
        }
        closed
    }

    /// Resets all state, clearing aggregators, candles, and the
    /// generation counter.
    pub fn reset(&mut self) {
        self.aggregators.clear();
        self.candles.clear();
        self.generation = 0;
    }
}

impl Default for MultiTimeframeAggregator {
    fn default() -> Self {
        Self::new()
    }
}
