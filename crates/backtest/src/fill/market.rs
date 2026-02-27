//! Standard fill simulator with configurable slippage models.
//!
//! [`StandardFillSimulator`] evaluates each active order against
//! the latest trade price:
//!
//! - **Market** orders fill immediately with slippage applied.
//! - **Limit** orders fill at their limit price when the trade
//!   crosses the limit.
//! - **Stop** orders trigger when the trade crosses the stop
//!   price, then fill as a market order (with slippage).
//! - **StopLimit** orders trigger like stops but only fill if the
//!   trade price also satisfies the limit constraint.
//!
//! Slippage is controlled by [`SlippageModel`] from the risk
//! configuration.

use super::{FillResult, FillSimulator};
use crate::config::instrument::InstrumentSpec;
use crate::config::risk::SlippageModel;
use crate::order::entity::Order;
use crate::order::types::{OrderSide, OrderType};
use kairos_data::{Depth, FuturesTicker, Price, Trade};
use std::collections::HashMap;

/// Standard fill simulator: slippage-based pricing for market and
/// stop orders, trigger-based fills for limit and stop-limit orders.
#[derive(Debug, Clone)]
pub struct StandardFillSimulator {
    slippage: SlippageModel,
}

impl StandardFillSimulator {
    /// Create a new simulator with the given slippage model.
    #[must_use]
    pub fn new(slippage: SlippageModel) -> Self {
        Self { slippage }
    }

    /// Return the slippage model in use.
    #[must_use]
    pub fn slippage_model(&self) -> &SlippageModel {
        &self.slippage
    }

    /// Apply the configured slippage model to a `base` price.
    ///
    /// Slippage is always adverse: buys are pushed up, sells are
    /// pushed down, simulating the cost of crossing the spread or
    /// moving the market.
    fn apply_slippage(&self, base: Price, side: OrderSide, instrument: &InstrumentSpec) -> Price {
        match &self.slippage {
            SlippageModel::None => base,
            SlippageModel::FixedTick(n) => {
                // Positive = adverse: buy gets worse (higher),
                // sell gets worse (lower).
                let steps = match side {
                    OrderSide::Buy => *n,
                    OrderSide::Sell => -n,
                };
                base.add_steps(steps, instrument.tick_size)
            }
            SlippageModel::Percentage(pct) => {
                let factor = match side {
                    OrderSide::Buy => 1.0 + pct,
                    OrderSide::Sell => 1.0 - pct,
                };
                Price::from_f64(base.to_f64() * factor)
            }
            SlippageModel::VolumeImpact {
                base_bps,
                average_daily_volume,
            } => {
                if *average_daily_volume <= 0.0 {
                    return base;
                }
                // Square-root market impact model:
                //   impact = base_bps * sqrt(1 / ADV)
                // The idea is that thinner markets (lower ADV)
                // produce more slippage per unit traded.
                let impact_bps = base_bps * (1.0_f64 / average_daily_volume).sqrt();
                let factor = match side {
                    OrderSide::Buy => 1.0 + impact_bps / 10_000.0,
                    OrderSide::Sell => 1.0 - impact_bps / 10_000.0,
                };
                Price::from_f64(base.to_f64() * factor)
            }
            // DepthBased slippage is handled by
            // DepthBasedFillSimulator; a no-op here.
            SlippageModel::DepthBased => base,
        }
    }
}

impl FillSimulator for StandardFillSimulator {
    fn check_fills(
        &self,
        trade: &Trade,
        _depth: Option<&Depth>,
        active_orders: &[&Order],
        instruments: &HashMap<FuturesTicker, InstrumentSpec>,
    ) -> Vec<FillResult> {
        let mut fills = Vec::new();
        let price = trade.price;

        for order in active_orders {
            let instrument = match instruments.get(&order.instrument) {
                Some(i) => i,
                None => continue,
            };
            let remaining = order.remaining_quantity();
            if remaining <= 0.0 {
                continue;
            }

            match order.order_type {
                OrderType::Market => {
                    let fill_price = self.apply_slippage(price, order.side, instrument);
                    fills.push(FillResult {
                        order_id: order.id,
                        fill_price,
                        fill_quantity: remaining,
                        timestamp: trade.time,
                    });
                }
                OrderType::Limit { price: limit_price } => {
                    // Limit buy triggers when market trades at or
                    // below the limit; limit sell when at or above.
                    let triggered = match order.side {
                        OrderSide::Buy => price <= limit_price,
                        OrderSide::Sell => price >= limit_price,
                    };
                    if triggered {
                        fills.push(FillResult {
                            order_id: order.id,
                            fill_price: limit_price,
                            fill_quantity: remaining,
                            timestamp: trade.time,
                        });
                    }
                }
                OrderType::Stop { trigger } => {
                    // Stop buy triggers when market trades at or
                    // above the trigger; stop sell when at or below.
                    // Once triggered the order becomes a market
                    // order and incurs slippage.
                    let triggered = match order.side {
                        OrderSide::Buy => price >= trigger,
                        OrderSide::Sell => price <= trigger,
                    };
                    if triggered {
                        let fill_price = self.apply_slippage(price, order.side, instrument);
                        fills.push(FillResult {
                            order_id: order.id,
                            fill_price,
                            fill_quantity: remaining,
                            timestamp: trade.time,
                        });
                    }
                }
                OrderType::StopLimit { trigger, limit } => {
                    // Two-phase check: first the stop trigger must
                    // fire, then the trade price must also satisfy
                    // the limit constraint.
                    let triggered = match order.side {
                        OrderSide::Buy => price >= trigger,
                        OrderSide::Sell => price <= trigger,
                    };
                    if triggered {
                        let limit_ok = match order.side {
                            OrderSide::Buy => price <= limit,
                            OrderSide::Sell => price >= limit,
                        };
                        if limit_ok {
                            fills.push(FillResult {
                                order_id: order.id,
                                fill_price: limit,
                                fill_quantity: remaining,
                                timestamp: trade.time,
                            });
                        }
                    }
                }
            }
        }

        fills
    }

    fn market_fill_price(
        &self,
        trade: &Trade,
        side: OrderSide,
        _quantity: f64,
        _depth: Option<&Depth>,
        instrument: &InstrumentSpec,
    ) -> Price {
        self.apply_slippage(trade.price, side, instrument)
    }

    fn clone_simulator(&self) -> Box<dyn FillSimulator> {
        Box::new(self.clone())
    }
}
