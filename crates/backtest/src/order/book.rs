//! [`OrderBook`] -- central registry and manager for all orders in a
//! backtest run.
//!
//! The order book owns every [`Order`] and maintains a fast-path list
//! of active order IDs. It handles creation (single and bracket),
//! modification, cancellation, expiration, and OCO linking.

use crate::order::entity::Order;
use crate::order::request::{BracketOrder, NewOrder};
use crate::order::types::{OrderId, OrderStatus, OrderType, TimeInForce};
use kairos_data::{FuturesTicker, Price, Timestamp};
use std::collections::HashMap;

/// Central order manager for the backtest engine.
///
/// Stores all orders (both active and terminal) keyed by [`OrderId`],
/// and maintains a separate list of active order IDs for efficient
/// iteration during fill matching.
pub struct OrderBook {
    /// All orders by ID, including terminal ones.
    orders: HashMap<OrderId, Order>,
    /// IDs of orders that are still working (active or partially
    /// filled). Periodically cleaned by [`cleanup_active`](Self::cleanup_active).
    active_ids: Vec<OrderId>,
}

impl OrderBook {
    /// Create an empty order book.
    #[must_use]
    pub fn new() -> Self {
        Self {
            orders: HashMap::new(),
            active_ids: Vec::new(),
        }
    }

    /// Look up an order by ID.
    #[must_use]
    pub fn get(&self, id: OrderId) -> Option<&Order> {
        self.orders.get(&id)
    }

    /// Look up an order by ID (mutable).
    pub fn get_mut(&mut self, id: OrderId) -> Option<&mut Order> {
        self.orders.get_mut(&id)
    }

    /// Iterate over all currently active orders.
    pub fn active_orders(&self) -> impl Iterator<Item = &Order> {
        self.active_ids
            .iter()
            .filter_map(|id| self.orders.get(id))
            .filter(|o| o.is_active())
    }

    /// Count the number of active (non-terminal) orders.
    #[must_use]
    pub fn active_count(&self) -> usize {
        self.active_ids
            .iter()
            .filter(|id| self.orders.get(id).is_some_and(|o| o.is_active()))
            .count()
    }

    /// Create and immediately activate a single order.
    ///
    /// Returns the newly assigned [`OrderId`].
    pub fn create_order(&mut self, new: &NewOrder, timestamp: Timestamp) -> OrderId {
        let id = OrderId::next();
        let order = Order {
            id,
            instrument: new.instrument,
            side: new.side,
            order_type: new.order_type,
            time_in_force: new.time_in_force,
            quantity: new.quantity,
            filled_quantity: 0.0,
            avg_fill_price: None,
            status: OrderStatus::Active,
            created_at: timestamp,
            updated_at: timestamp,
            label: new.label.clone(),
            parent_id: None,
            oco_partner: None,
            reduce_only: new.reduce_only,
        };
        self.orders.insert(id, order);
        self.active_ids.push(id);
        id
    }

    /// Create a bracket order set: entry + stop-loss + optional
    /// take-profit.
    ///
    /// The stop-loss and take-profit are linked as OCO partners so
    /// that filling one automatically cancels the other. The child
    /// orders start in [`OrderStatus::Pending`] and are activated when
    /// the entry fills (see [`activate_bracket_children`](Self::activate_bracket_children)).
    ///
    /// Returns `(entry_id, stop_loss_id, Option<take_profit_id>)`.
    pub fn create_bracket(
        &mut self,
        bracket: &BracketOrder,
        timestamp: Timestamp,
    ) -> (OrderId, OrderId, Option<OrderId>) {
        let entry_id = self.create_order(&bracket.entry, timestamp);
        let exit_side = bracket.entry.side.opposite();

        // Stop-loss order (pending until entry fills)
        let sl_id = OrderId::next();
        let sl_order = Order {
            id: sl_id,
            instrument: bracket.entry.instrument,
            side: exit_side,
            order_type: OrderType::Stop {
                trigger: bracket.stop_loss,
            },
            time_in_force: TimeInForce::GTC,
            quantity: bracket.entry.quantity,
            filled_quantity: 0.0,
            avg_fill_price: None,
            status: OrderStatus::Pending,
            created_at: timestamp,
            updated_at: timestamp,
            label: Some("Bracket SL".to_string()),
            parent_id: Some(entry_id),
            oco_partner: None,
            reduce_only: true,
        };
        self.orders.insert(sl_id, sl_order);

        // Take-profit order (pending, if specified)
        let tp_id = bracket.take_profit.map(|tp_price| {
            let tp_id = OrderId::next();
            let tp_order = Order {
                id: tp_id,
                instrument: bracket.entry.instrument,
                side: exit_side,
                order_type: OrderType::Limit { price: tp_price },
                time_in_force: TimeInForce::GTC,
                quantity: bracket.entry.quantity,
                filled_quantity: 0.0,
                avg_fill_price: None,
                status: OrderStatus::Pending,
                created_at: timestamp,
                updated_at: timestamp,
                label: Some("Bracket TP".to_string()),
                parent_id: Some(entry_id),
                oco_partner: Some(sl_id),
                reduce_only: true,
            };
            self.orders.insert(tp_id, tp_order);
            tp_id
        });

        // Link OCO partners (SL <-> TP)
        if let Some(tp_id) = tp_id
            && let Some(sl) = self.orders.get_mut(&sl_id)
        {
            sl.oco_partner = Some(tp_id);
        }

        (entry_id, sl_id, tp_id)
    }

