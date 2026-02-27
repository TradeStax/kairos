//! Futures domain types — venue, ticker, contract types, and timeframes.
//!
//! [`FuturesTicker`] is a stack-allocated 28-byte symbol that avoids heap
//! allocation for the hot path of market data processing.

use std::fmt;

use chrono::Datelike;
use serde::{Deserialize, Serialize};

use crate::domain::core::types::Price;

// ── Venue ─────────────────────────────────────────────────────────────

/// Futures exchange venue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FuturesVenue {
    /// CME Group Globex electronic platform
    CMEGlobex,
}

impl fmt::Display for FuturesVenue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FuturesVenue::CMEGlobex => write!(f, "CME Globex"),
        }
    }
}

impl FuturesVenue {
    /// All supported venues.
    pub const ALL: [FuturesVenue; 1] = [FuturesVenue::CMEGlobex];

    /// Return the Databento dataset identifier for this venue
    #[must_use]
    pub fn dataset(&self) -> &'static str {
        match self {
            FuturesVenue::CMEGlobex => "GLBX.MDP3",
        }
    }

    /// Return the stable key used for serialization
    #[must_use]
    pub fn serialization_key(&self) -> &'static str {
        match self {
            FuturesVenue::CMEGlobex => "CMEGlobex",
        }
    }

    /// Return the IANA timezone name for this venue's trading hours
    #[must_use]
    pub fn trading_timezone_name(&self) -> &'static str {
        match self {
            FuturesVenue::CMEGlobex => "US/Eastern",
        }
    }
}

// ── Contract Types ────────────────────────────────────────────────────

/// Whether a contract is a continuous front-month roll or a specific expiry.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ContractType {
    /// Continuous front-month with offset (0 = front, 1 = next, etc.)
    Continuous(u8),
    /// Specific contract symbol (e.g. `"ESH26"`)
    Specific(String),
}

impl ContractType {
    /// Parse a symbol string into a contract type.
    ///
    /// Recognizes `"ES.c.0"` as `Continuous(0)`, everything else as `Specific`.
    #[must_use]
    pub fn parse(symbol: &str) -> Option<Self> {
        let parts: Vec<&str> = symbol.split('.').collect();
        if parts.len() == 3
            && parts[1] == "c"
            && let Ok(offset) = parts[2].parse::<u8>()
        {
            return Some(ContractType::Continuous(offset));
        }
        Some(ContractType::Specific(symbol.to_string()))
    }

    /// Reconstruct the full symbol from product root and contract type
    #[must_use]
    pub fn to_symbol(&self, product: &str) -> String {
        match self {
            ContractType::Continuous(offset) => {
                format!("{}.c.{}", product, offset)
            }
            ContractType::Specific(contract) => contract.clone(),
        }
    }

    /// Return `true` for continuous-roll contracts
    #[must_use]
    pub fn is_continuous(&self) -> bool {
        matches!(self, ContractType::Continuous(_))
    }
}

impl fmt::Display for ContractType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ContractType::Continuous(offset) => {
                write!(f, "Continuous +{}", offset)
            }
            ContractType::Specific(contract) => write!(f, "{}", contract),
        }
    }
}

/// Instrument specification category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ContractSpec {
    /// Standard futures outright
    FuturesOutright,
    /// Futures calendar or inter-commodity spread
    FuturesSpread,
    /// Options on futures
    Options,
}

impl fmt::Display for ContractSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ContractSpec::FuturesOutright => write!(f, "Futures"),
            ContractSpec::FuturesSpread => write!(f, "Spread"),
            ContractSpec::Options => write!(f, "Options"),
        }
    }
}

// ── Futures Ticker ────────────────────────────────────────────────────

