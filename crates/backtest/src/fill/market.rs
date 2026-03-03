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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::instrument::InstrumentSpec;
    use crate::config::risk::SlippageModel;
    use crate::order::entity::Order;
    use crate::order::types::{OrderId, OrderSide, OrderStatus, OrderType, TimeInForce};
    use kairos_data::{FuturesTicker, FuturesVenue, Price, Quantity, Side, Timestamp, Trade};
    use std::collections::HashMap;

    fn es_ticker() -> FuturesTicker {
        FuturesTicker::new("ES.c.0", FuturesVenue::CMEGlobex)
    }

    fn es_spec() -> InstrumentSpec {
        InstrumentSpec::new(es_ticker(), Price::from_f64(0.25), 50.0)
    }

    fn make_instruments() -> HashMap<FuturesTicker, InstrumentSpec> {
        let mut m = HashMap::new();
        m.insert(es_ticker(), es_spec());
        m
    }

    fn make_trade(price: f64) -> Trade {
        Trade {
            time: Timestamp(1000),
            price: Price::from_f64(price),
            quantity: Quantity(1.0),
            side: Side::Buy,
        }
    }

    fn make_active_order(id_val: u64, side: OrderSide, order_type: OrderType, qty: f64) -> Order {
        Order {
            id: OrderId(id_val),
            instrument: es_ticker(),
            side,
            order_type,
            time_in_force: TimeInForce::GTC,
            quantity: qty,
            filled_quantity: 0.0,
            avg_fill_price: None,
            status: OrderStatus::Active,
            created_at: Timestamp(500),
            updated_at: Timestamp(500),
            label: None,
            parent_id: None,
            oco_partner: None,
            reduce_only: false,
        }
    }

    // ── Market order fills ───────────────────────────────────────

    #[test]
    fn test_market_order_fills_immediately_no_slippage() {
        let sim = StandardFillSimulator::new(SlippageModel::None);
        let trade = make_trade(5000.0);
        let order = make_active_order(1, OrderSide::Buy, OrderType::Market, 1.0);
        let orders: Vec<&Order> = vec![&order];

        let fills = sim.check_fills(&trade, None, &orders, &make_instruments());

        assert_eq!(fills.len(), 1);
        assert!((fills[0].fill_price.to_f64() - 5000.0).abs() < 1e-10);
        assert!((fills[0].fill_quantity - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_market_order_with_fixed_tick_slippage_buy() {
        let sim = StandardFillSimulator::new(SlippageModel::FixedTick(2));
        let trade = make_trade(5000.0);
        let order = make_active_order(1, OrderSide::Buy, OrderType::Market, 1.0);
        let orders: Vec<&Order> = vec![&order];

        let fills = sim.check_fills(&trade, None, &orders, &make_instruments());

        assert_eq!(fills.len(), 1);
        // Buy slippage: +2 ticks * 0.25 = +0.50
        assert!((fills[0].fill_price.to_f64() - 5000.50).abs() < 1e-10);
    }

    #[test]
    fn test_market_order_with_fixed_tick_slippage_sell() {
        let sim = StandardFillSimulator::new(SlippageModel::FixedTick(2));
        let trade = make_trade(5000.0);
        let order = make_active_order(1, OrderSide::Sell, OrderType::Market, 1.0);
        let orders: Vec<&Order> = vec![&order];

        let fills = sim.check_fills(&trade, None, &orders, &make_instruments());

        assert_eq!(fills.len(), 1);
        // Sell slippage: -2 ticks * 0.25 = -0.50
        assert!((fills[0].fill_price.to_f64() - 4999.50).abs() < 1e-10);
    }

    #[test]
    fn test_market_order_with_percentage_slippage() {
        let sim = StandardFillSimulator::new(SlippageModel::Percentage(0.001));
        let trade = make_trade(5000.0);
        let order = make_active_order(1, OrderSide::Buy, OrderType::Market, 1.0);
        let orders: Vec<&Order> = vec![&order];

        let fills = sim.check_fills(&trade, None, &orders, &make_instruments());

        assert_eq!(fills.len(), 1);
        // Buy: 5000 * 1.001 = 5005.0
        assert!((fills[0].fill_price.to_f64() - 5005.0).abs() < 0.01);
    }

    // ── Limit order fills ────────────────────────────────────────

    #[test]
    fn test_limit_buy_fills_at_limit_price_when_trade_at_limit() {
        let sim = StandardFillSimulator::new(SlippageModel::None);
        let trade = make_trade(4990.0);
        let order = make_active_order(
            1,
            OrderSide::Buy,
            OrderType::Limit {
                price: Price::from_f64(4990.0),
            },
            1.0,
        );
        let orders: Vec<&Order> = vec![&order];

        let fills = sim.check_fills(&trade, None, &orders, &make_instruments());

        assert_eq!(fills.len(), 1);
        assert!((fills[0].fill_price.to_f64() - 4990.0).abs() < 1e-10);
    }

    #[test]
    fn test_limit_buy_fills_when_trade_below_limit() {
        let sim = StandardFillSimulator::new(SlippageModel::None);
        let trade = make_trade(4985.0);
        let order = make_active_order(
            1,
            OrderSide::Buy,
            OrderType::Limit {
                price: Price::from_f64(4990.0),
            },
            1.0,
        );
        let orders: Vec<&Order> = vec![&order];

        let fills = sim.check_fills(&trade, None, &orders, &make_instruments());

        assert_eq!(fills.len(), 1);
        // Fills at the limit price, not the trade price
        assert!((fills[0].fill_price.to_f64() - 4990.0).abs() < 1e-10);
    }

    #[test]
    fn test_limit_buy_does_not_fill_when_price_above() {
        let sim = StandardFillSimulator::new(SlippageModel::None);
        let trade = make_trade(5000.0);
        let order = make_active_order(
            1,
            OrderSide::Buy,
            OrderType::Limit {
                price: Price::from_f64(4990.0),
            },
            1.0,
        );
        let orders: Vec<&Order> = vec![&order];

        let fills = sim.check_fills(&trade, None, &orders, &make_instruments());

        assert!(fills.is_empty());
    }

    #[test]
    fn test_limit_sell_fills_when_trade_above_limit() {
        let sim = StandardFillSimulator::new(SlippageModel::None);
        let trade = make_trade(5015.0);
        let order = make_active_order(
            1,
            OrderSide::Sell,
            OrderType::Limit {
                price: Price::from_f64(5010.0),
            },
            1.0,
        );
        let orders: Vec<&Order> = vec![&order];

        let fills = sim.check_fills(&trade, None, &orders, &make_instruments());

        assert_eq!(fills.len(), 1);
        assert!((fills[0].fill_price.to_f64() - 5010.0).abs() < 1e-10);
    }

    #[test]
    fn test_limit_sell_does_not_fill_when_price_below() {
        let sim = StandardFillSimulator::new(SlippageModel::None);
        let trade = make_trade(5005.0);
        let order = make_active_order(
            1,
            OrderSide::Sell,
            OrderType::Limit {
                price: Price::from_f64(5010.0),
            },
            1.0,
        );
        let orders: Vec<&Order> = vec![&order];

        let fills = sim.check_fills(&trade, None, &orders, &make_instruments());

        assert!(fills.is_empty());
    }

    // ── Stop order fills ─────────────────────────────────────────

    #[test]
    fn test_stop_buy_triggers_when_price_reaches_trigger() {
        let sim = StandardFillSimulator::new(SlippageModel::None);
        let trade = make_trade(5020.0);
        let order = make_active_order(
            1,
            OrderSide::Buy,
            OrderType::Stop {
                trigger: Price::from_f64(5020.0),
            },
            1.0,
        );
        let orders: Vec<&Order> = vec![&order];

        let fills = sim.check_fills(&trade, None, &orders, &make_instruments());

        assert_eq!(fills.len(), 1);
        // Fills at trade price (no slippage)
        assert!((fills[0].fill_price.to_f64() - 5020.0).abs() < 1e-10);
    }

    #[test]
    fn test_stop_buy_does_not_trigger_below() {
        let sim = StandardFillSimulator::new(SlippageModel::None);
        let trade = make_trade(5015.0);
        let order = make_active_order(
            1,
            OrderSide::Buy,
            OrderType::Stop {
                trigger: Price::from_f64(5020.0),
            },
            1.0,
        );
        let orders: Vec<&Order> = vec![&order];

        let fills = sim.check_fills(&trade, None, &orders, &make_instruments());
        assert!(fills.is_empty());
    }

    #[test]
    fn test_stop_sell_triggers_when_price_falls() {
        let sim = StandardFillSimulator::new(SlippageModel::None);
        let trade = make_trade(4990.0);
        let order = make_active_order(
            1,
            OrderSide::Sell,
            OrderType::Stop {
                trigger: Price::from_f64(4990.0),
            },
            1.0,
        );
        let orders: Vec<&Order> = vec![&order];

        let fills = sim.check_fills(&trade, None, &orders, &make_instruments());
        assert_eq!(fills.len(), 1);
    }

    #[test]
    fn test_stop_with_slippage() {
        let sim = StandardFillSimulator::new(SlippageModel::FixedTick(1));
        let trade = make_trade(5020.0);
        let order = make_active_order(
            1,
            OrderSide::Buy,
            OrderType::Stop {
                trigger: Price::from_f64(5020.0),
            },
            1.0,
        );
        let orders: Vec<&Order> = vec![&order];

        let fills = sim.check_fills(&trade, None, &orders, &make_instruments());

        assert_eq!(fills.len(), 1);
        // Triggered, then slippage applied: 5020 + 1 tick (0.25) = 5020.25
        assert!((fills[0].fill_price.to_f64() - 5020.25).abs() < 1e-10);
    }

    // ── StopLimit order fills ────────────────────────────────────

    #[test]
    fn test_stop_limit_buy_both_conditions_met() {
        let sim = StandardFillSimulator::new(SlippageModel::None);
        // trigger=5020, limit=5025; trade at 5022 => trigger met, under limit => fill at limit
        let trade = make_trade(5022.0);
        let order = make_active_order(
            1,
            OrderSide::Buy,
            OrderType::StopLimit {
                trigger: Price::from_f64(5020.0),
                limit: Price::from_f64(5025.0),
            },
            1.0,
        );
        let orders: Vec<&Order> = vec![&order];

        let fills = sim.check_fills(&trade, None, &orders, &make_instruments());

        assert_eq!(fills.len(), 1);
        assert!((fills[0].fill_price.to_f64() - 5025.0).abs() < 1e-10);
    }

    #[test]
    fn test_stop_limit_buy_trigger_met_limit_not() {
        let sim = StandardFillSimulator::new(SlippageModel::None);
        // trigger=5020, limit=5025; trade at 5030 => trigger met, above limit => no fill
        let trade = make_trade(5030.0);
        let order = make_active_order(
            1,
            OrderSide::Buy,
            OrderType::StopLimit {
                trigger: Price::from_f64(5020.0),
                limit: Price::from_f64(5025.0),
            },
            1.0,
        );
        let orders: Vec<&Order> = vec![&order];

        let fills = sim.check_fills(&trade, None, &orders, &make_instruments());
        assert!(fills.is_empty());
    }

    // ── Multiple orders ──────────────────────────────────────────

    #[test]
    fn test_multiple_orders_checked() {
        let sim = StandardFillSimulator::new(SlippageModel::None);
        let trade = make_trade(5000.0);
        let o1 = make_active_order(1, OrderSide::Buy, OrderType::Market, 1.0);
        let o2 = make_active_order(
            2,
            OrderSide::Buy,
            OrderType::Limit {
                price: Price::from_f64(5000.0),
            },
            2.0,
        );
        // This limit won't fill
        let o3 = make_active_order(
            3,
            OrderSide::Buy,
            OrderType::Limit {
                price: Price::from_f64(4990.0),
            },
            1.0,
        );
        let orders: Vec<&Order> = vec![&o1, &o2, &o3];

        let fills = sim.check_fills(&trade, None, &orders, &make_instruments());

        assert_eq!(fills.len(), 2);
        assert_eq!(fills[0].order_id, OrderId(1));
        assert_eq!(fills[1].order_id, OrderId(2));
    }

    // ── Zero remaining quantity ──────────────────────────────────

    #[test]
    fn test_zero_remaining_quantity_skipped() {
        let sim = StandardFillSimulator::new(SlippageModel::None);
        let trade = make_trade(5000.0);
        let mut order = make_active_order(1, OrderSide::Buy, OrderType::Market, 1.0);
        order.filled_quantity = 1.0; // fully filled already
        let orders: Vec<&Order> = vec![&order];

        let fills = sim.check_fills(&trade, None, &orders, &make_instruments());
        assert!(fills.is_empty());
    }

    // ── VolumeImpact slippage ────────────────────────────────────

    #[test]
    fn test_volume_impact_slippage() {
        let sim = StandardFillSimulator::new(SlippageModel::VolumeImpact {
            base_bps: 5.0,
            average_daily_volume: 100_000.0,
        });
        let trade = make_trade(5000.0);
        let order = make_active_order(1, OrderSide::Buy, OrderType::Market, 1.0);
        let orders: Vec<&Order> = vec![&order];

        let fills = sim.check_fills(&trade, None, &orders, &make_instruments());

        assert_eq!(fills.len(), 1);
        // Should have some adverse slippage (price > 5000 for buy)
        assert!(fills[0].fill_price.to_f64() > 5000.0);
    }

    #[test]
    fn test_volume_impact_zero_adv() {
        let sim = StandardFillSimulator::new(SlippageModel::VolumeImpact {
            base_bps: 5.0,
            average_daily_volume: 0.0,
        });
        let trade = make_trade(5000.0);
        let order = make_active_order(1, OrderSide::Buy, OrderType::Market, 1.0);
        let orders: Vec<&Order> = vec![&order];

        let fills = sim.check_fills(&trade, None, &orders, &make_instruments());

        assert_eq!(fills.len(), 1);
        // Zero ADV => no slippage applied
        assert!((fills[0].fill_price.to_f64() - 5000.0).abs() < 1e-10);
    }

    // ── market_fill_price method ─────────────────────────────────

    #[test]
    fn test_market_fill_price_method() {
        let sim = StandardFillSimulator::new(SlippageModel::FixedTick(1));
        let trade = make_trade(5000.0);

        let buy_price = sim.market_fill_price(&trade, OrderSide::Buy, 1.0, None, &es_spec());
        let sell_price = sim.market_fill_price(&trade, OrderSide::Sell, 1.0, None, &es_spec());

        assert!((buy_price.to_f64() - 5000.25).abs() < 1e-10);
        assert!((sell_price.to_f64() - 4999.75).abs() < 1e-10);
    }
}
