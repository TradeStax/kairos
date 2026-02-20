use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::fmt;

use super::types::{Price, Quantity, Timestamp};

/// Option contract type (call or put)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OptionType {
    Call,
    Put,
    Other,
}

impl fmt::Display for OptionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OptionType::Call => write!(f, "call"),
            OptionType::Put => write!(f, "put"),
            OptionType::Other => write!(f, "other"),
        }
    }
}

/// Exercise style for options contracts
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExerciseStyle {
    American,
    European,
    Bermudan,
}

impl fmt::Display for ExerciseStyle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExerciseStyle::American => write!(f, "american"),
            ExerciseStyle::European => write!(f, "european"),
            ExerciseStyle::Bermudan => write!(f, "bermudan"),
        }
    }
}

/// Greek values for an option contract
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Greek {
    /// Delta: Rate of change of option price with respect to underlying price
    pub delta: Option<f64>,

    /// Gamma: Rate of change of delta with respect to underlying price
    pub gamma: Option<f64>,

    /// Theta: Rate of change of option price with respect to time (time decay)
    pub theta: Option<f64>,

    /// Vega: Rate of change of option price with respect to volatility
    pub vega: Option<f64>,

    /// Rho: Rate of change of option price with respect to interest rates
    pub rho: Option<f64>,
}

impl Greek {
    /// Create a new Greek with all values set to None
    pub fn empty() -> Self {
        Self {
            delta: None,
            gamma: None,
            theta: None,
            vega: None,
            rho: None,
        }
    }

    /// Create a new Greek with all required values
    pub fn new(delta: f64, gamma: f64, theta: f64, vega: f64) -> Self {
        Self {
            delta: Some(delta),
            gamma: Some(gamma),
            theta: Some(theta),
            vega: Some(vega),
            rho: None,
        }
    }

    /// Check if all greek values are present
    pub fn is_complete(&self) -> bool {
        self.delta.is_some() && self.gamma.is_some() && self.theta.is_some() && self.vega.is_some()
    }

    /// Check if any greek value is present
    pub fn has_data(&self) -> bool {
        self.delta.is_some()
            || self.gamma.is_some()
            || self.theta.is_some()
            || self.vega.is_some()
            || self.rho.is_some()
    }
}

impl Default for Greek {
    fn default() -> Self {
        Self::empty()
    }
}

/// Option contract metadata and identification
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OptionContract {
    /// Full option ticker symbol (e.g., "O:TSLA210903C00700000")
    pub ticker: String,

    /// Underlying asset ticker (e.g., "TSLA")
    pub underlying_ticker: String,

    /// Strike price
    pub strike_price: Price,

    /// Expiration date
    pub expiration_date: NaiveDate,

    /// Contract type (call/put)
    pub contract_type: OptionType,

    /// Exercise style
    pub exercise_style: ExerciseStyle,

    /// Number of shares per contract (typically 100)
    pub shares_per_contract: u32,

    /// Primary exchange MIC code (optional)
    pub primary_exchange: Option<String>,

    /// CFI classification code (optional)
    pub cfi: Option<String>,
}

impl OptionContract {
    /// Create a new option contract
    pub fn new(
        ticker: String,
        underlying_ticker: String,
        strike_price: Price,
        expiration_date: NaiveDate,
        contract_type: OptionType,
        exercise_style: ExerciseStyle,
    ) -> Self {
        Self {
            ticker,
            underlying_ticker,
            strike_price,
            expiration_date,
            contract_type,
            exercise_style,
            shares_per_contract: 100,
            primary_exchange: None,
            cfi: None,
        }
    }

    /// Check if the contract has expired
    pub fn is_expired(&self, as_of: NaiveDate) -> bool {
        self.expiration_date < as_of
    }

    /// Days until expiration (negative if expired)
    pub fn days_to_expiry(&self, as_of: NaiveDate) -> i64 {
        (self.expiration_date - as_of).num_days()
    }

    /// Check if this is a call option
    pub fn is_call(&self) -> bool {
        self.contract_type == OptionType::Call
    }

    /// Check if this is a put option
    pub fn is_put(&self) -> bool {
        self.contract_type == OptionType::Put
    }
}

