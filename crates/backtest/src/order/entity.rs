use crate::order::types::{OrderId, OrderSide, OrderStatus, OrderType, TimeInForce};
use kairos_data::{FuturesTicker, Price, Timestamp};
use serde::{Deserialize, Serialize};

/// A single order in the backtest order management system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub id: OrderId,
    pub instrument: FuturesTicker,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub time_in_force: TimeInForce,
    pub quantity: f64,
    pub filled_quantity: f64,
    pub avg_fill_price: Option<Price>,
    pub status: OrderStatus,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub label: Option<String>,
    /// If this order is part of a bracket, the parent order ID.
    pub parent_id: Option<OrderId>,
    /// OCO partner order ID (cancels partner when filled).
    pub oco_partner: Option<OrderId>,
    /// If true, this order can only reduce an existing position.
    pub reduce_only: bool,
}

impl Order {
    pub fn remaining_quantity(&self) -> f64 {
        self.quantity - self.filled_quantity
    }

    pub fn is_active(&self) -> bool {
        matches!(
            self.status,
            OrderStatus::Active | OrderStatus::PartiallyFilled
        )
    }

    pub fn is_pending(&self) -> bool {
        self.status == OrderStatus::Pending
    }

    pub fn is_terminal(&self) -> bool {
        self.status.is_terminal()
    }

    pub fn record_fill(&mut self, fill_qty: f64, fill_price: Price, timestamp: Timestamp) {
        let prev_filled = self.filled_quantity;
        self.filled_quantity += fill_qty;

        // Weighted average fill price
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
