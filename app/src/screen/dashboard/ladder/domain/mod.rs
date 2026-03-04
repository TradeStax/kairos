//! Ladder-specific Domain Logic
//!
//! Types and business logic for the ladder.
//! NO UI dependencies - these are domain concepts only.
//!
//! Architecture:
//! - chase_tracker: Algorithm for detecting consecutive price moves with fade
//! - trade_aggregator: Aggregation logic for buy/sell metrics (stacked bars)
//! - depth_grouping: Orderbook level grouping by tick size
//!
//! All logic uses Price units (i64) for precision.

#[allow(dead_code)]
pub mod chase_tracker;
#[allow(dead_code)]
pub mod depth_grouping;
#[allow(dead_code)]
pub mod trade_aggregator;