impl fmt::Display for OptionContract {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {} ${} {}",
            self.underlying_ticker,
            self.expiration_date.format("%Y-%m-%d"),
            self.strike_price.to_f64(),
            self.contract_type
        )
    }
}

/// Market snapshot for an option contract at a specific point in time
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OptionSnapshot {
    /// Contract details
    pub contract: OptionContract,

    /// Snapshot timestamp
    pub time: Timestamp,

    /// Last trade price (if available)
    pub last_price: Option<Price>,

    /// Bid price (if available)
    pub bid: Option<Price>,

    /// Ask price (if available)
    pub ask: Option<Price>,

    /// Bid size
    pub bid_size: Option<Quantity>,

    /// Ask size
    pub ask_size: Option<Quantity>,

    /// Implied volatility (as percentage, e.g., 0.25 = 25%)
    pub implied_volatility: Option<f64>,

    /// Greek values
    pub greeks: Greek,

    /// Open interest (contracts outstanding)
    pub open_interest: Option<u64>,

    /// Volume for the day
    pub volume: Option<u64>,

    /// Break-even price
    pub break_even_price: Option<Price>,

    /// Underlying asset price at snapshot time
    pub underlying_price: Option<Price>,
}

impl OptionSnapshot {
    /// Create a new option snapshot
    pub fn new(contract: OptionContract, time: Timestamp) -> Self {
        Self {
            contract,
            time,
            last_price: None,
            bid: None,
            ask: None,
            bid_size: None,
            ask_size: None,
            implied_volatility: None,
            greeks: Greek::empty(),
            open_interest: None,
            volume: None,
            break_even_price: None,
            underlying_price: None,
        }
    }

    /// Calculate mid price from bid/ask
    pub fn mid_price(&self) -> Option<Price> {
        match (self.bid, self.ask) {
            (Some(bid), Some(ask)) => Some(Price::from_units((bid.units() + ask.units()) / 2)),
            _ => None,
        }
    }

    /// Calculate spread from bid/ask
    pub fn spread(&self) -> Option<Price> {
        match (self.bid, self.ask) {
            (Some(bid), Some(ask)) => Some(Price::from_units(ask.units() - bid.units())),
            _ => None,
        }
    }

    /// Check if snapshot has complete market data
    pub fn has_market_data(&self) -> bool {
        self.last_price.is_some() || (self.bid.is_some() && self.ask.is_some())
    }

    /// Check if snapshot has Greeks data
    pub fn has_greeks(&self) -> bool {
        self.greeks.has_data()
    }

    /// Check if snapshot has IV data
    pub fn has_iv(&self) -> bool {
        self.implied_volatility.is_some()
    }

    /// Get the best available price (last, mid, or bid/ask)
    pub fn best_price(&self) -> Option<Price> {
        self.last_price
            .or_else(|| self.mid_price())
            .or(self.bid)
            .or(self.ask)
    }

    /// Check if the contract is in-the-money based on underlying price
    pub fn is_itm(&self) -> Option<bool> {
        self.underlying_price.map(|underlying| {
            let strike = self.contract.strike_price;
            match self.contract.contract_type {
                OptionType::Call => underlying > strike,
                OptionType::Put => underlying < strike,
                OptionType::Other => false,
            }
        })
    }

    /// Calculate intrinsic value
    pub fn intrinsic_value(&self) -> Option<Price> {
        self.underlying_price.map(|underlying| {
            let strike = self.contract.strike_price;
            let value = match self.contract.contract_type {
                OptionType::Call => (underlying.units() - strike.units()).max(0),
                OptionType::Put => (strike.units() - underlying.units()).max(0),
                OptionType::Other => 0,
            };
            Price::from_units(value)
        })
    }

    /// Calculate extrinsic value (time value)
    pub fn extrinsic_value(&self) -> Option<Price> {
        match (self.best_price(), self.intrinsic_value()) {
            (Some(price), Some(intrinsic)) => Some(Price::from_units(
                (price.units() - intrinsic.units()).max(0),
            )),
            _ => None,
        }
    }
}

