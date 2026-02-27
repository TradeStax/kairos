//! Fill simulation for the backtesting engine.
//!
//! This module provides the [`FillSimulator`] trait and concrete
//! implementations that decide when and at what price pending
//! orders are filled during a backtest.
//!
//! Two simulators are included:
//!
//! - [`StandardFillSimulator`] — applies a configurable slippage
//!   model to market/stop orders and triggers limit orders at their
//!   limit price.
//! - [`DepthBasedFillSimulator`] — walks the order-book depth
//!   snapshot to compute a volume-weighted average fill price,
//!   falling back to `StandardFillSimulator` when depth is absent.
//!
//! The [`LatencyModel`] trait (and its implementations) model the
//! delay between order submission and activation. Latency is not
//! yet wired into the engine; fills are currently instant.

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

/// Result of a fill check — one per order that was triggered.
#[derive(Debug, Clone, PartialEq)]
pub struct FillResult {
    /// Identifier of the filled order.
    pub order_id: OrderId,
    /// Execution price (may include slippage or depth-walk adjustment).
    pub fill_price: Price,
    /// Number of contracts filled.
    pub fill_quantity: f64,
    /// Simulation timestamp at which the fill occurred.
    pub timestamp: Timestamp,
}

/// Trait for simulating order fills against market data.
///
/// Implementors decide which pending orders are triggered by an
/// incoming trade (and optional depth snapshot) and compute the
/// resulting fill price and quantity.
pub trait FillSimulator: Send + Sync {
    /// Check which active orders should be filled given the latest
    /// trade and (optional) depth snapshot.
    ///
    /// Returns a [`FillResult`] for every order that was triggered.
    fn check_fills(
        &self,
        trade: &Trade,
        depth: Option<&Depth>,
        active_orders: &[&Order],
        instruments: &std::collections::HashMap<kairos_data::FuturesTicker, InstrumentSpec>,
    ) -> Vec<FillResult>;

    /// Compute the fill price for a market order of the given
    /// `side` and `quantity`.
    ///
    /// Used by the engine when a strategy submits a new market
    /// order mid-tick.
    fn market_fill_price(
        &self,
        trade: &Trade,
        side: OrderSide,
        quantity: f64,
        depth: Option<&Depth>,
        instrument: &InstrumentSpec,
    ) -> Price;

    /// Create a boxed clone of this simulator.
    ///
    /// Required because `FillSimulator` is object-safe and stored
    /// as `Box<dyn FillSimulator>`.
    fn clone_simulator(&self) -> Box<dyn FillSimulator>;
}
