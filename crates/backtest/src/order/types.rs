//! Primitive value types used throughout the order module.
//!
//! These are small, `Copy` types that represent the fundamental
//! attributes of an order: identity, side, type, time-in-force, and
//! lifecycle status.

use kairos_data::Price;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

/// Unique order identifier within a single backtest run.
///
/// IDs are assigned sequentially via a global atomic counter starting
/// at 1. Call [`OrderId::reset`] between runs to restart numbering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OrderId(pub u64);

/// Global monotonic counter for order IDs.
static NEXT_ORDER_ID: AtomicU64 = AtomicU64::new(1);

impl OrderId {
    /// Allocate the next sequential order ID.
    #[must_use]
    pub fn next() -> Self {
        Self(NEXT_ORDER_ID.fetch_add(1, Ordering::Relaxed))
    }

    /// Reset the ID counter to 1. Call between backtest runs.
    pub fn reset() {
        NEXT_ORDER_ID.store(1, Ordering::Relaxed);
    }
}

impl fmt::Display for OrderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{}", self.0)
    }
}

/// Side of an order (buy or sell).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OrderSide {
    /// A buy (long entry / short exit).
    Buy,
    /// A sell (short entry / long exit).
    Sell,
}

impl OrderSide {
    /// Return the opposite side.
    #[must_use]
    pub fn opposite(self) -> Self {
        match self {
            Self::Buy => Self::Sell,
            Self::Sell => Self::Buy,
        }
    }

    /// Convert from [`kairos_data::Side`].
    #[must_use]
    pub fn from_data_side(side: kairos_data::Side) -> Self {
        match side {
            kairos_data::Side::Buy | kairos_data::Side::Bid => Self::Buy,
            kairos_data::Side::Sell | kairos_data::Side::Ask => Self::Sell,
        }
    }

    /// Convert to [`kairos_data::Side`].
    #[must_use]
    pub fn to_data_side(self) -> kairos_data::Side {
        match self {
            Self::Buy => kairos_data::Side::Buy,
            Self::Sell => kairos_data::Side::Sell,
        }
    }
}

impl fmt::Display for OrderSide {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Buy => f.write_str("Buy"),
            Self::Sell => f.write_str("Sell"),
        }
    }
}

/// Type of order, determining how and when it executes.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum OrderType {
    /// Execute immediately at the current market price.
    Market,
    /// Execute at the specified price or better.
    Limit {
        /// The limit price.
        price: Price,
    },
    /// Becomes a market order when the trigger price is reached.
    Stop {
        /// The stop trigger price.
        trigger: Price,
    },
    /// Becomes a limit order when the trigger price is reached.
    StopLimit {
        /// The stop trigger price.
        trigger: Price,
        /// The limit price used once triggered.
        limit: Price,
    },
}

/// Time-in-force policy controlling order lifetime.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TimeInForce {
    /// Good till cancelled (or end of backtest).
    #[default]
    GTC,
    /// Day order -- cancelled at session close.
    Day,
    /// Immediate or cancel -- fill what you can, cancel the rest.
    IOC,
}

impl fmt::Display for TimeInForce {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GTC => f.write_str("GTC"),
            Self::Day => f.write_str("Day"),
            Self::IOC => f.write_str("IOC"),
        }
    }
}

/// Lifecycle status of an order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OrderStatus {
    /// Submitted but not yet active (e.g. bracket child awaiting
    /// parent fill, or simulated latency delay).
    Pending,
    /// Active in the order book and eligible for matching.
    Active,
    /// Some quantity has been filled; the remainder is still working.
    PartiallyFilled,
    /// Fully filled -- all requested quantity has been executed.
    Filled,
    /// Cancelled by strategy request or engine (e.g. OCO partner).
    Cancelled,
    /// Rejected by the engine (insufficient margin, invalid params).
    Rejected,
    /// Expired (e.g. a `Day` order at session close).
    Expired,
}

impl OrderStatus {
    /// Returns `true` if this status represents a final state from
    /// which no further transitions are possible.
    #[must_use]
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Filled | Self::Cancelled | Self::Rejected | Self::Expired
        )
    }
}

impl fmt::Display for OrderStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pending => f.write_str("Pending"),
            Self::Active => f.write_str("Active"),
            Self::PartiallyFilled => f.write_str("PartiallyFilled"),
            Self::Filled => f.write_str("Filled"),
            Self::Cancelled => f.write_str("Cancelled"),
            Self::Rejected => f.write_str("Rejected"),
            Self::Expired => f.write_str("Expired"),
        }
    }
}
