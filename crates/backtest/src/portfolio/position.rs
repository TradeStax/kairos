use crate::order::types::OrderSide;
use kairos_data::{FuturesTicker, Price, Timestamp};

/// A single position entry (one fill event).
#[derive(Debug, Clone)]
pub struct PositionEntry {
    pub price: Price,
    pub quantity: f64,
    pub timestamp: Timestamp,
}

/// An open position for a single instrument.
/// Supports multiple entries (scaling in), partial closes.
#[derive(Debug, Clone)]
pub struct Position {
    pub instrument: FuturesTicker,
    pub side: OrderSide,
    pub entries: Vec<PositionEntry>,
    pub quantity: f64,
    pub avg_entry_price: Price,
    pub mark_price: Price,
    pub mae_price: Price,
    pub mfe_price: Price,
    pub opened_at: Timestamp,
    pub label: Option<String>,
    /// Stop loss set by the strategy (used for R:R calculation).
    pub initial_stop_loss: Option<Price>,
}

impl Position {
    /// Create a new position from a first fill.
    pub fn new(
        instrument: FuturesTicker,
        side: OrderSide,
        price: Price,
        quantity: f64,
        timestamp: Timestamp,
        label: Option<String>,
    ) -> Self {
        Self {
            instrument,
            side,
            entries: vec![PositionEntry {
                price,
                quantity,
                timestamp,
            }],
            quantity,
            avg_entry_price: price,
            mark_price: price,
            mae_price: price,
            mfe_price: price,
            opened_at: timestamp,
            label,
            initial_stop_loss: None,
        }
    }

    /// Apply a fill to this position.
    /// Returns: how much was consumed by this position (may be less
    /// than fill_qty if it closes or reverses).
    ///
    /// - Same-side fill: adds to position (scale in).
    /// - Opposite-side fill: reduces position. If fill_qty >
    ///   position qty, only position qty is consumed (caller handles
    ///   reversal).
    ///
    /// Returns (consumed_qty, closed: bool).
    pub fn apply_fill(
        &mut self,
        fill_side: OrderSide,
        fill_price: Price,
        fill_qty: f64,
        timestamp: Timestamp,
    ) -> (f64, bool) {
        if fill_side == self.side {
            // Scale in: add to position
            let prev_value = self.avg_entry_price.to_f64() * self.quantity;
            let fill_value = fill_price.to_f64() * fill_qty;
            self.quantity += fill_qty;
            self.avg_entry_price = Price::from_f64((prev_value + fill_value) / self.quantity);
            self.entries.push(PositionEntry {
                price: fill_price,
                quantity: fill_qty,
                timestamp,
            });
            (fill_qty, false)
        } else {
            // Reduce position
            let consumed = fill_qty.min(self.quantity);
            self.quantity -= consumed;
            let closed = self.quantity < 1e-9;
            if closed {
                self.quantity = 0.0;
            }
            (consumed, closed)
        }
    }

    /// Update MAE/MFE with current market price.
    pub fn update_mark(&mut self, current_price: Price) {
        self.mark_price = current_price;
        match self.side {
            OrderSide::Buy => {
                if current_price < self.mae_price {
                    self.mae_price = current_price;
                }
                if current_price > self.mfe_price {
                    self.mfe_price = current_price;
                }
            }
            OrderSide::Sell => {
                if current_price > self.mae_price {
                    self.mae_price = current_price;
                }
                if current_price < self.mfe_price {
                    self.mfe_price = current_price;
                }
            }
        }
    }

    /// Compute unrealized PnL in USD.
    pub fn unrealized_pnl(&self, tick_size: Price, tick_value: f64) -> f64 {
        if tick_size.units() == 0 {
            return 0.0;
        }
        let diff = match self.side {
            OrderSide::Buy => self.mark_price.units() - self.avg_entry_price.units(),
            OrderSide::Sell => self.avg_entry_price.units() - self.mark_price.units(),
        };
        let ticks = diff as f64 / tick_size.units() as f64;
        ticks * tick_value * self.quantity
    }

    /// MAE in ticks (always non-negative).
    pub fn mae_ticks(&self, tick_size: Price) -> i64 {
        if tick_size.units() == 0 {
            return 0;
        }
        let diff = match self.side {
            OrderSide::Buy => self.avg_entry_price.units() - self.mae_price.units(),
            OrderSide::Sell => self.mae_price.units() - self.avg_entry_price.units(),
        };
        (diff / tick_size.units()).max(0)
    }

    /// MFE in ticks (always non-negative).
    pub fn mfe_ticks(&self, tick_size: Price) -> i64 {
        if tick_size.units() == 0 {
            return 0;
        }
        let diff = match self.side {
            OrderSide::Buy => self.mfe_price.units() - self.avg_entry_price.units(),
            OrderSide::Sell => self.avg_entry_price.units() - self.mfe_price.units(),
        };
        (diff / tick_size.units()).max(0)
    }

    /// Set the initial stop loss price (used for R:R calculation).
    pub fn set_stop_loss(&mut self, price: Price) {
        self.initial_stop_loss = Some(price);
    }
}
