use kairos_data::{Price, Side, Timestamp};

/// An open (not yet closed) position held by the simulated broker.
#[derive(Debug, Clone)]
pub struct OpenPosition {
    pub side: Side,
    pub entry_price: Price,
    pub entry_time: Timestamp,
    pub quantity: f64,
    pub stop_loss: Option<Price>,
    pub take_profit: Option<Price>,
    /// Worst price seen since entry — used for MAE calculation.
    pub mae: Price,
    /// Best price seen since entry — used for MFE calculation.
    pub mfe: Price,
    pub label: Option<String>,
}

impl OpenPosition {
    pub fn new(
        side: Side,
        entry_price: Price,
        entry_time: Timestamp,
        quantity: f64,
        stop_loss: Option<Price>,
        take_profit: Option<Price>,
        label: Option<String>,
    ) -> Self {
        Self {
            side,
            entry_price,
            entry_time,
            quantity,
            stop_loss,
            take_profit,
            mae: entry_price,
            mfe: entry_price,
            label,
        }
    }

    /// Update MAE/MFE given the latest market price.
    pub fn update_extremes(&mut self, current_price: Price) {
        match self.side {
            Side::Buy => {
                if current_price < self.mae {
                    self.mae = current_price;
                }
                if current_price > self.mfe {
                    self.mfe = current_price;
                }
            }
            Side::Sell => {
                if current_price > self.mae {
                    self.mae = current_price;
                }
                if current_price < self.mfe {
                    self.mfe = current_price;
                }
            }
            _ => {}
        }
    }

    /// Compute current unrealized PnL in USD.
    pub fn unrealized_pnl(
        &self,
        current_price: Price,
        tick_size: Price,
        tick_value: f64,
    ) -> f64 {
        if tick_size.units() == 0 {
            return 0.0;
        }
        let price_diff = match self.side {
            Side::Buy => current_price.units() - self.entry_price.units(),
            Side::Sell => self.entry_price.units() - current_price.units(),
            _ => 0,
        };
        let ticks = price_diff / tick_size.units();
        ticks as f64 * tick_value * self.quantity
    }
}