/// Collection of option contracts for a single underlying at a specific time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptionChain {
    /// Underlying asset ticker
    pub underlying_ticker: String,

    /// Chain snapshot date
    pub date: NaiveDate,

    /// Snapshot timestamp
    pub time: Timestamp,

    /// Current underlying price
    pub underlying_price: Option<Price>,

    /// All option contracts in the chain
    pub contracts: Vec<OptionSnapshot>,
}

impl OptionChain {
    /// Create a new empty option chain
    pub fn new(underlying_ticker: String, date: NaiveDate, time: Timestamp) -> Self {
        Self {
            underlying_ticker,
            date,
            time,
            underlying_price: None,
            contracts: Vec::new(),
        }
    }

    /// Add a contract to the chain
    pub fn add_contract(&mut self, snapshot: OptionSnapshot) {
        self.contracts.push(snapshot);
    }

    /// Get all call options
    pub fn calls(&self) -> Vec<&OptionSnapshot> {
        self.contracts
            .iter()
            .filter(|s| s.contract.is_call())
            .collect()
    }

    /// Get all put options
    pub fn puts(&self) -> Vec<&OptionSnapshot> {
        self.contracts
            .iter()
            .filter(|s| s.contract.is_put())
            .collect()
    }

    /// Get contracts for a specific expiration date
    pub fn by_expiration(&self, expiration: NaiveDate) -> Vec<&OptionSnapshot> {
        self.contracts
            .iter()
            .filter(|s| s.contract.expiration_date == expiration)
            .collect()
    }

    /// Get contracts for a specific strike price
    pub fn by_strike(&self, strike: Price) -> Vec<&OptionSnapshot> {
        self.contracts
            .iter()
            .filter(|s| s.contract.strike_price == strike)
            .collect()
    }

    /// Get all unique expiration dates in the chain
    pub fn expiration_dates(&self) -> Vec<NaiveDate> {
        let mut dates: Vec<NaiveDate> = self
            .contracts
            .iter()
            .map(|s| s.contract.expiration_date)
            .collect();
        dates.sort();
        dates.dedup();
        dates
    }

    /// Get all unique strike prices in the chain
    pub fn strike_prices(&self) -> Vec<Price> {
        let mut strikes: Vec<Price> = self
            .contracts
            .iter()
            .map(|s| s.contract.strike_price)
            .collect();
        strikes.sort();
        strikes.dedup();
        strikes
    }

    /// Get the number of contracts in the chain
    pub fn contract_count(&self) -> usize {
        self.contracts.len()
    }

    /// Check if chain is empty
    pub fn is_empty(&self) -> bool {
        self.contracts.is_empty()
    }

    /// Find the at-the-money strike price
    pub fn atm_strike(&self) -> Option<Price> {
        let underlying = self.underlying_price?;
        let strikes = self.strike_prices();

        if strikes.is_empty() {
            return None;
        }

        // Find strike closest to underlying price
        strikes.into_iter().min_by_key(|strike| {
            let diff = strike.units() - underlying.units();
            diff.abs()
        })
    }

    /// Filter contracts with complete Greeks data
    pub fn with_greeks(&self) -> Vec<&OptionSnapshot> {
        self.contracts
            .iter()
            .filter(|s| s.greeks.is_complete())
            .collect()
    }

    /// Filter contracts with IV data
    pub fn with_iv(&self) -> Vec<&OptionSnapshot> {
        self.contracts
            .iter()
            .filter(|s| s.implied_volatility.is_some())
            .collect()
    }

    /// Calculate total open interest for the chain
    pub fn total_open_interest(&self) -> u64 {
        self.contracts.iter().filter_map(|s| s.open_interest).sum()
    }

