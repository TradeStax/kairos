use crate::order::types::{OrderId, OrderSide, OrderType, TimeInForce};
use crate::output::trade_record::ExitReason;
use kairos_data::{FuturesTicker, Price};
use serde::{Deserialize, Serialize};

/// A strategy's order instruction to the engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OrderRequest {
    /// Submit a single order.
    Submit(NewOrder),
    /// Submit a bracket: entry + stop-loss + optional take-profit
    /// (OCO pair).
    SubmitBracket(BracketOrder),
    /// Cancel a specific order by ID.
    Cancel { order_id: OrderId },
    /// Cancel all active orders, optionally filtered by instrument.
    CancelAll { instrument: Option<FuturesTicker> },
    /// Modify an existing order's price and/or quantity.
    Modify {
        order_id: OrderId,
        new_price: Option<Price>,
        new_quantity: Option<f64>,
    },
    /// Flatten (close) entire position for an instrument.
    Flatten {
        instrument: FuturesTicker,
        reason: ExitReason,
    },
    /// Do nothing.
    Noop,
}

/// Parameters for a new single order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewOrder {
    pub instrument: FuturesTicker,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub quantity: f64,
    #[serde(default)]
    pub time_in_force: TimeInForce,
    pub label: Option<String>,
    #[serde(default)]
    pub reduce_only: bool,
}

/// Parameters for a bracket order (entry + protective orders).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BracketOrder {
    pub entry: NewOrder,
    pub stop_loss: Price,
    pub take_profit: Option<Price>,
}
