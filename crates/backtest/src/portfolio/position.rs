//! Individual position tracking with scale-in and MAE/MFE support.
//!
//! A [`Position`] represents an open directional exposure in a single
//! instrument. It supports:
//!
//! - **Scaling in** -- multiple fills on the same side compute a
//!   volume-weighted average entry price.
//! - **Partial closes** -- opposite-side fills reduce quantity
//!   without exceeding the position size.
//! - **MAE/MFE tracking** -- records the worst and best
//!   mark-to-market prices seen while the position was open.

use crate::order::types::OrderSide;
use kairos_data::{FuturesTicker, Price, Timestamp};

/// A single fill that contributed to a position.
#[derive(Debug, Clone)]
pub struct PositionEntry {
    /// Fill price.
    pub price: Price,
    /// Number of contracts filled.
    pub quantity: f64,
    /// Timestamp of the fill event.
    pub timestamp: Timestamp,
}

/// An open position for a single instrument.
///
/// Supports multiple entries (scaling in) and partial closes.
/// Tracks maximum adverse and favorable excursion for post-trade
/// analytics.
#[derive(Debug, Clone)]
pub struct Position {
    /// Instrument this position is in.
    pub instrument: FuturesTicker,
    /// Direction of the position (long = Buy, short = Sell).
    pub side: OrderSide,
    /// Individual fills that built up this position.
    pub entries: Vec<PositionEntry>,
    /// Current open quantity (contracts).
    pub quantity: f64,
    /// Volume-weighted average entry price across all fills.
    pub avg_entry_price: Price,
    /// Last mark-to-market price.
    pub mark_price: Price,
    /// Maximum Adverse Excursion price -- worst mark seen.
    ///
    /// For longs this is the lowest price; for shorts the highest.
    pub mae_price: Price,
    /// Maximum Favorable Excursion price -- best mark seen.
    ///
    /// For longs this is the highest price; for shorts the lowest.
    pub mfe_price: Price,
    /// Timestamp when the position was first opened.
    pub opened_at: Timestamp,
    /// Optional strategy label for grouping trades in analytics.
    pub label: Option<String>,
    /// Stop-loss price set by the strategy at entry time.
    ///
    /// Used to compute the risk-reward ratio (R:R) on the resulting
    /// [`TradeRecord`](crate::output::trade_record::TradeRecord).
    pub initial_stop_loss: Option<Price>,
}

impl Position {
    /// Create a new position from an initial fill.
    #[must_use]
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
    ///
    /// # Same-side fills (scale in)
    ///
    /// Adds to the position and recomputes the volume-weighted
    /// average entry price:
    ///
    /// ```text
    /// new_avg = (old_avg * old_qty + fill_price * fill_qty)
    ///         / (old_qty + fill_qty)
    /// ```
    ///
    /// # Opposite-side fills (reduce / close)
    ///
    /// Reduces the position quantity. If `fill_qty >= position qty`
    /// the position is fully closed. Only the consumed portion
    /// (up to position size) is applied -- the caller handles any
    /// reversal.
    ///
    /// Returns `(consumed_qty, closed)` where `consumed_qty` is
    /// the number of contracts actually applied and `closed` is
    /// true if the position is now flat.
    pub fn apply_fill(
        &mut self,
        fill_side: OrderSide,
        fill_price: Price,
        fill_qty: f64,
        timestamp: Timestamp,
    ) -> (f64, bool) {
        if fill_side == self.side {
            // Scale in: recompute VWAP entry price
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

    /// Update the mark price and refresh MAE/MFE tracking.
    ///
    /// Should be called on every price update (tick or bar close)
    /// while the position is open.
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

    /// Compute unrealized PnL in USD at the current mark price.
    ///
    /// # Formula
    ///
    /// ```text
    /// ticks = (mark - entry) / tick_size   [long]
    /// ticks = (entry - mark) / tick_size   [short]
    /// pnl   = ticks * tick_value * quantity
    /// ```
    #[must_use]
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

    /// Maximum Adverse Excursion in ticks (always non-negative).
    ///
    /// Measures how far the market moved *against* the position
    /// from entry before exiting or recovering.
    #[must_use]
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

    /// Maximum Favorable Excursion in ticks (always non-negative).
    ///
    /// Measures how far the market moved *in favor of* the position
    /// from entry before exiting or pulling back.
    #[must_use]
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

    /// Set the initial stop-loss price for R:R calculation.
    pub fn set_stop_loss(&mut self, price: Price) {
        self.initial_stop_loss = Some(price);
    }
}
