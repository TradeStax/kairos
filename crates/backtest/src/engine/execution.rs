use crate::config::risk::SlippageModel;
use crate::domain::trade_record::ExitReason;
use crate::portfolio::order::FillEvent;
use crate::portfolio::position::OpenPosition;
use kairos_data::{Price, Side, Trade};

/// Direction of a fill, used to determine which way slippage works.
#[derive(Debug, Clone, Copy)]
pub enum FillDirection {
    Enter(Side),
    Exit(Side),
}

/// Simulated brokerage that checks SL/TP fills and applies slippage.
pub struct SimulatedBroker {
    slippage: SlippageModel,
    tick_size: Price,
    /// True if the next fill is the first of a new session (gap-fill detection).
    is_new_session: bool,
}

impl SimulatedBroker {
    pub fn new(slippage: SlippageModel, tick_size: Price) -> Self {
        Self { slippage, tick_size, is_new_session: false }
    }

    /// Signal to the broker that a new session has just started.
    /// The next fill will be flagged as a potential gap fill.
    pub fn mark_new_session(&mut self) {
        self.is_new_session = true;
    }

    /// Check if pending SL or TP on the open position should fill against this trade.
    ///
    /// Stop-loss is checked before take-profit (priority order).
    pub fn check_fills(&mut self, trade: &Trade, position: &OpenPosition) -> Option<FillEvent> {
        let price = trade.price;
        let gap = self.is_new_session;

        // --- Stop-loss check ---
        if let Some(sl) = position.stop_loss {
            let triggered = match position.side {
                Side::Buy => price <= sl,
                Side::Sell => price >= sl,
                _ => false,
            };
            if triggered {
                // On a gap, fill at the actual trade price (not the SL level)
                let base = if gap { price } else { sl };
                let fill_price =
                    self.apply_slippage(base, FillDirection::Exit(position.side));
                self.is_new_session = false;
                return Some(FillEvent {
                    fill_price,
                    exit_reason: ExitReason::StopLoss,
                    is_gap_fill: gap,
                });
            }
        }

        // --- Take-profit check ---
        if let Some(tp) = position.take_profit {
            let triggered = match position.side {
                Side::Buy => price >= tp,
                Side::Sell => price <= tp,
                _ => false,
            };
            if triggered {
                let fill_price = self.apply_slippage(tp, FillDirection::Exit(position.side));
                self.is_new_session = false;
                return Some(FillEvent {
                    fill_price,
                    exit_reason: ExitReason::TakeProfit,
                    is_gap_fill: false,
                });
            }
        }

        self.is_new_session = false;
        None
    }

    /// Apply the configured slippage model to a base fill price.
    pub fn apply_slippage(&self, base_price: Price, direction: FillDirection) -> Price {
        match &self.slippage {
            SlippageModel::None => base_price,
            SlippageModel::FixedTick(n) => {
                // Adverse direction: buys fill higher, sells fill lower
                let steps = match direction {
                    FillDirection::Enter(Side::Buy) | FillDirection::Exit(Side::Sell) => *n,
                    FillDirection::Enter(Side::Sell) | FillDirection::Exit(Side::Buy) => -n,
                    _ => 0,
                };
                base_price.add_steps(steps, self.tick_size)
            }
            SlippageModel::Percentage(pct) => {
                let factor = match direction {
                    FillDirection::Enter(Side::Buy) | FillDirection::Exit(Side::Sell) => {
                        1.0 + pct
                    }
                    FillDirection::Enter(Side::Sell) | FillDirection::Exit(Side::Buy) => {
                        1.0 - pct
                    }
                    _ => 1.0,
                };
                Price::from_f64(base_price.to_f64() * factor)
            }
        }
    }

    /// Compute the entry fill price for a new position.
    pub fn entry_fill_price(&self, trade_price: Price, side: Side) -> Price {
        self.apply_slippage(trade_price, FillDirection::Enter(side))
    }
}