    /// Activate pending bracket child orders after the parent entry
    /// order has been filled.
    pub fn activate_bracket_children(&mut self, parent_id: OrderId) {
        let children: Vec<OrderId> = self
            .orders
            .values()
            .filter(|o| o.parent_id == Some(parent_id) && o.is_pending())
            .map(|o| o.id)
            .collect();

        for id in children {
            if let Some(order) = self.orders.get_mut(&id) {
                order.status = OrderStatus::Active;
                self.active_ids.push(id);
            }
        }
    }

    /// Find the stop-loss trigger price for a bracket entry order
    /// by searching its child orders.
    #[must_use]
    pub fn bracket_stop_loss(&self, parent_id: OrderId) -> Option<Price> {
        self.orders.values().find_map(|o| {
            if o.parent_id == Some(parent_id)
                && let OrderType::Stop { trigger } = o.order_type
            {
                return Some(trigger);
            }
            None
        })
    }

    /// Cancel an order. If it has an OCO partner, that partner is
    /// cancelled too.
    pub fn cancel(&mut self, id: OrderId, timestamp: Timestamp) {
        if let Some(order) = self.orders.get_mut(&id)
            && !order.is_terminal()
        {
            order.status = OrderStatus::Cancelled;
            order.updated_at = timestamp;

            // Cancel OCO partner
            let oco = order.oco_partner;
            if let Some(partner_id) = oco
                && let Some(partner) = self.orders.get_mut(&partner_id)
                && !partner.is_terminal()
            {
                partner.status = OrderStatus::Cancelled;
                partner.updated_at = timestamp;
            }
        }
        self.cleanup_active();
    }

    /// Cancel all non-terminal orders, optionally filtered by
    /// instrument.
    pub fn cancel_all(&mut self, instrument: Option<FuturesTicker>, timestamp: Timestamp) {
        let ids: Vec<OrderId> = self
            .orders
            .values()
            .filter(|o| {
                !o.is_terminal() && instrument.as_ref().is_none_or(|inst| o.instrument == *inst)
            })
            .map(|o| o.id)
            .collect();

        for id in ids {
            if let Some(order) = self.orders.get_mut(&id) {
                order.status = OrderStatus::Cancelled;
                order.updated_at = timestamp;
            }
        }
        self.cleanup_active();
    }

    /// Modify an active order's price and/or quantity.
    ///
    /// Returns `true` if the modification was applied, `false` if the
    /// order was not found or is already in a terminal state.
    #[must_use]
    pub fn modify(
        &mut self,
        id: OrderId,
        new_price: Option<Price>,
        new_quantity: Option<f64>,
        timestamp: Timestamp,
    ) -> bool {
        let Some(order) = self.orders.get_mut(&id) else {
            return false;
        };
        if order.is_terminal() {
            return false;
        }

        if let Some(price) = new_price {
            order.order_type = match order.order_type {
                OrderType::Limit { .. } => OrderType::Limit { price },
                OrderType::Stop { .. } => OrderType::Stop { trigger: price },
                OrderType::StopLimit { limit, .. } => OrderType::StopLimit {
                    trigger: price,
                    limit,
                },
                other => other,
            };
        }
        if let Some(qty) = new_quantity {
            order.quantity = qty;
        }
        order.updated_at = timestamp;
        true
    }

    /// Expire all orders with [`TimeInForce::Day`].
    ///
    /// Called at session close to cancel working day orders.
    pub fn expire_day_orders(&mut self, timestamp: Timestamp) {
        let ids: Vec<OrderId> = self
            .orders
            .values()
            .filter(|o| !o.is_terminal() && o.time_in_force == TimeInForce::Day)
            .map(|o| o.id)
            .collect();

        for id in ids {
            if let Some(order) = self.orders.get_mut(&id) {
                order.status = OrderStatus::Expired;
                order.updated_at = timestamp;
            }
        }
        self.cleanup_active();
    }

