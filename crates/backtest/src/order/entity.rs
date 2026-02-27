//! The [`Order`] struct -- a single order tracked by the backtest
//! engine's order book.
//!
//! An `Order` captures everything about a submitted order: its
//! identity, instrument, type, fill state, lifecycle status, and
//! optional bracket/OCO linkage.

use crate::order::types::{OrderId, OrderSide, OrderStatus, OrderType, TimeInForce};
use kairos_data::{FuturesTicker, Price, Timestamp};
use serde::{Deserialize, Serialize};

/// A single order in the backtest order management system.
///
/// Orders progress through a lifecycle defined by [`OrderStatus`]:
/// `Pending` -> `Active` -> `PartiallyFilled` / `Filled`, or
/// `Cancelled` / `Rejected` / `Expired` at any working stage.
///
/// Bracket orders link a parent entry to child stop-loss and
/// take-profit orders via [`parent_id`](Self::parent_id) and
/// [`oco_partner`](Self::oco_partner).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    /// Unique identifier assigned by [`OrderId::next`].
    pub id: OrderId,
    /// The futures instrument this order targets.
    pub instrument: FuturesTicker,
    /// Buy or sell.
    pub side: OrderSide,
    /// Market, limit, stop, or stop-limit.
    pub order_type: OrderType,
    /// Lifetime policy (GTC, Day, IOC).
    pub time_in_force: TimeInForce,
    /// Total requested quantity (contracts).
    pub quantity: f64,
    /// Cumulative filled quantity so far.
    pub filled_quantity: f64,
    /// Volume-weighted average fill price, or `None` if unfilled.
    pub avg_fill_price: Option<Price>,
    /// Current lifecycle status.
    pub status: OrderStatus,
    /// Timestamp when the order was created.
    pub created_at: Timestamp,
    /// Timestamp of the most recent state change.
    pub updated_at: Timestamp,
    /// Optional strategy-defined label for identification.
    pub label: Option<String>,
    /// If this order is part of a bracket, the parent entry order ID.
    pub parent_id: Option<OrderId>,
    /// OCO partner order ID -- when this order fills, the partner is
    /// automatically cancelled.
    pub oco_partner: Option<OrderId>,
    /// If `true`, this order can only reduce an existing position
    /// (never open a new one).
    pub reduce_only: bool,
}

impl Order {
    /// Returns the unfilled quantity (`quantity - filled_quantity`).
    #[must_use]
    pub fn remaining_quantity(&self) -> f64 {
        self.quantity - self.filled_quantity
    }

    /// Returns `true` if the order is working (`Active` or
    /// `PartiallyFilled`).
    #[must_use]
    pub fn is_active(&self) -> bool {
        matches!(
            self.status,
            OrderStatus::Active | OrderStatus::PartiallyFilled
        )
    }

    /// Returns `true` if the order is `Pending` (not yet activated).
    #[must_use]
    pub fn is_pending(&self) -> bool {
        self.status == OrderStatus::Pending
    }

    /// Returns `true` if the order has reached a terminal status
    /// (`Filled`, `Cancelled`, `Rejected`, or `Expired`).
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        self.status.is_terminal()
    }

    /// Record a (partial or full) fill against this order.
    ///
    /// Updates [`filled_quantity`](Self::filled_quantity),
    /// [`avg_fill_price`](Self::avg_fill_price) (volume-weighted),
    /// [`status`](Self::status), and [`updated_at`](Self::updated_at).
    pub fn record_fill(&mut self, fill_qty: f64, fill_price: Price, timestamp: Timestamp) {
        let prev_filled = self.filled_quantity;
        self.filled_quantity += fill_qty;

        // Volume-weighted average fill price
        self.avg_fill_price = Some(if let Some(prev_avg) = self.avg_fill_price {
            let prev_value = prev_avg.to_f64() * prev_filled;
            let fill_value = fill_price.to_f64() * fill_qty;
            Price::from_f64((prev_value + fill_value) / self.filled_quantity)
        } else {
            fill_price
        });

        self.updated_at = timestamp;

        if (self.filled_quantity - self.quantity).abs() < 1e-9 {
            self.status = OrderStatus::Filled;
        } else {
            self.status = OrderStatus::PartiallyFilled;
        }
    }
}