/// Stack-allocated futures ticker symbol (28 bytes max).
///
/// Avoids heap allocation on the hot path. The symbol, product root,
/// and optional display name are stored inline as fixed-size byte arrays.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct FuturesTicker {
    /// Full symbol bytes (e.g. `"ES.c.0"` or `"ESH26"`)
    bytes: [u8; 28],
    /// Exchange venue
    pub venue: FuturesVenue,
    /// Product root bytes (e.g. `"ES"`, `"NQ"`)
    product_bytes: [u8; 8],
    /// Optional human-readable display name bytes
    display_bytes: [u8; 28],
    /// Whether `display_bytes` contains a valid display name
    has_display_name: bool,
}

impl FuturesTicker {
    /// Create a ticker from a symbol string and venue
    #[must_use]
    pub fn new(symbol: &str, venue: FuturesVenue) -> Self {
        Self::new_with_display(symbol, venue, None, None)
    }

    /// Parse the expiration date from the symbol suffix.
    ///
    /// Returns the third Friday of the expiration month for specific
    /// contracts like `"ESH26"`. Returns `None` for continuous symbols.
    #[must_use]
    pub fn expiration_date(&self) -> Option<chrono::NaiveDate> {
        let symbol = self.as_str();
        if symbol.contains(".c.") {
            return None;
        }

        let product = self.product();
        if symbol.len() > product.len() {
            let suffix = &symbol[product.len()..];
            if suffix.len() >= 3 {
                let month_code = suffix.chars().next()?;
                let year_str = &suffix[1..];

                let month = match month_code {
                    'F' => 1,
                    'G' => 2,
                    'H' => 3,
                    'J' => 4,
                    'K' => 5,
                    'M' => 6,
                    'N' => 7,
                    'Q' => 8,
                    'U' => 9,
                    'V' => 10,
                    'X' => 11,
                    'Z' => 12,
                    _ => return None,
                };

                // 2-digit year: assume 2000-2049 for < 50, 1950-1999 otherwise.
                // This will need updating before 2050.
                let year = if year_str.len() == 2 {
                    let y = year_str.parse::<i32>().ok()?;
                    if y < 50 { 2000 + y } else { 1900 + y }
                } else if year_str.len() == 4 {
                    year_str.parse::<i32>().ok()?
                } else {
                    return None;
                };

                // Third Friday: find the first Friday, then add 14 days.
                let first_of_month = chrono::NaiveDate::from_ymd_opt(year, month as u32, 1)?;
                let first_weekday = first_of_month.weekday().num_days_from_monday();
                let days_to_first_friday = (4 + 7 - first_weekday) % 7;
                let third_friday =
                    first_of_month + chrono::Duration::days(days_to_first_friday as i64 + 14);
                Some(third_friday)
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Return `true` if the contract has expired (before today UTC)
    #[must_use]
    pub fn is_expired(&self) -> bool {
        if let Some(expiry) = self.expiration_date() {
            expiry < chrono::Utc::now().date_naive()
        } else {
            false
        }
    }

    /// Return the number of calendar days until expiration, or `None` for
    /// continuous contracts
    #[must_use]
    pub fn days_until_expiry(&self) -> Option<i64> {
        self.expiration_date().map(|expiry| {
            let today = chrono::Utc::now().date_naive();
            (expiry - today).num_days()
        })
    }

    /// Create a ticker with optional explicit product root and display name
    #[must_use]
    pub fn new_with_display(
        symbol: &str,
        venue: FuturesVenue,
        product: Option<&str>,
        display_name: Option<&str>,
    ) -> Self {
        let mut bytes = [0u8; 28];
        let len = symbol.len().min(28);
        bytes[..len].copy_from_slice(&symbol.as_bytes()[..len]);

        let mut product_bytes = [0u8; 8];
        if let Some(prod) = product {
            if prod.len() > 8 {
                log::warn!("Product string '{}' truncated to 8 characters", prod);
            }
            let prod_len = prod.len().min(8);
            product_bytes[..prod_len].copy_from_slice(&prod.as_bytes()[..prod_len]);
        } else {
            let extracted = Self::extract_product(symbol);
            if extracted.len() > 8 {
                log::warn!("Product string '{}' truncated to 8 characters", extracted);
            }
            let prod_len = extracted.len().min(8);
            product_bytes[..prod_len].copy_from_slice(&extracted.as_bytes()[..prod_len]);
        }

        let mut display_bytes = [0u8; 28];
        let has_display_name = if let Some(display) = display_name {
            let disp_len = display.len().min(28);
            display_bytes[..disp_len].copy_from_slice(&display.as_bytes()[..disp_len]);
            true
        } else {
            false
        };

        Self {
            bytes,
            venue,
            product_bytes,
            display_bytes,
            has_display_name,
        }
    }

    /// Extract the product root from a symbol string (e.g. `"ES"` from `"ESH26"`).
    fn extract_product(symbol: &str) -> String {
        if let Some(dot_pos) = symbol.find('.') {
            return symbol[..dot_pos].to_string();
        }
        let month_codes = ['F', 'G', 'H', 'J', 'K', 'M', 'N', 'Q', 'U', 'V', 'X', 'Z'];
        if let Some(pos) = symbol.chars().position(|c| month_codes.contains(&c)) {
            return symbol[..pos].to_string();
        }
        symbol.chars().take_while(|c| c.is_alphabetic()).collect()
    }

    /// Return the symbol as a string slice
    #[must_use]
    pub fn as_str(&self) -> &str {
        let end = self.bytes.iter().position(|&b| b == 0).unwrap_or(28);
        match std::str::from_utf8(&self.bytes[..end]) {
            Ok(s) => s,
            Err(_) => {
                log::warn!("FuturesTicker contains invalid UTF-8");
                "?"
            }
        }
    }

    /// Return the product root (e.g. `"ES"`, `"NQ"`)
    #[must_use]
    pub fn product(&self) -> &str {
        let end = self.product_bytes.iter().position(|&b| b == 0).unwrap_or(8);
        match std::str::from_utf8(&self.product_bytes[..end]) {
            Ok(s) => s,
            Err(_) => {
                log::warn!("FuturesTicker product contains invalid UTF-8");
                "?"
            }
        }
    }

    /// Return the display name, if one was set
    #[must_use]
    pub fn display_name(&self) -> Option<&str> {
        if self.has_display_name {
            let end = self
                .display_bytes
                .iter()
                .position(|&b| b == 0)
                .unwrap_or(28);
            Some(std::str::from_utf8(&self.display_bytes[..end]).unwrap_or("?"))
        } else {
            None
        }
    }

    /// Determine the contract type from the symbol
    #[must_use]
    pub fn contract_type(&self) -> ContractType {
        ContractType::parse(self.as_str())
            .unwrap_or_else(|| ContractType::Specific(self.as_str().to_string()))
    }

    /// Format as `"SYMBOL (VENUE)"`
    #[must_use]
    pub fn symbol_and_exchange_string(&self) -> String {
        format!("{} ({})", self.as_str(), self.venue)
    }

    /// Return `(display_symbol, contract_type_description)` for UI display
    #[must_use]
    pub fn display_symbol_and_type(&self) -> (String, String) {
        let symbol = self.display_name().unwrap_or(self.as_str()).to_string();
        let contract_type = match self.contract_type() {
            ContractType::Continuous(offset) => {
                format!("Continuous ({})", offset)
            }
            ContractType::Specific(_) => {
                if let Some(expiry) = self.expiration_date() {
                    format!("Expires {}", expiry.format("%Y-%m-%d"))
                } else {
                    "Specific".to_string()
                }
            }
        };
        (symbol, contract_type)
    }

    /// Return the market type string (always `"futures"`)
    #[must_use]
    pub fn market_type(&self) -> &'static str {
        "futures"
    }
}

impl fmt::Display for FuturesTicker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl fmt::Debug for FuturesTicker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.venue, self.as_str())
    }
}

