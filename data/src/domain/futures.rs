//! Futures Domain Types
//!
//! Core domain types for futures markets - venue, ticker, contract types.
//! These are pure domain concepts independent of any exchange adapter.

use super::types::Price;
use chrono::Datelike;
use serde::{Deserialize, Serialize};
use std::fmt;

// ── Venue ─────────────────────────────────────────────────────────────

/// Futures exchange venue
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FuturesVenue {
    /// CME Globex (ES, NQ, YM, RTY, ZN, etc.)
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
    pub const ALL: [FuturesVenue; 1] = [FuturesVenue::CMEGlobex];

    pub fn dataset(&self) -> &'static str {
        match self {
            FuturesVenue::CMEGlobex => "GLBX.MDP3",
        }
    }
}

// ── Contract Types ────────────────────────────────────────────────────

/// Type of futures contract
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ContractType {
    /// Continuous contract with offset (e.g., ES.c.0 for front month)
    Continuous(u8),
    /// Specific contract (e.g., ESH24)
    Specific(String),
}

impl ContractType {
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

    pub fn to_symbol(&self, product: &str) -> String {
        match self {
            ContractType::Continuous(offset) => format!("{}.c.{}", product, offset),
            ContractType::Specific(contract) => contract.clone(),
        }
    }

    pub fn is_continuous(&self) -> bool {
        matches!(self, ContractType::Continuous(_))
    }
}

impl fmt::Display for ContractType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ContractType::Continuous(offset) => write!(f, "Continuous +{}", offset),
            ContractType::Specific(contract) => write!(f, "{}", contract),
        }
    }
}

/// Contract specification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ContractSpec {
    FuturesOutright,
    FuturesSpread,
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

/// Futures ticker identifier
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct FuturesTicker {
    bytes: [u8; 28],
    pub venue: FuturesVenue,
    product_bytes: [u8; 8],
    display_bytes: [u8; 28],
    has_display_name: bool,
}

impl FuturesTicker {
    pub fn new(symbol: &str, venue: FuturesVenue) -> Self {
        Self::new_with_display(symbol, venue, None, None)
    }

