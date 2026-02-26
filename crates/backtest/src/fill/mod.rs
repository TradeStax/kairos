pub mod depth;
pub mod latency;
pub mod market;

pub use depth::DepthBasedFillSimulator;
pub use latency::{FixedLatency, LatencyModel, ZeroLatency};
pub use market::StandardFillSimulator;

use crate::config::instrument::InstrumentSpec;
use crate::order::entity::Order;
use crate::order::types::{OrderId, OrderSide};
use kairos_data::{Depth, Price, Timestamp, Trade};

/// Result of a fill check.
#[derive(Debug, Clone)]
pub struct FillResult {
    pub order_id: OrderId,
    pub fill_price: Price,
    pub fill_quantity: f64,
    pub timestamp: Timestamp,
}

/// Trait for simulating order fills against market data.
pub trait FillSimulator: Send + Sync {
    /// Check which active orders should be filled given current
    /// market data.
    fn check_fills(
        &self,
        trade: &Trade,
        depth: Option<&Depth>,
        active_orders: &[&Order],
        instruments: &std::collections::HashMap<kairos_data::FuturesTicker, InstrumentSpec>,
    ) -> Vec<FillResult>;

    /// Compute the fill price for a market order.
    fn market_fill_price(
        &self,
        trade: &Trade,
        side: OrderSide,
        quantity: f64,
        depth: Option<&Depth>,
        instrument: &InstrumentSpec,
    ) -> Price;

    fn clone_simulator(&self) -> Box<dyn FillSimulator>;
}