    /// Remove terminal orders from the active ID list.
    fn cleanup_active(&mut self) {
        self.active_ids
            .retain(|id| self.orders.get(id).is_some_and(|o| !o.is_terminal()));
    }

    /// Clear all orders and reset the ID counter.
    ///
    /// Call between backtest runs to start fresh.
    pub fn reset(&mut self) {
        self.orders.clear();
        self.active_ids.clear();
        OrderId::reset();
    }
}

impl Default for OrderBook {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::order::request::{BracketOrder, NewOrder};
    use crate::order::types::{OrderSide, OrderStatus, OrderType, TimeInForce};
    use kairos_data::{FuturesTicker, Price, Timestamp};

    fn es_ticker() -> FuturesTicker {
        FuturesTicker::new("ES.c.0", kairos_data::FuturesVenue::CMEGlobex)
    }

    fn ts(ms: u64) -> Timestamp {
        Timestamp(ms)
    }

    fn market_buy_order() -> NewOrder {
        NewOrder {
            instrument: es_ticker(),
            side: OrderSide::Buy,
            order_type: OrderType::Market,
            quantity: 1.0,
            time_in_force: TimeInForce::GTC,
            label: None,
            reduce_only: false,
        }
    }

    fn limit_buy_order(price: f64) -> NewOrder {
        NewOrder {
            instrument: es_ticker(),
            side: OrderSide::Buy,
            order_type: OrderType::Limit {
                price: Price::from_f64(price),
            },
            quantity: 1.0,
            time_in_force: TimeInForce::GTC,
            label: None,
            reduce_only: false,
        }
    }

    fn day_order() -> NewOrder {
        NewOrder {
            instrument: es_ticker(),
            side: OrderSide::Buy,
            order_type: OrderType::Limit {
                price: Price::from_f64(5000.0),
            },
            quantity: 1.0,
            time_in_force: TimeInForce::Day,
            label: None,
            reduce_only: false,
        }
    }

    // ── Create / Active ──────────────────────────────────────────

    #[test]
    fn test_create_order_is_active() {
        let mut book = OrderBook::new();
        OrderId::reset();
        let id = book.create_order(&market_buy_order(), ts(1000));

        let order = book.get(id).unwrap();
        assert_eq!(order.status, OrderStatus::Active);
        assert_eq!(book.active_count(), 1);
    }

    #[test]
    fn test_empty_book() {
        let book = OrderBook::new();
        assert_eq!(book.active_count(), 0);
        assert!(book.get(OrderId(999)).is_none());
    }

    // ── Fill lifecycle ───────────────────────────────────────────

    #[test]
    fn test_fill_order_fully() {
        let mut book = OrderBook::new();
        OrderId::reset();
        let id = book.create_order(&market_buy_order(), ts(1000));

        let order = book.get_mut(id).unwrap();
        order.record_fill(1.0, Price::from_f64(5000.0), ts(1001));

        assert_eq!(book.get(id).unwrap().status, OrderStatus::Filled);
        assert!(book.get(id).unwrap().is_terminal());
    }

    #[test]
    fn test_partial_fill() {
        let mut new = market_buy_order();
        new.quantity = 5.0;
        let mut book = OrderBook::new();
        OrderId::reset();
        let id = book.create_order(&new, ts(1000));

        let order = book.get_mut(id).unwrap();
        order.record_fill(3.0, Price::from_f64(5000.0), ts(1001));

        let order = book.get(id).unwrap();
        assert_eq!(order.status, OrderStatus::PartiallyFilled);
        assert!((order.remaining_quantity() - 2.0).abs() < 1e-9);
        assert!(order.is_active());
    }

    // ── Cancel ───────────────────────────────────────────────────

    #[test]
    fn test_cancel_order() {
        let mut book = OrderBook::new();
        OrderId::reset();
        let id = book.create_order(&limit_buy_order(5000.0), ts(1000));

        book.cancel(id, ts(1001));

        assert_eq!(book.get(id).unwrap().status, OrderStatus::Cancelled);
        assert_eq!(book.active_count(), 0);
    }

    #[test]
    fn test_cancel_already_terminal_is_noop() {
        let mut book = OrderBook::new();
        OrderId::reset();
        let id = book.create_order(&market_buy_order(), ts(1000));

        // Fill it fully
        book.get_mut(id)
            .unwrap()
            .record_fill(1.0, Price::from_f64(5000.0), ts(1001));

        // Cancel a filled order should not change state
        book.cancel(id, ts(1002));
        assert_eq!(book.get(id).unwrap().status, OrderStatus::Filled);
    }