    /// Calculate total volume for the chain
    pub fn total_volume(&self) -> u64 {
        self.contracts.iter().filter_map(|s| s.volume).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_option_type_display() {
        assert_eq!(OptionType::Call.to_string(), "call");
        assert_eq!(OptionType::Put.to_string(), "put");
    }

    #[test]
    fn test_greek_completeness() {
        let empty = Greek::empty();
        assert!(!empty.is_complete());
        assert!(!empty.has_data());

        let partial = Greek {
            delta: Some(0.5),
            gamma: None,
            theta: None,
            vega: None,
            rho: None,
        };
        assert!(!partial.is_complete());
        assert!(partial.has_data());

        let complete = Greek::new(0.5, 0.1, -0.05, 0.2);
        assert!(complete.is_complete());
        assert!(complete.has_data());
    }

    #[test]
    fn test_option_contract_expiry() {
        let contract = OptionContract::new(
            "O:TEST".to_string(),
            "TEST".to_string(),
            Price::from_f64(100.0),
            NaiveDate::from_ymd_opt(2024, 12, 31).unwrap(),
            OptionType::Call,
            ExerciseStyle::American,
        );

        let before = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
        let after = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();

        assert!(!contract.is_expired(before));
        assert!(contract.is_expired(after));
        assert!(contract.days_to_expiry(before) > 0);
        assert!(contract.days_to_expiry(after) < 0);
    }

    #[test]
    fn test_option_snapshot_prices() {
        let contract = OptionContract::new(
            "O:TEST".to_string(),
            "TEST".to_string(),
            Price::from_f64(100.0),
            NaiveDate::from_ymd_opt(2024, 12, 31).unwrap(),
            OptionType::Call,
            ExerciseStyle::American,
        );

        let mut snapshot =
            OptionSnapshot::new(contract, Timestamp(Utc::now().timestamp_millis() as u64));
        snapshot.bid = Some(Price::from_f64(5.0));
        snapshot.ask = Some(Price::from_f64(5.5));

        assert_eq!(snapshot.mid_price(), Some(Price::from_f64(5.25)));
        assert_eq!(snapshot.spread(), Some(Price::from_f64(0.5)));
        assert!(snapshot.has_market_data());
    }

    #[test]
    fn test_intrinsic_value() {
        let call_contract = OptionContract::new(
            "O:TEST_CALL".to_string(),
            "TEST".to_string(),
            Price::from_f64(100.0),
            NaiveDate::from_ymd_opt(2024, 12, 31).unwrap(),
            OptionType::Call,
            ExerciseStyle::American,
        );

        let mut call_snapshot = OptionSnapshot::new(
            call_contract,
            Timestamp(Utc::now().timestamp_millis() as u64),
        );
        call_snapshot.underlying_price = Some(Price::from_f64(105.0));

        assert_eq!(call_snapshot.intrinsic_value(), Some(Price::from_f64(5.0)));
        assert_eq!(call_snapshot.is_itm(), Some(true));

        let put_contract = OptionContract::new(
            "O:TEST_PUT".to_string(),
            "TEST".to_string(),
            Price::from_f64(100.0),
            NaiveDate::from_ymd_opt(2024, 12, 31).unwrap(),
            OptionType::Put,
            ExerciseStyle::American,
        );

        let mut put_snapshot = OptionSnapshot::new(
            put_contract,
            Timestamp(Utc::now().timestamp_millis() as u64),
        );
        put_snapshot.underlying_price = Some(Price::from_f64(95.0));

        assert_eq!(put_snapshot.intrinsic_value(), Some(Price::from_f64(5.0)));
        assert_eq!(put_snapshot.is_itm(), Some(true));
    }

    #[test]
    fn test_option_chain() {
        let mut chain = OptionChain::new(
            "TEST".to_string(),
            NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            Timestamp(Utc::now().timestamp_millis() as u64),
        );

        chain.underlying_price = Some(Price::from_f64(100.0));

        // Add call and put at same strike
        let call = OptionContract::new(
            "O:TEST_CALL".to_string(),
            "TEST".to_string(),
            Price::from_f64(100.0),
            NaiveDate::from_ymd_opt(2024, 12, 31).unwrap(),
            OptionType::Call,
            ExerciseStyle::American,
        );
        chain.add_contract(OptionSnapshot::new(call, chain.time));

        let put = OptionContract::new(
            "O:TEST_PUT".to_string(),
            "TEST".to_string(),
            Price::from_f64(100.0),
            NaiveDate::from_ymd_opt(2024, 12, 31).unwrap(),
            OptionType::Put,
            ExerciseStyle::American,
        );
        chain.add_contract(OptionSnapshot::new(put, chain.time));

        assert_eq!(chain.contract_count(), 2);
        assert_eq!(chain.calls().len(), 1);
        assert_eq!(chain.puts().len(), 1);
        assert_eq!(chain.atm_strike(), Some(Price::from_f64(100.0)));
    }
}
