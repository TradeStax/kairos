//! Trade Aggregator - Pure Domain Logic
//!
//! Aggregates buy/sell trades for stacked bar visualization.
//! Supports multiple aggregation modes: Count, Volume, Average Size.
//!
//! This is pure calculation logic with no UI dependencies.

/// Aggregation mode for stacked bar
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AggregationMode {
    /// Count of trades (buy vs sell count)
    Count,
    /// Volume (sum of quantities)
    Volume,
    /// Average trade size
    AverageSize,
}

/// Stacked bar metrics (output)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StackedBarMetrics {
    /// Buy side value (meaning depends on mode)
    pub buy_value: f64,
    /// Sell side value (meaning depends on mode)
    pub sell_value: f64,
    /// Buy ratio (0.0 to 1.0)
    pub buy_ratio: f32,
}

/// Trade Aggregator - Accumulates buy/sell statistics
#[derive(Debug, Clone, Default)]
pub struct TradeAggregator {
    buy_count: u64,
    sell_count: u64,
    buy_sum: f64,  // Sum of buy volumes
    sell_sum: f64, // Sum of sell volumes
}

impl TradeAggregator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a trade to the aggregation
    ///
    /// # Arguments
    /// * `qty` - Trade quantity
    /// * `is_sell` - true for sell side, false for buy side
    pub fn add_trade(&mut self, qty: f32, is_sell: bool) {
        let qty_f64 = qty as f64;

        if is_sell {
            self.sell_count += 1;
            self.sell_sum += qty_f64;
        } else {
            self.buy_count += 1;
            self.buy_sum += qty_f64;
        }
    }

    /// Remove a trade from the aggregation (for time-based sliding window)
    ///
    /// # Arguments
    /// * `qty` - Trade quantity
    /// * `is_sell` - true for sell side, false for buy side
    pub fn remove_trade(&mut self, qty: f32, is_sell: bool) {
        let qty_f64 = qty as f64;

        if is_sell {
            self.sell_count = self.sell_count.saturating_sub(1);
            self.sell_sum = (self.sell_sum - qty_f64).max(0.0);
        } else {
            self.buy_count = self.buy_count.saturating_sub(1);
            self.buy_sum = (self.buy_sum - qty_f64).max(0.0);
        }
    }

    /// Get metrics for a specific aggregation mode
    ///
    /// Returns None if there's no data (prevents division by zero)
    pub fn metrics(&self, mode: AggregationMode) -> Option<StackedBarMetrics> {
        match mode {
            AggregationMode::Count => self.count_metrics(),
            AggregationMode::Volume => self.volume_metrics(),
            AggregationMode::AverageSize => self.avg_size_metrics(),
        }
    }

    /// Clear all accumulated data
    pub fn clear(&mut self) {
        self.buy_count = 0;
        self.sell_count = 0;
        self.buy_sum = 0.0;
        self.sell_sum = 0.0;
    }

    // Private helper methods

    fn count_metrics(&self) -> Option<StackedBarMetrics> {
        let buy = self.buy_count as f64;
        let sell = self.sell_count as f64;
        let total = buy + sell;

        if total <= 0.0 {
            return None;
        }

        let buy_ratio = (buy / total) as f32;

        Some(StackedBarMetrics {
            buy_value: buy,
            sell_value: sell,
            buy_ratio,
        })
    }

    fn volume_metrics(&self) -> Option<StackedBarMetrics> {
        let buy = self.buy_sum;
        let sell = self.sell_sum;
        let total = buy + sell;

        if total <= 0.0 {
            return None;
        }

        let buy_ratio = (buy / total) as f32;

        Some(StackedBarMetrics {
            buy_value: buy,
            sell_value: sell,
            buy_ratio,
        })
    }

    fn avg_size_metrics(&self) -> Option<StackedBarMetrics> {
        let buy_avg = if self.buy_count > 0 {
            self.buy_sum / self.buy_count as f64
        } else {
            0.0
        };

        let sell_avg = if self.sell_count > 0 {
            self.sell_sum / self.sell_count as f64
        } else {
            0.0
        };

        let denom = buy_avg + sell_avg;
        if denom <= 0.0 {
            return None;
        }

        let buy_ratio = (buy_avg / denom) as f32;

        Some(StackedBarMetrics {
            buy_value: buy_avg,
            sell_value: sell_avg,
            buy_ratio,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_mode() {
        let mut agg = TradeAggregator::new();

        agg.add_trade(10.0, false); // Buy
        agg.add_trade(20.0, false); // Buy
        agg.add_trade(15.0, true); // Sell

        let metrics = agg.metrics(AggregationMode::Count).unwrap();
        assert_eq!(metrics.buy_value, 2.0);
        assert_eq!(metrics.sell_value, 1.0);
        assert!((metrics.buy_ratio - 0.667).abs() < 0.01); // 2/3
    }

    #[test]
    fn test_volume_mode() {
        let mut agg = TradeAggregator::new();

        agg.add_trade(10.0, false); // Buy 10
        agg.add_trade(20.0, false); // Buy 20
        agg.add_trade(15.0, true); // Sell 15

        let metrics = agg.metrics(AggregationMode::Volume).unwrap();
        assert_eq!(metrics.buy_value, 30.0); // 10 + 20
        assert_eq!(metrics.sell_value, 15.0);
        assert!((metrics.buy_ratio - 0.667).abs() < 0.01); // 30/45
    }

    #[test]
    fn test_avg_size_mode() {
        let mut agg = TradeAggregator::new();

        agg.add_trade(10.0, false); // Buy avg: (10+20)/2 = 15
        agg.add_trade(20.0, false);
        agg.add_trade(30.0, true); // Sell avg: 30

        let metrics = agg.metrics(AggregationMode::AverageSize).unwrap();
        assert_eq!(metrics.buy_value, 15.0);
        assert_eq!(metrics.sell_value, 30.0);
        assert!((metrics.buy_ratio - 0.333).abs() < 0.01); // 15/45
    }

    #[test]
    fn test_remove_trade() {
        let mut agg = TradeAggregator::new();

        agg.add_trade(10.0, false);
        agg.add_trade(20.0, false);
        agg.add_trade(15.0, true);

        // Remove one buy trade
        agg.remove_trade(10.0, false);

        let metrics = agg.metrics(AggregationMode::Volume).unwrap();
        assert_eq!(metrics.buy_value, 20.0); // Only 20 left
        assert_eq!(metrics.sell_value, 15.0);
    }

    #[test]
    fn test_empty_returns_none() {
        let agg = TradeAggregator::new();

        assert!(agg.metrics(AggregationMode::Count).is_none());
        assert!(agg.metrics(AggregationMode::Volume).is_none());
        assert!(agg.metrics(AggregationMode::AverageSize).is_none());
    }

    #[test]
    fn test_clear() {
        let mut agg = TradeAggregator::new();

        agg.add_trade(10.0, false);
        agg.add_trade(20.0, true);

        assert!(agg.metrics(AggregationMode::Count).is_some());

        agg.clear();

        assert!(agg.metrics(AggregationMode::Count).is_none());
    }
}
