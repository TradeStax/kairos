use super::{FillResult, FillSimulator};
use crate::config::instrument::InstrumentSpec;
use crate::config::risk::SlippageModel;
use crate::order::entity::Order;
use crate::order::types::{OrderSide, OrderType};
use kairos_data::{Depth, FuturesTicker, Price, Trade};
use std::collections::HashMap;

/// Standard fill simulator: slippage-based for market orders,
/// trigger-based for stop/limit.
pub struct StandardFillSimulator {
    slippage: SlippageModel,
}

impl StandardFillSimulator {
    pub fn new(slippage: SlippageModel) -> Self {
        Self { slippage }
    }

    fn apply_slippage(&self, base: Price, side: OrderSide, instrument: &InstrumentSpec) -> Price {
        match &self.slippage {
            SlippageModel::None => base,
            SlippageModel::FixedTick(n) => {
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
                let impact_bps = base_bps * (1.0_f64 / average_daily_volume).sqrt();
                let factor = match side {
                    OrderSide::Buy => 1.0 + impact_bps / 10_000.0,
                    OrderSide::Sell => 1.0 - impact_bps / 10_000.0,
                };
                Price::from_f64(base.to_f64() * factor)
            }
            // DepthBased is handled by DepthBasedFillSimulator
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
        Box::new(StandardFillSimulator {
            slippage: self.slippage.clone(),
        })
    }
}