impl Serialize for FuturesTicker {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let venue_str = self.venue.serialization_key();
        let s = if self.has_display_name {
            format!(
                "{}:{}|{}",
                venue_str,
                self.as_str(),
                self.display_name().unwrap_or("?")
            )
        } else {
            format!("{}:{}", venue_str, self.as_str())
        };
        serializer.serialize_str(&s)
    }
}

impl<'de> Deserialize<'de> for FuturesTicker {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let (venue_str, rest) = s
            .split_once(':')
            .ok_or_else(|| serde::de::Error::custom("expected \"Venue:Symbol\""))?;

        let venue = match venue_str {
            "CMEGlobex" => FuturesVenue::CMEGlobex,
            _ => {
                return Err(serde::de::Error::custom(format!(
                    "unknown venue: {}",
                    venue_str
                )));
            }
        };

        let (symbol, display) = if let Some((sym, disp)) = rest.split_once('|') {
            (sym, Some(disp))
        } else {
            (rest, None)
        };

        Ok(FuturesTicker::new_with_display(
            symbol, venue, None, display,
        ))
    }
}

// ── Ticker Info ───────────────────────────────────────────────────────

/// Futures ticker with instrument specifications (tick size, contract size, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FuturesTickerInfo {
    /// The underlying ticker symbol
    pub ticker: FuturesTicker,
    /// Minimum price increment
    pub tick_size: f32,
    /// Minimum order quantity
    pub min_qty: f32,
    /// Contract multiplier (e.g. 50 for ES)
    pub contract_size: f32,
}

