use super::candle::{CandleAggregator, PartialCandle};
use kairos_data::{Candle, FuturesTicker, Timeframe, Trade};
use std::collections::HashMap;

/// Key for a specific (instrument, timeframe) pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AggregatorKey {
    pub instrument: FuturesTicker,
    pub timeframe: Timeframe,
}

/// Manages CandleAggregators for multiple instrument/timeframe
/// combinations.
pub struct MultiTimeframeAggregator {
    aggregators: HashMap<AggregatorKey, CandleAggregator>,
    /// Completed candles per key, in chronological order.
    candles: HashMap<AggregatorKey, Vec<Candle>>,
    /// Monotonic counter incremented each time a new candle closes.
    generation: u64,
}

impl MultiTimeframeAggregator {
    pub fn new() -> Self {
        Self {
            aggregators: HashMap::new(),
            candles: HashMap::new(),
            generation: 0,
        }
    }

    /// Register a new (instrument, timeframe) pair.
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

    /// Feed a trade to all aggregators for the given instrument.
    /// Returns a list of (key, candle) for any candles that closed.
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

    /// Get completed candles for a specific key.
    pub fn candles(&self, instrument: FuturesTicker, timeframe: Timeframe) -> &[Candle] {
        let key = AggregatorKey {
            instrument,
            timeframe,
        };
        self.candles.get(&key).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Get the partial (in-progress) candle for a key.
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

    /// Current generation counter — incremented each time any
    /// aggregator produces a new closed candle.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Flush all aggregators and return any remaining candles.
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
