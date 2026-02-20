//! Panel-specific Domain Logic
//!
//! Pure business logic for panel features (ladder, time&sales).
//! NO UI dependencies - these are domain concepts only.
//!
//! Architecture:
//! - chase_tracker: Algorithm for detecting consecutive price moves with fade
//! - trade_aggregator: Aggregation logic for buy/sell metrics (stacked bars)
//! - depth_grouping: Orderbook level grouping by tick size
//!
//! All logic uses exchange::util::Price units (i64) for precision.

pub mod chase_tracker;
pub mod depth_grouping;
pub mod trade_aggregator;

// Re-export commonly used types
pub use chase_tracker::ChaseTracker;
pub use depth_grouping::{DepthSide, group_depth_by_tick};
pub use trade_aggregator::{AggregationMode, StackedBarMetrics, TradeAggregator};