    #[test]
    fn test_cancel_all() {
        let mut book = OrderBook::new();
        OrderId::reset();
        let _id1 = book.create_order(&market_buy_order(), ts(1000));
        let _id2 = book.create_order(&limit_buy_order(4950.0), ts(1001));
        let _id3 = book.create_order(&limit_buy_order(4900.0), ts(1002));

        assert_eq!(book.active_count(), 3);
        book.cancel_all(None, ts(1003));
        assert_eq!(book.active_count(), 0);
    }

    #[test]
    fn test_cancel_all_filtered_by_instrument() {
        let mut book = OrderBook::new();
        OrderId::reset();
        let _es_id = book.create_order(&market_buy_order(), ts(1000));

        let nq = FuturesTicker::new("NQ.c.0", kairos_data::FuturesVenue::CMEGlobex);
        let nq_order = NewOrder {
            instrument: nq,
            side: OrderSide::Buy,
            order_type: OrderType::Market,
            quantity: 1.0,
            time_in_force: TimeInForce::GTC,
            label: None,
            reduce_only: false,
        };
        let _nq_id = book.create_order(&nq_order, ts(1001));

        assert_eq!(book.active_count(), 2);

        // Cancel only ES orders
        book.cancel_all(Some(es_ticker()), ts(1002));
        assert_eq!(book.active_count(), 1);
    }

    // ── Modify ───────────────────────────────────────────────────

    #[test]
    fn test_modify_limit_price() {
        let mut book = OrderBook::new();
        OrderId::reset();
        let id = book.create_order(&limit_buy_order(5000.0), ts(1000));

        let success = book.modify(id, Some(Price::from_f64(4990.0)), None, ts(1001));
        assert!(success);

        let order = book.get(id).unwrap();
        match order.order_type {
            OrderType::Limit { price } => {
                assert!((price.to_f64() - 4990.0).abs() < 1e-10);
            }
            _ => panic!("Expected Limit order type"),
        }
    }

    #[test]
    fn test_modify_quantity() {
        let mut book = OrderBook::new();
        OrderId::reset();
        let id = book.create_order(&market_buy_order(), ts(1000));

        let success = book.modify(id, None, Some(3.0), ts(1001));
        assert!(success);
        assert!((book.get(id).unwrap().quantity - 3.0).abs() < 1e-9);
    }

    #[test]
    fn test_modify_terminal_returns_false() {
        let mut book = OrderBook::new();
        OrderId::reset();
        let id = book.create_order(&market_buy_order(), ts(1000));
        book.cancel(id, ts(1001));

        let success = book.modify(id, None, Some(5.0), ts(1002));
        assert!(!success);
    }

    #[test]
    fn test_modify_nonexistent_returns_false() {
        let mut book = OrderBook::new();
        let success = book.modify(OrderId(9999), None, Some(5.0), ts(1002));
        assert!(!success);
    }

    // ── Day order expiration ─────────────────────────────────────

    #[test]
    fn test_expire_day_orders() {
        let mut book = OrderBook::new();
        OrderId::reset();
        let day_id = book.create_order(&day_order(), ts(1000));
        let gtc_id = book.create_order(&limit_buy_order(4990.0), ts(1001));

        book.expire_day_orders(ts(86_400_000));

        assert_eq!(book.get(day_id).unwrap().status, OrderStatus::Expired);
        assert_eq!(book.get(gtc_id).unwrap().status, OrderStatus::Active);
        assert_eq!(book.active_count(), 1);
    }

    // ── Bracket orders ───────────────────────────────────────────

    #[test]
    fn test_bracket_order_creation() {
        let mut book = OrderBook::new();
        OrderId::reset();
        let bracket = BracketOrder {
            entry: NewOrder {
                instrument: es_ticker(),
                side: OrderSide::Buy,
                order_type: OrderType::Market,
                quantity: 1.0,
                time_in_force: TimeInForce::GTC,
                label: None,
                reduce_only: false,
            },
            stop_loss: Price::from_f64(4990.0),
            take_profit: Some(Price::from_f64(5020.0)),
        };

        let (entry_id, sl_id, tp_id) = book.create_bracket(&bracket, ts(1000));

        // Entry is active
        assert_eq!(book.get(entry_id).unwrap().status, OrderStatus::Active);
        // SL is pending
        assert_eq!(book.get(sl_id).unwrap().status, OrderStatus::Pending);
        // TP is pending
        let tp_id = tp_id.unwrap();
        assert_eq!(book.get(tp_id).unwrap().status, OrderStatus::Pending);
        // OCO linkage
        assert_eq!(book.get(sl_id).unwrap().oco_partner, Some(tp_id));
        assert_eq!(book.get(tp_id).unwrap().oco_partner, Some(sl_id));
        // Parent linkage
        assert_eq!(book.get(sl_id).unwrap().parent_id, Some(entry_id));
        assert_eq!(book.get(tp_id).unwrap().parent_id, Some(entry_id));
    }

