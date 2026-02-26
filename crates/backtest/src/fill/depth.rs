use super::market::StandardFillSimulator;
use super::{FillResult, FillSimulator};
use crate::config::instrument::InstrumentSpec;
use crate::config::risk::SlippageModel;
use crate::order::entity::Order;
use crate::order::types::OrderSide;
use kairos_data::{Depth, FuturesTicker, Price, Trade};
use std::collections::HashMap;

/// Fill simulator that walks order-book depth for realistic
/// pricing. Falls back to StandardFillSimulator when no depth
/// data is available.
pub struct DepthBasedFillSimulator {
    fallback: StandardFillSimulator,
}

impl DepthBasedFillSimulator {
    pub fn new(slippage: SlippageModel) -> Self {
        Self {
            fallback: StandardFillSimulator::new(slippage),
        }
    }

    /// Walk the book to compute average fill price for `quantity`
    /// contracts.
    fn walk_book(
        &self,
        levels: &std::collections::BTreeMap<i64, f32>,
        quantity: f64,
        ascending: bool,
    ) -> Option<Price> {
        let mut remaining = quantity;
        let mut cost = 0.0;
        let mut filled = 0.0;

        let iter: Box<dyn Iterator<Item = (&i64, &f32)>> = if ascending {
            Box::new(levels.iter())
        } else {
            Box::new(levels.iter().rev())
        };

        for (price_units, qty) in iter {
            let available = (*qty as f64).min(remaining);
            cost += Price::from_units(*price_units).to_f64() * available;
            filled += available;
            remaining -= available;
            if remaining <= 0.0 {
                break;
            }
        }

        if filled > 0.0 {
            Some(Price::from_f64(cost / filled))
        } else {
            None
        }
    }
}

impl FillSimulator for DepthBasedFillSimulator {
    fn check_fills(
        &self,
        trade: &Trade,
        depth: Option<&Depth>,
        active_orders: &[&Order],
        instruments: &HashMap<FuturesTicker, InstrumentSpec>,
    ) -> Vec<FillResult> {
        // Delegate trigger/fill logic to standard simulator,
        // but override fill prices using depth when available.
        let mut base_fills = self
            .fallback
            .check_fills(trade, depth, active_orders, instruments);

        if let Some(depth) = depth {
            for fill in &mut base_fills {
                let order = active_orders.iter().find(|o| o.id == fill.order_id);
                if let Some(order) = order {
                    let book_side = match order.side {
                        OrderSide::Buy => &depth.asks,
                        OrderSide::Sell => &depth.bids,
                    };
                    let ascending = order.side == OrderSide::Buy;
                    if let Some(avg_price) =
                        self.walk_book(book_side, fill.fill_quantity, ascending)
                    {
                        fill.fill_price = avg_price;
                    }
                }
            }
        }

        base_fills
    }

    fn market_fill_price(
        &self,
        trade: &Trade,
        side: OrderSide,
        quantity: f64,
        depth: Option<&Depth>,
        instrument: &InstrumentSpec,
    ) -> Price {
        if let Some(depth) = depth {
            let book_side = match side {
                OrderSide::Buy => &depth.asks,
                OrderSide::Sell => &depth.bids,
            };
            let ascending = side == OrderSide::Buy;
            if let Some(avg_price) = self.walk_book(book_side, quantity, ascending) {
                return avg_price;
            }
        }
        self.fallback
            .market_fill_price(trade, side, quantity, depth, instrument)
    }

    fn clone_simulator(&self) -> Box<dyn FillSimulator> {
        Box::new(DepthBasedFillSimulator {
            fallback: StandardFillSimulator::new(SlippageModel::None),
        })
    }
}
