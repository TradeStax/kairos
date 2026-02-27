//! Depth-based fill simulator that walks order-book snapshots.
//!
//! [`DepthBasedFillSimulator`] computes a volume-weighted average
//! fill price by walking the relevant side of the order book
//! (asks for buys, bids for sells). When no depth snapshot is
//! available it delegates to [`StandardFillSimulator`].
//!
//! This provides more realistic fill pricing than a flat slippage
//! model because the fill cost depends on actual resting liquidity
//! at each price level.

use super::market::StandardFillSimulator;
use super::{FillResult, FillSimulator};
use crate::config::instrument::InstrumentSpec;
use crate::config::risk::SlippageModel;
use crate::order::entity::Order;
use crate::order::types::OrderSide;
use kairos_data::{Depth, FuturesTicker, Price, Trade};
use std::collections::HashMap;

/// Fill simulator that walks order-book depth for realistic
/// volume-weighted average fill pricing.
///
/// When a depth snapshot accompanies the trade tick the simulator
/// iterates through price levels on the relevant side of the book
/// (ascending asks for buys, descending bids for sells) and
/// accumulates cost until the order quantity is satisfied.
///
/// Falls back to [`StandardFillSimulator`] when no depth data is
/// available.
#[derive(Debug, Clone)]
pub struct DepthBasedFillSimulator {
    /// Fallback simulator used when depth is unavailable and for
    /// initial trigger detection.
    fallback: StandardFillSimulator,
}

impl DepthBasedFillSimulator {
    /// Create a new depth-based simulator.
    ///
    /// The `slippage` model is forwarded to the internal
    /// [`StandardFillSimulator`] which handles trigger detection
    /// and serves as a fallback when no depth data is present.
    #[must_use]
    pub fn new(slippage: SlippageModel) -> Self {
        Self {
            fallback: StandardFillSimulator::new(slippage),
        }
    }

    /// Walk price levels in the book to compute a volume-weighted
    /// average fill price for `quantity` contracts.
    ///
    /// `ascending` should be `true` for buy orders (walk asks from
    /// best to worst) and `false` for sell orders (walk bids from
    /// best to worst, i.e. highest first).
    ///
    /// Returns `None` if the book side is empty (zero liquidity).
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
            // Fill as many contracts as available at this level,
            // up to the remaining quantity needed.
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
        // Delegate trigger/fill logic to the standard simulator,
        // then override fill prices using depth when available.
        let mut base_fills = self
            .fallback
            .check_fills(trade, depth, active_orders, instruments);

        if let Some(depth) = depth {
            for fill in &mut base_fills {
                let order = active_orders.iter().find(|o| o.id == fill.order_id);
                if let Some(order) = order {
                    // Buy orders consume the ask side; sell orders
                    // consume the bid side.
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
        Box::new(self.clone())
    }
}
