//! Exchange Types - Market Data Types Only
//!
//! Exchange-specific types for market data (trades, klines, depth, etc.).
//! Domain types (FuturesTicker, Timeframe, etc.) are now in data::domain::futures.

use crate::util::{Price, PriceStep};
use kairos_data::domain::FuturesTicker;
use serde::{Deserialize, Serialize};

// ── Download Schema ─────────────────────────────────────────────────

/// Databento download schema selection.
///
/// Wraps the Databento-specific schema variants used for historical data
/// downloads, providing a stable API boundary so callers don't depend on
/// the third-party `databento` crate directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DownloadSchema {
    Trades,
    Mbp10,
    Mbp1,
    Ohlcv1M,
    Tbbo,
    Mbo,
}

impl DownloadSchema {
    /// Convert to the u16 discriminant used by Databento's Schema enum.
    pub fn as_discriminant(self) -> u16 {
        self.to_databento_schema() as u16
    }

    /// Convert to the underlying `databento::dbn::Schema`.
    pub fn to_databento_schema(self) -> databento::dbn::Schema {
        match self {
            Self::Trades => databento::dbn::Schema::Trades,
            Self::Mbp10 => databento::dbn::Schema::Mbp10,
            Self::Mbp1 => databento::dbn::Schema::Mbp1,
            Self::Ohlcv1M => databento::dbn::Schema::Ohlcv1M,
            Self::Tbbo => databento::dbn::Schema::Tbbo,
            Self::Mbo => databento::dbn::Schema::Mbo,
        }
    }
}

impl std::fmt::Display for DownloadSchema {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Trades => write!(f, "Trades"),
            Self::Mbp10 => write!(f, "MBP-10"),
            Self::Mbp1 => write!(f, "MBP-1"),
            Self::Ohlcv1M => write!(f, "OHLCV-1M"),
            Self::Tbbo => write!(f, "TBBO"),
            Self::Mbo => write!(f, "MBO"),
        }
    }
}
use std::collections::BTreeMap;

// ── Market Data Types ─────────────────────────────────────────────────

/// Trade execution
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Trade {
    pub time: u64,
    pub price: f32,
    pub qty: f32,
    pub side: TradeSide,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TradeSide {
    Buy,
    Sell,
}

/// OHLCV Candle/Kline
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Kline {
    pub time: u64,
    pub open: f32,
    pub high: f32,
    pub low: f32,
    pub close: f32,
    pub volume: f32,
    pub buy_volume: f32,
    pub sell_volume: f32,
}

impl Kline {
    pub fn new(time: u64) -> Self {
        Self {
            time,
            open: 0.0,
            high: 0.0,
            low: 0.0,
            close: 0.0,
            volume: 0.0,
            buy_volume: 0.0,
            sell_volume: 0.0,
        }
    }
}

/// Open Interest
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct OpenInterest {
    pub time: u64,
    pub open_interest: f32,
}

// ── Orderbook / Depth Types ───────────────────────────────────────────

/// Market depth (orderbook) snapshot
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Depth {
    pub time: u64,
    pub bids: BTreeMap<i64, f32>, // price_units -> quantity
    pub asks: BTreeMap<i64, f32>,
}

impl Depth {
    pub fn new(time: u64) -> Self {
        Self {
            time,
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
        }
    }

    pub fn best_bid(&self) -> Option<(Price, f32)> {
        self.bids
            .iter()
            .next_back()
            .map(|(price_units, qty)| (Price::from_units(*price_units), *qty))
    }

    pub fn best_ask(&self) -> Option<(Price, f32)> {
        self.asks
            .iter()
            .next()
            .map(|(price_units, qty)| (Price::from_units(*price_units), *qty))
    }

    pub fn get_bid_qty(&self, price: Price) -> f32 {
        self.bids.get(&price.units()).copied().unwrap_or(0.0)
    }

    pub fn get_ask_qty(&self, price: Price) -> f32 {
        self.asks.get(&price.units()).copied().unwrap_or(0.0)
    }

    pub fn update_bid(&mut self, price: Price, qty: f32) {
        if qty > 0.0 {
            self.bids.insert(price.units(), qty);
        } else {
            self.bids.remove(&price.units());
        }
    }

    pub fn update_ask(&mut self, price: Price, qty: f32) {
        if qty > 0.0 {
            self.asks.insert(price.units(), qty);
        } else {
            self.asks.remove(&price.units());
        }
    }

    /// Get top N levels of bids (highest first)
    pub fn top_bids(&self, n: usize) -> Vec<(Price, f32)> {
        self.bids
            .iter()
            .rev()
            .take(n)
            .map(|(price_units, qty)| (Price::from_units(*price_units), *qty))
            .collect()
    }

    /// Get top N levels of asks (lowest first)
    pub fn top_asks(&self, n: usize) -> Vec<(Price, f32)> {
        self.asks
            .iter()
            .take(n)
            .map(|(price_units, qty)| (Price::from_units(*price_units), *qty))
            .collect()
    }
}

impl Default for Depth {
    fn default() -> Self {
        Self::new(0)
    }
}

// ── Ticker Info (for exchange adapter) ────────────────────────────────

/// Ticker info used by exchange adapter (uses exchange::util::Price types)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TickerInfo {
    pub ticker: FuturesTicker,
    pub min_ticksize: PriceStep,
    pub min_qty: f32,
    pub contract_size: f32,
}

impl std::hash::Hash for TickerInfo {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.ticker.hash(state);
        self.min_ticksize.hash(state);
        // Hash f32 as bytes for deterministic hashing
        self.min_qty.to_bits().hash(state);
        self.contract_size.to_bits().hash(state);
    }
}

impl From<crate::FuturesTickerInfo> for TickerInfo {
    fn from(info: crate::FuturesTickerInfo) -> Self {
        TickerInfo {
            ticker: info.ticker,
            min_ticksize: PriceStep::from_f32(info.tick_size),
            min_qty: info.min_qty,
            contract_size: info.contract_size,
        }
    }
}

impl From<TickerInfo> for crate::FuturesTickerInfo {
    fn from(info: TickerInfo) -> Self {
        crate::FuturesTickerInfo {
            ticker: info.ticker,
            tick_size: info.min_ticksize.to_f32_lossy(),
            min_qty: info.min_qty,
            contract_size: info.contract_size,
        }
    }
}

impl Eq for TickerInfo {}

impl TickerInfo {
    pub fn new(ticker: FuturesTicker, tick_size: f32, min_qty: f32, contract_size: f32) -> Self {
        Self {
            ticker,
            min_ticksize: PriceStep::from_f32(tick_size),
            min_qty,
            contract_size,
        }
    }

    /// Convert to domain FuturesTickerInfo
    pub fn to_domain(&self) -> kairos_data::domain::FuturesTickerInfo {
        kairos_data::domain::FuturesTickerInfo::new(
            self.ticker,
            self.min_ticksize.to_f32_lossy(),
            self.min_qty,
            self.contract_size,
        )
    }

    /// From domain FuturesTickerInfo
    pub fn from_domain(info: kairos_data::domain::FuturesTickerInfo) -> Self {
        Self {
            ticker: info.ticker,
            min_ticksize: PriceStep::from_f32(info.tick_size),
            min_qty: info.min_qty,
            contract_size: info.contract_size,
        }
    }

    /// Get market type (for compatibility)
    pub fn market_type(&self) -> &'static str {
        "futures"
    }

    /// Get exchange/venue
    pub fn exchange(&self) -> kairos_data::domain::FuturesVenue {
        self.ticker.venue
    }
}
