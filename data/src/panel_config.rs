//! Panel Configuration Types
//!
//! Configuration and state types for panels (Ladder, Time & Sales)

use serde::{Deserialize, Serialize};

// Re-export domain panel types
pub use crate::domain::panel::{ChaseTracker, TradeAggregator};

pub mod ladder {
    use serde::{Deserialize, Serialize};
    pub use crate::domain::panel::ChaseTracker;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Config {
        pub levels: usize,
        pub group_by_ticks: usize,
        pub show_chase: bool,
        pub show_chase_tracker: bool,
        pub show_spread: bool,
        pub trade_retention: std::time::Duration,
    }

    impl Default for Config {
        fn default() -> Self {
            Self {
                levels: 20,
                group_by_ticks: 1,
                show_chase: true,
                show_chase_tracker: true,
                show_spread: true,
                trade_retention: std::time::Duration::from_secs(300),
            }
        }
    }

    pub use crate::domain::Side;

    /// Grouped depth level
    #[derive(Debug, Clone)]
    pub struct GroupedDepth {
        pub price: i64,
        pub buy_qty: f32,
        pub sell_qty: f32,
    }

    /// Trade store for ladder
    #[derive(Debug, Clone, Default)]
    pub struct TradeStore {
        pub trades: Vec<(u64, i64, f32, bool)>, // (time, price, qty, is_sell)
    }
}

pub mod timeandsales {
    use serde::{Deserialize, Serialize};
    use crate::domain::{Price, Timestamp};

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Config {
        pub max_rows: usize,
        pub show_delta: bool,
        pub stacked_bar: Option<StackedBar>,
        pub trade_size_filter: f32,
        pub trade_retention: std::time::Duration,
    }

    impl Default for Config {
        fn default() -> Self {
            Self {
                max_rows: 100,
                show_delta: true,
                stacked_bar: Some(StackedBar::Full(StackedBarRatio::Volume)),
                trade_size_filter: 0.0,
                trade_retention: std::time::Duration::from_secs(300), // 5 minutes
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    pub enum StackedBar {
        Compact(StackedBarRatio),
        Full(StackedBarRatio),
    }

    impl StackedBar {
        pub fn ratio(&self) -> StackedBarRatio {
            match self {
                StackedBar::Compact(r) | StackedBar::Full(r) => *r,
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    pub enum StackedBarRatio {
        Count,
        Volume,
        AverageSize,
    }

    impl StackedBarRatio {
        pub const ALL: &'static [StackedBarRatio] = &[
            StackedBarRatio::Count,
            StackedBarRatio::Volume,
            StackedBarRatio::AverageSize,
        ];
    }

    impl Default for StackedBarRatio {
        fn default() -> Self {
            StackedBarRatio::Volume
        }
    }

    impl std::fmt::Display for StackedBarRatio {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                StackedBarRatio::Count => write!(f, "Count"),
                StackedBarRatio::Volume => write!(f, "Volume"),
                StackedBarRatio::AverageSize => write!(f, "Avg Size"),
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    pub enum TradeDisplay {
        All,
        BuysOnly,
        SellsOnly,
        LargeTrades { min_size: u32 },
    }

    impl Default for TradeDisplay {
        fn default() -> Self {
            TradeDisplay::All
        }
    }

    /// Trade entry for display (contains formatted data)
    #[derive(Debug, Clone)]
    pub struct TradeEntry {
        pub time: Timestamp,
        pub ts_ms: u64,
        pub price: Price,
        pub quantity: f32,
        pub is_sell: bool,
        pub display: TradeDisplayData,
    }

    /// Display data for a single trade (for rendering)
    #[derive(Debug, Clone)]
    pub struct TradeDisplayData {
        pub time_str: String,
        pub price: f32,
        pub qty: f32,
        pub is_sell: bool,
    }

    impl TradeEntry {
        pub fn new(time: Timestamp, price: Price, quantity: f32, is_sell: bool) -> Self {
            let dt = time.to_datetime();
            let time_str = dt.format("%M:%S.%3f").to_string();

            Self {
                ts_ms: time.0,
                time,
                price,
                quantity,
                is_sell,
                display: TradeDisplayData {
                    time_str,
                    price: price.to_f32(),
                    qty: quantity,
                    is_sell,
                },
            }
        }
    }

    /// Historical aggregation for stacked bar
    #[derive(Debug, Clone, Default)]
    pub struct HistAgg {
        buy_count: usize,
        sell_count: usize,
        buy_volume: f32,
        sell_volume: f32,
        buy_size_sum: f32,
        sell_size_sum: f32,
    }

    impl HistAgg {
        pub fn new() -> Self {
            Self::default()
        }

        /// Add a trade to the aggregation
        pub fn add(&mut self, trade: &TradeDisplayData) {
            if trade.is_sell {
                self.sell_count += 1;
                self.sell_volume += trade.qty;
                self.sell_size_sum += trade.qty;
            } else {
                self.buy_count += 1;
                self.buy_volume += trade.qty;
                self.buy_size_sum += trade.qty;
            }
        }

        /// Remove a trade from the aggregation
        pub fn remove(&mut self, trade: &TradeDisplayData) {
            if trade.is_sell {
                self.sell_count = self.sell_count.saturating_sub(1);
                self.sell_volume = (self.sell_volume - trade.qty).max(0.0);
                self.sell_size_sum = (self.sell_size_sum - trade.qty).max(0.0);
            } else {
                self.buy_count = self.buy_count.saturating_sub(1);
                self.buy_volume = (self.buy_volume - trade.qty).max(0.0);
                self.buy_size_sum = (self.buy_size_sum - trade.qty).max(0.0);
            }
        }

        /// Get values for display based on ratio kind
        /// Returns (buy_value, sell_value, buy_ratio)
        pub fn values_for(&self, ratio: StackedBarRatio) -> Option<(f64, f64, f32)> {
            let (buy_val, sell_val) = match ratio {
                StackedBarRatio::Count => {
                    (self.buy_count as f64, self.sell_count as f64)
                }
                StackedBarRatio::Volume => {
                    (self.buy_volume as f64, self.sell_volume as f64)
                }
                StackedBarRatio::AverageSize => {
                    let buy_avg = if self.buy_count > 0 {
                        self.buy_size_sum as f64 / self.buy_count as f64
                    } else {
                        0.0
                    };
                    let sell_avg = if self.sell_count > 0 {
                        self.sell_size_sum as f64 / self.sell_count as f64
                    } else {
                        0.0
                    };
                    (buy_avg, sell_avg)
                }
            };

            let total = buy_val + sell_val;
            if total > 0.0 {
                let buy_ratio = (buy_val / total) as f32;
                Some((buy_val, sell_val, buy_ratio))
            } else {
                None
            }
        }
    }
}
