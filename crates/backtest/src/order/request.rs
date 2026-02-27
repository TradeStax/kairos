//! Order requests -- the instructions a strategy sends to the
//! backtest engine.
//!
//! Strategies return [`OrderRequest`] variants from their `on_trade`
//! callback to submit, cancel, modify, or flatten positions. The
//! engine processes these requests on each tick.

use crate::order::types::{OrderId, OrderSide, OrderType, TimeInForce};
use crate::output::trade_record::ExitReason;
use kairos_data::{FuturesTicker, Price};
use serde::{Deserialize, Serialize};

/// A strategy's order instruction to the engine.
///
/// Returned by strategy lifecycle callbacks (e.g. `on_candle`,
/// `on_tick`) to express the desired action on each tick.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OrderRequest {
    /// Submit a single order.
    Submit(NewOrder),
    /// Submit a bracket order set: entry + stop-loss + optional
    /// take-profit (linked as an OCO pair).
    SubmitBracket(BracketOrder),
    /// Cancel a specific order by ID.
    Cancel {
        /// The ID of the order to cancel.
        order_id: OrderId,
    },
    /// Cancel all active orders, optionally filtered by instrument.
    CancelAll {
        /// If `Some`, only cancel orders for this instrument.
        instrument: Option<FuturesTicker>,
    },
    /// Modify an existing order's price and/or quantity.
    Modify {
        /// The ID of the order to modify.
        order_id: OrderId,
        /// New price (limit or trigger), or `None` to keep current.
        new_price: Option<Price>,
        /// New total quantity, or `None` to keep current.
        new_quantity: Option<f64>,
    },
    /// Flatten (close) the entire position for an instrument.
    Flatten {
        /// The instrument whose position should be flattened.
        instrument: FuturesTicker,
        /// Why the position is being closed.
        reason: ExitReason,
    },
    /// No action this tick.
    Noop,
}

/// Parameters for submitting a new single order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewOrder {
    /// The futures instrument to trade.
    pub instrument: FuturesTicker,
    /// Buy or sell.
    pub side: OrderSide,
    /// Market, limit, stop, or stop-limit.
    pub order_type: OrderType,
    /// Number of contracts.
    pub quantity: f64,
    /// Lifetime policy. Defaults to [`TimeInForce::GTC`].
    #[serde(default)]
    pub time_in_force: TimeInForce,
    /// Optional strategy-defined label.
    pub label: Option<String>,
    /// If `true`, this order can only reduce an existing position.
    #[serde(default)]
    pub reduce_only: bool,
}

/// Parameters for a bracket order: an entry order paired with
/// protective stop-loss and optional take-profit exit orders.
///
/// The stop-loss and take-profit are linked as OCO (one-cancels-other)
/// partners: when one fills, the engine cancels the other.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BracketOrder {
    /// The entry order parameters.
    pub entry: NewOrder,
    /// Stop-loss trigger price.
    pub stop_loss: Price,
    /// Optional take-profit limit price.
    pub take_profit: Option<Price>,
}