    #[test]
    fn test_activate_bracket_children() {
        let mut book = OrderBook::new();
        OrderId::reset();
        let bracket = BracketOrder {
            entry: NewOrder {
                instrument: es_ticker(),
                side: OrderSide::Buy,
                order_type: OrderType::Market,
                quantity: 1.0,
                time_in_force: TimeInForce::GTC,
                label: None,
                reduce_only: false,
            },
            stop_loss: Price::from_f64(4990.0),
            take_profit: Some(Price::from_f64(5020.0)),
        };

        let (entry_id, sl_id, tp_id) = book.create_bracket(&bracket, ts(1000));
        let tp_id = tp_id.unwrap();

        book.activate_bracket_children(entry_id);

        assert_eq!(book.get(sl_id).unwrap().status, OrderStatus::Active);
        assert_eq!(book.get(tp_id).unwrap().status, OrderStatus::Active);
    }

    #[test]
    fn test_bracket_oco_cancel() {
        let mut book = OrderBook::new();
        OrderId::reset();
        let bracket = BracketOrder {
            entry: NewOrder {
                instrument: es_ticker(),
                side: OrderSide::Buy,
                order_type: OrderType::Market,
                quantity: 1.0,
                time_in_force: TimeInForce::GTC,
                label: None,
                reduce_only: false,
            },
            stop_loss: Price::from_f64(4990.0),
            take_profit: Some(Price::from_f64(5020.0)),
        };

        let (entry_id, sl_id, tp_id) = book.create_bracket(&bracket, ts(1000));
        let tp_id = tp_id.unwrap();

        // Activate children
        book.activate_bracket_children(entry_id);

        // Cancel the SL => OCO partner (TP) should also be cancelled
        book.cancel(sl_id, ts(1002));

        assert_eq!(book.get(sl_id).unwrap().status, OrderStatus::Cancelled);
        assert_eq!(book.get(tp_id).unwrap().status, OrderStatus::Cancelled);
    }

    #[test]
    fn test_bracket_without_take_profit() {
        let mut book = OrderBook::new();
        OrderId::reset();
        let bracket = BracketOrder {
            entry: NewOrder {
                instrument: es_ticker(),
                side: OrderSide::Buy,
                order_type: OrderType::Market,
                quantity: 1.0,
                time_in_force: TimeInForce::GTC,
                label: None,
                reduce_only: false,
            },
            stop_loss: Price::from_f64(4990.0),
            take_profit: None,
        };

        let (entry_id, sl_id, tp_id) = book.create_bracket(&bracket, ts(1000));

        assert!(tp_id.is_none());
        assert_eq!(book.get(sl_id).unwrap().oco_partner, None);
        assert_eq!(book.get(sl_id).unwrap().parent_id, Some(entry_id));
    }

    #[test]
    fn test_bracket_stop_loss_lookup() {
        let mut book = OrderBook::new();
        OrderId::reset();
        let bracket = BracketOrder {
            entry: NewOrder {
                instrument: es_ticker(),
                side: OrderSide::Buy,
                order_type: OrderType::Market,
                quantity: 1.0,
                time_in_force: TimeInForce::GTC,
                label: None,
                reduce_only: false,
            },
            stop_loss: Price::from_f64(4990.0),
            take_profit: None,
        };

        let (entry_id, _sl_id, _tp_id) = book.create_bracket(&bracket, ts(1000));

        let sl_price = book.bracket_stop_loss(entry_id).unwrap();
        assert!((sl_price.to_f64() - 4990.0).abs() < 1e-10);
    }

    // ── Reset ────────────────────────────────────────────────────

    #[test]
    fn test_reset_clears_all() {
        let mut book = OrderBook::new();
        OrderId::reset();
        book.create_order(&market_buy_order(), ts(1000));
        book.create_order(&market_buy_order(), ts(1001));

        assert_eq!(book.active_count(), 2);
        book.reset();
        assert_eq!(book.active_count(), 0);
    }
}
