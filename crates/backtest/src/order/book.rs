use crate::order::entity::Order;
use crate::order::request::{BracketOrder, NewOrder};
use crate::order::types::{OrderId, OrderStatus, OrderType, TimeInForce};
use kairos_data::{FuturesTicker, Price, Timestamp};
use std::collections::HashMap;

/// Manages all orders in the backtest engine.
pub struct OrderBook {
    orders: HashMap<OrderId, Order>,
    active_ids: Vec<OrderId>,
}

impl OrderBook {
    pub fn new() -> Self {
        Self {
            orders: HashMap::new(),
            active_ids: Vec::new(),
        }
    }

    pub fn get(&self, id: OrderId) -> Option<&Order> {
        self.orders.get(&id)
    }

    pub fn get_mut(&mut self, id: OrderId) -> Option<&mut Order> {
        self.orders.get_mut(&id)
    }

    pub fn active_orders(&self) -> impl Iterator<Item = &Order> {
        self.active_ids
            .iter()
            .filter_map(|id| self.orders.get(id))
            .filter(|o| o.is_active())
    }

    pub fn active_count(&self) -> usize {
        self.active_ids
            .iter()
            .filter(|id| self.orders.get(id).is_some_and(|o| o.is_active()))
            .count()
    }

    /// Create and activate a single order.
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
    /// take-profit. The SL and TP are OCO partners.
    pub fn create_bracket(
        &mut self,
        bracket: &BracketOrder,
        timestamp: Timestamp,
    ) -> (OrderId, OrderId, Option<OrderId>) {
        let entry_id = self.create_order(&bracket.entry, timestamp);
        let exit_side = bracket.entry.side.opposite();

        // Stop-loss order
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

        // Take-profit order (if specified)
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

        // Link OCO partners
        if let Some(tp_id) = tp_id
            && let Some(sl) = self.orders.get_mut(&sl_id)
        {
            sl.oco_partner = Some(tp_id);
        }

        (entry_id, sl_id, tp_id)
    }

    /// Activate pending bracket orders (called after entry fill).
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

    /// Find the stop-loss price for a bracket entry order (from its
    /// child SL order).
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

    /// Cancel an order. If it has an OCO partner, cancel that too.
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

    /// Cancel all orders for an instrument.
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

    /// Modify an order's price and/or quantity.
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

    /// Expire all Day orders.
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

    /// Remove terminal orders from active list.
    fn cleanup_active(&mut self) {
        self.active_ids
            .retain(|id| self.orders.get(id).is_some_and(|o| !o.is_terminal()));
    }

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