impl FuturesTickerInfo {
    /// Create a new ticker info
    #[must_use]
    pub fn new(ticker: FuturesTicker, tick_size: f32, min_qty: f32, contract_size: f32) -> Self {
        Self {
            ticker,
            tick_size,
            min_qty,
            contract_size,
        }
    }

    /// Return the venue of the underlying ticker
    #[must_use]
    pub fn venue(&self) -> FuturesVenue {
        self.ticker.venue
    }

    /// Return the product root of the underlying ticker
    #[must_use]
    pub fn product(&self) -> &str {
        self.ticker.product()
    }

    /// Return the contract type of the underlying ticker
    #[must_use]
    pub fn contract_type(&self) -> ContractType {
        self.ticker.contract_type()
    }

    /// Return the tick size as a [`Price`]
    #[must_use]
    pub fn min_ticksize(&self) -> Price {
        Price::from_f32(self.tick_size)
    }

    /// Return the tick size as a [`PriceStep`](crate::domain::core::price::PriceStep)
    #[must_use]
    pub fn tick_step(&self) -> crate::domain::core::price::PriceStep {
        crate::domain::core::price::PriceStep::from_f32(self.tick_size)
    }
}

impl std::hash::Hash for FuturesTickerInfo {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.ticker.hash(state);
    }
}

impl Eq for FuturesTickerInfo {}

// ── Timeframe ─────────────────────────────────────────────────────────

/// Bar aggregation timeframe from 1 second to 1 day.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Timeframe {
    /// 1 second
    #[serde(rename = "1s")]
    M1s,
    /// 5 seconds
    #[serde(rename = "5s")]
    M5s,
    /// 10 seconds
    #[serde(rename = "10s")]
    M10s,
    /// 30 seconds
    #[serde(rename = "30s")]
    M30s,
    /// 1 minute
    #[serde(rename = "1m")]
    M1,
    /// 3 minutes
    #[serde(rename = "3m")]
    M3,
    /// 5 minutes
    #[serde(rename = "5m")]
    M5,
    /// 15 minutes
    #[serde(rename = "15m")]
    M15,
    /// 30 minutes
    #[serde(rename = "30m")]
    M30,
    /// 1 hour
    #[serde(rename = "1h")]
    H1,
    /// 4 hours
    #[serde(rename = "4h")]
    H4,
    /// 1 day
    #[serde(rename = "1d")]
    D1,
}

impl Timeframe {
    /// Timeframes available for kline (candlestick) charts.
    pub const KLINE: [Timeframe; 8] = [
        Timeframe::M1,
        Timeframe::M3,
        Timeframe::M5,
        Timeframe::M15,
        Timeframe::M30,
        Timeframe::H1,
        Timeframe::H4,
        Timeframe::D1,
    ];

