use kairos_data::Price;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};

/// Unique order identifier within a backtest run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OrderId(pub u64);

static NEXT_ORDER_ID: AtomicU64 = AtomicU64::new(1);

impl OrderId {
    pub fn next() -> Self {
        Self(NEXT_ORDER_ID.fetch_add(1, Ordering::Relaxed))
    }

    pub fn reset() {
        NEXT_ORDER_ID.store(1, Ordering::Relaxed);
    }
}

/// Side of an order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OrderSide {
    Buy,
    Sell,
}

impl OrderSide {
    pub fn opposite(self) -> Self {
        match self {
            Self::Buy => Self::Sell,
            Self::Sell => Self::Buy,
        }
    }

    /// Convert from kairos_data::Side.
    pub fn from_data_side(side: kairos_data::Side) -> Self {
        match side {
            kairos_data::Side::Buy | kairos_data::Side::Bid => Self::Buy,
            kairos_data::Side::Sell | kairos_data::Side::Ask => Self::Sell,
        }
    }

    /// Convert to kairos_data::Side.
    pub fn to_data_side(self) -> kairos_data::Side {
        match self {
            Self::Buy => kairos_data::Side::Buy,
            Self::Sell => kairos_data::Side::Sell,
        }
    }
}

/// Type of order.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum OrderType {
    Market,
    Limit { price: Price },
    Stop { trigger: Price },
    StopLimit { trigger: Price, limit: Price },
}

/// Time-in-force for an order.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimeInForce {
    /// Good till cancelled (or end of backtest).
    #[default]
    GTC,
    /// Day order -- cancelled at session close.
    Day,
    /// Immediate or cancel -- fill what you can, cancel rest.
    IOC,
}

/// Lifecycle status of an order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderStatus {
    /// Submitted but not yet active (e.g. pending latency delay).
    Pending,
    /// Active in the order book.
    Active,
    /// Some quantity has been filled.
    PartiallyFilled,
    /// Fully filled.
    Filled,
    /// Cancelled by strategy or engine.
    Cancelled,
    /// Rejected (insufficient margin, invalid params, etc.).
    Rejected,
    /// Expired (e.g. Day order at session close).
    Expired,
}

impl OrderStatus {
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Filled | Self::Cancelled | Self::Rejected | Self::Expired
        )
    }
}