    /// Extract expiration date from ticker symbol (if it's a specific contract)
    ///
    /// For specific contracts like "ESH24", returns the expiration date
    /// Returns None for continuous contracts or if parsing fails
    pub fn expiration_date(&self) -> Option<chrono::NaiveDate> {
        let symbol = self.as_str();

        // Check if it's a continuous contract (e.g., "ES.c.0")
        if symbol.contains(".c.") {
            return None;
        }

        // Try to extract month/year from symbols like "ESH24" or "ESZ2024"
        let product = self.product();
        if symbol.len() > product.len() {
            let suffix = &symbol[product.len()..];

            // Check if we have at least month code + year
            if suffix.len() >= 3 {
                let month_code = suffix.chars().next()?;
                let year_str = &suffix[1..];

                // Map futures month codes to month numbers
                let month = match month_code {
                    'F' => 1,  // January
                    'G' => 2,  // February
                    'H' => 3,  // March
                    'J' => 4,  // April
                    'K' => 5,  // May
                    'M' => 6,  // June
                    'N' => 7,  // July
                    'Q' => 8,  // August
                    'U' => 9,  // September
                    'V' => 10, // October
                    'X' => 11, // November
                    'Z' => 12, // December
                    _ => return None,
                };

                // Parse year (handles both 2-digit and 4-digit years)
                // NOTE: 2-digit years use a pivot at 50: 00-49 → 2000-2049,
                // 50-99 → 1950-1999. This will need updating before 2050.
                let year = if year_str.len() == 2 {
                    let y = year_str.parse::<i32>().ok()?;
                    if y < 50 { 2000 + y } else { 1900 + y }
                } else if year_str.len() == 4 {
                    year_str.parse::<i32>().ok()?
                } else {
                    return None;
                };

                // Get third Friday of the month (typical futures expiration)
                // Note: This is a simplification - actual expiration rules vary by product
                let first_day = chrono::NaiveDate::from_ymd_opt(year, month as u32, 1)?;
                let mut third_friday = first_day;
                let mut friday_count = 0;

                for day in 1..=31 {
                    if let Some(date) = chrono::NaiveDate::from_ymd_opt(year, month as u32, day) {
                        if date.weekday() == chrono::Weekday::Fri {
                            friday_count += 1;
                            if friday_count == 3 {
                                third_friday = date;
                                break;
                            }
                        }
                    } else {
                        break;
                    }
                }

                Some(third_friday)
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Check if contract is expired (based on current date)
    pub fn is_expired(&self) -> bool {
        if let Some(expiry) = self.expiration_date() {
            expiry < chrono::Local::now().naive_local().date()
        } else {
            false
        }
    }

    /// Days until expiration (negative if expired)
    pub fn days_until_expiry(&self) -> Option<i64> {
        self.expiration_date().map(|expiry| {
            let today = chrono::Local::now().naive_local().date();
            (expiry - today).num_days()
        })
    }

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

    fn extract_product(symbol: &str) -> String {
        if let Some(dot_pos) = symbol.find('.') {
            return symbol[..dot_pos].to_string();
        }

        // For specific contracts like "ESH24", extract just the product code (e.g., "ES")
        // Month codes are single letters: F, G, H, J, K, M, N, Q, U, V, X, Z
        // Find the first character that is a valid month code
        let month_codes = ['F', 'G', 'H', 'J', 'K', 'M', 'N', 'Q', 'U', 'V', 'X', 'Z'];
        if let Some(pos) = symbol.chars().position(|c| month_codes.contains(&c)) {
            return symbol[..pos].to_string();
        }

        // Fallback: take alphabetic characters
        symbol.chars().take_while(|c| c.is_alphabetic()).collect()
    }

    pub fn as_str(&self) -> &str {
        let end = self.bytes.iter().position(|&b| b == 0).unwrap_or(28);
        std::str::from_utf8(&self.bytes[..end]).unwrap()
    }

    pub fn product(&self) -> &str {
        let end = self.product_bytes.iter().position(|&b| b == 0).unwrap_or(8);
        std::str::from_utf8(&self.product_bytes[..end]).unwrap()
    }

    pub fn display_name(&self) -> Option<&str> {
        if self.has_display_name {
            let end = self
                .display_bytes
                .iter()
                .position(|&b| b == 0)
                .unwrap_or(28);
            Some(std::str::from_utf8(&self.display_bytes[..end]).unwrap())
        } else {
            None
        }
    }

    pub fn contract_type(&self) -> ContractType {
        ContractType::parse(self.as_str())
            .unwrap_or_else(|| ContractType::Specific(self.as_str().to_string()))
    }

    /// Get symbol with exchange/venue string (for UI display)
    pub fn symbol_and_exchange_string(&self) -> String {
        format!("{} ({})", self.as_str(), self.venue)
    }

    /// Get display symbol and contract type (for UI)
    pub fn display_symbol_and_type(&self) -> (String, String) {
        let symbol = self.display_name().unwrap_or(self.as_str()).to_string();
        let contract_type = match self.contract_type() {
            ContractType::Continuous(offset) => format!("Continuous ({})", offset),
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

    /// Get market type (for compatibility - futures don't have spot/linear/inverse)
    pub fn market_type(&self) -> &'static str {
        "futures"
    }

    /// Get exchange (alias for venue)
    pub fn exchange(&self) -> FuturesVenue {
        self.venue
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
        let venue_str = "CMEGlobex";
        let s = if self.has_display_name {
            format!(
                "{}:{}|{}",
                venue_str,
                self.as_str(),
                self.display_name().unwrap()
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

/// Futures ticker information with contract specifications
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FuturesTickerInfo {
    pub ticker: FuturesTicker,
    pub tick_size: f32,
    pub min_qty: f32,
    pub contract_size: f32,
}

impl FuturesTickerInfo {
    pub fn new(ticker: FuturesTicker, tick_size: f32, min_qty: f32, contract_size: f32) -> Self {
        Self {
            ticker,
            tick_size,
            min_qty,
            contract_size,
        }
    }

    pub fn venue(&self) -> FuturesVenue {
        self.ticker.venue
    }

    pub fn product(&self) -> &str {
        self.ticker.product()
    }

    pub fn contract_type(&self) -> ContractType {
        self.ticker.contract_type()
    }

    pub fn exchange(&self) -> FuturesVenue {
        self.venue()
    }

    /// Get min tick size as PriceStep (for compatibility with exchange layer)
    pub fn min_ticksize(&self) -> Price {
        Price::from_f32(self.tick_size)
    }
}

// Implement Hash and Eq for FuturesTickerInfo
impl std::hash::Hash for FuturesTickerInfo {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.ticker.hash(state);
    }
}

impl Eq for FuturesTickerInfo {}

// ── Timeframe ─────────────────────────────────────────────────────────

/// Timeframe for candle aggregation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Timeframe {
    // Time-based
    #[serde(rename = "1s")]
    M1s,
    #[serde(rename = "5s")]
    M5s,
    #[serde(rename = "10s")]
    M10s,
    #[serde(rename = "30s")]
    M30s,
    #[serde(rename = "1m")]
    M1,
    #[serde(rename = "3m")]
    M3,
    #[serde(rename = "5m")]
    M5,
    #[serde(rename = "15m")]
    M15,
    #[serde(rename = "30m")]
    M30,
    #[serde(rename = "1h")]
    H1,
    #[serde(rename = "4h")]
    H4,
    #[serde(rename = "1d")]
    D1,
}

impl Timeframe {
    /// Timeframes suitable for candlestick charts (UI constant)
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

    /// Timeframes suitable for heatmap charts (UI constant)
    pub const HEATMAP: [Timeframe; 6] = [
        Timeframe::M1s,
        Timeframe::M5s,
        Timeframe::M10s,
        Timeframe::M30s,
        Timeframe::M1,
        Timeframe::M5,
    ];

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

/// Ticker statistics (price, volume, etc.)
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TickerStats {
    pub mark_price: f32,
    pub daily_price_chg: f32,
    pub daily_volume: f32,
}

impl Default for TickerStats {
    fn default() -> Self {
        Self {
            mark_price: 0.0,
            daily_price_chg: 0.0,
            daily_volume: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expiration_date_parsing() {
        // Test specific contract with 2-digit year (using future date)
        let ticker = FuturesTicker::new("ESH26", FuturesVenue::CMEGlobex);
        eprintln!("Symbol: {}", ticker.as_str());
        eprintln!("Product: {}", ticker.product());

        let expiry = ticker.expiration_date();
        assert!(
            expiry.is_some(),
            "Expected expiration date to be parsed for ESH26, symbol: {}, product: {}",
            ticker.as_str(),
            ticker.product()
        );

        // March 2026 third Friday should be March 20, 2026
        if let Some(date) = expiry {
            assert_eq!(date.month(), 3);
            assert_eq!(date.year(), 2026);
        }

        // Test continuous contract (no expiration)
        let cont_ticker = FuturesTicker::new("ES.c.0", FuturesVenue::CMEGlobex);
        assert!(cont_ticker.expiration_date().is_none());
        assert!(!cont_ticker.is_expired());
    }

    #[test]
    fn test_month_code_mapping() {
        // Test all month codes (using future year 2026)
        let test_cases = vec![
            ("ESF26", 1),  // January
            ("ESG26", 2),  // February
            ("ESH26", 3),  // March
            ("ESJ26", 4),  // April
            ("ESK26", 5),  // May
            ("ESM26", 6),  // June
            ("ESN26", 7),  // July
            ("ESQ26", 8),  // August
            ("ESU26", 9),  // September
            ("ESV26", 10), // October
            ("ESX26", 11), // November
            ("ESZ26", 12), // December
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
        // Test with a future date (Dec 2026)
        let ticker = FuturesTicker::new("ESZ26", FuturesVenue::CMEGlobex);
        let days = ticker
            .days_until_expiry()
            .expect("ESZ26 should return Some for days_until_expiry");

        // ESZ26 expires on the third Friday of December 2026 (Dec 18, 2026).
        // This test is written in early 2026, so days should be positive.
        assert!(
            days > 0,
            "ESZ26 expiry is in Dec 2026, days_until_expiry should be positive, got {}",
            days
        );
        // Sanity upper bound: should be less than ~365 days from any point in 2026
        assert!(
            days < 365,
            "ESZ26 days_until_expiry should be less than 365, got {}",
            days
        );

        // Test continuous contract
        let cont_ticker = FuturesTicker::new("ES.c.0", FuturesVenue::CMEGlobex);
        assert!(cont_ticker.days_until_expiry().is_none());
    }
}