    /// Timeframes available for heatmap charts.
    pub const HEATMAP: [Timeframe; 6] = [
        Timeframe::M1s,
        Timeframe::M5s,
        Timeframe::M10s,
        Timeframe::M30s,
        Timeframe::M1,
        Timeframe::M5,
    ];

    /// Return the duration in milliseconds
    #[must_use]
    pub fn to_milliseconds(self) -> u64 {
        match self {
            Timeframe::M1s => 1_000,
            Timeframe::M5s => 5_000,
            Timeframe::M10s => 10_000,
            Timeframe::M30s => 30_000,
            Timeframe::M1 => 60_000,
            Timeframe::M3 => 180_000,
            Timeframe::M5 => 300_000,
            Timeframe::M15 => 900_000,
            Timeframe::M30 => 1_800_000,
            Timeframe::H1 => 3_600_000,
            Timeframe::H4 => 14_400_000,
            Timeframe::D1 => 86_400_000,
        }
    }

    /// Return the duration in seconds
    #[must_use]
    pub fn to_seconds(self) -> u64 {
        self.to_milliseconds() / 1000
    }
}

impl fmt::Display for Timeframe {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Timeframe::M1s => write!(f, "1s"),
            Timeframe::M5s => write!(f, "5s"),
            Timeframe::M10s => write!(f, "10s"),
            Timeframe::M30s => write!(f, "30s"),
            Timeframe::M1 => write!(f, "1m"),
            Timeframe::M3 => write!(f, "3m"),
            Timeframe::M5 => write!(f, "5m"),
            Timeframe::M15 => write!(f, "15m"),
            Timeframe::M30 => write!(f, "30m"),
            Timeframe::H1 => write!(f, "1h"),
            Timeframe::H4 => write!(f, "4h"),
            Timeframe::D1 => write!(f, "1d"),
        }
    }
}

// ── Ticker Stats ──────────────────────────────────────────────────────

/// Real-time ticker statistics for the instrument list.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct TickerStats {
    /// Current mark / last traded price
    pub mark_price: f32,
    /// Daily price change (absolute)
    pub daily_price_chg: f32,
    /// Daily traded volume
    pub daily_volume: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expiration_date_parsing() {
        let ticker = FuturesTicker::new("ESH26", FuturesVenue::CMEGlobex);
        let expiry = ticker.expiration_date();
        assert!(expiry.is_some());
        if let Some(date) = expiry {
            assert_eq!(date.month(), 3);
            assert_eq!(date.year(), 2026);
        }

        let cont_ticker = FuturesTicker::new("ES.c.0", FuturesVenue::CMEGlobex);
        assert!(cont_ticker.expiration_date().is_none());
        assert!(!cont_ticker.is_expired());
    }

    #[test]
    fn test_month_code_mapping() {
        let test_cases = vec![
            ("ESF26", 1),
            ("ESG26", 2),
            ("ESH26", 3),
            ("ESJ26", 4),
            ("ESK26", 5),
            ("ESM26", 6),
            ("ESN26", 7),
            ("ESQ26", 8),
            ("ESU26", 9),
            ("ESV26", 10),
            ("ESX26", 11),
            ("ESZ26", 12),
        ];

        for (symbol, expected_month) in test_cases {
            let ticker = FuturesTicker::new(symbol, FuturesVenue::CMEGlobex);
            let date = ticker.expiration_date();
            assert!(date.is_some(), "Failed to parse expiration for {}", symbol);
            assert_eq!(date.unwrap().month(), expected_month);
        }
    }

    #[test]
    fn test_days_until_expiry() {
        let ticker = FuturesTicker::new("ESZ26", FuturesVenue::CMEGlobex);
        let days = ticker
            .days_until_expiry()
            .expect("ESZ26 should return Some for days_until_expiry");
        assert!(days > 0);
        assert!(days < 365);

        let cont_ticker = FuturesTicker::new("ES.c.0", FuturesVenue::CMEGlobex);
        assert!(cont_ticker.days_until_expiry().is_none());
    }
}
