use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::options::{OptionType, OptionChain, OptionSnapshot};
use super::types::{Price, Timestamp};

/// Gamma exposure for a single strike price
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GammaExposure {
    /// Strike price
    pub strike_price: Price,

    /// Total gamma exposure at this strike (calls - puts, adjusted for OI)
    pub total_gamma: f64,

    /// Call gamma exposure
    pub call_gamma: f64,

    /// Put gamma exposure (stored as positive value)
    pub put_gamma: f64,

    /// Net gamma (calls - puts)
    pub net_gamma: f64,

    /// Call open interest
    pub call_oi: u64,

    /// Put open interest
    pub put_oi: u64,

    /// Total open interest at strike
    pub total_oi: u64,

    /// Call delta-adjusted gamma (gamma * delta * OI)
    pub call_dex: f64,

    /// Put delta-adjusted gamma
    pub put_dex: f64,
}

impl GammaExposure {
    /// Create a new gamma exposure entry
    pub fn new(strike_price: Price) -> Self {
        Self {
            strike_price,
            total_gamma: 0.0,
            call_gamma: 0.0,
            put_gamma: 0.0,
            net_gamma: 0.0,
            call_oi: 0,
            put_oi: 0,
            total_oi: 0,
            call_dex: 0.0,
            put_dex: 0.0,
        }
    }

    /// Calculate gamma exposure from option snapshots at this strike
    ///
    /// Uses the industry-standard GEX formula:
    /// **GEX = Gamma × OI × Contract_Size × Spot_Price² × 0.01**
    ///
    /// This calculates notional gamma exposure representing how many dollars
    /// of underlying market makers must hedge per 1% move in the underlying.
    ///
    /// # Dealer Perspective
    /// Assumes market makers are SHORT options (standard assumption):
    /// - Short calls: Negative gamma (must buy rallies, sell dips) = destabilizing
    /// - Short puts: Negative gamma (must sell dips, buy rallies) = destabilizing
    ///
    /// # Returns
    /// GammaExposure with call_gamma, put_gamma in notional dollars.
    /// - Positive values: Stabilizing (dealers hedge against moves)
    /// - Negative values: Destabilizing (dealers amplify moves)
    pub fn calculate_from_contracts(
        strike_price: Price,
        contracts: &[&OptionSnapshot],
        underlying_price: Price,
    ) -> Self {
        let mut exposure = Self::new(strike_price);
        let spot = underlying_price.to_f64();

        for contract in contracts {
            let gamma = contract.greeks.gamma.unwrap_or(0.0);
            let delta = contract.greeks.delta.unwrap_or(0.0);
            let oi = contract.open_interest.unwrap_or(0) as f64;
            let shares_per_contract = contract.contract.shares_per_contract as f64;

            // Industry-standard GEX formula
            // GEX = Gamma × OI × Contract_Size × Spot_Price² × 0.01
            //
            // Why Spot²?
            // - Gamma measures delta change per $1 move
            // - For 1% move: need (1% × Spot) points
            // - Total exposure: Gamma × OI × 100 × (0.01 × Spot)
            // - Simplifies to: Gamma × OI × 100 × Spot² × 0.01
            let spot_squared = spot * spot;
            let gex_notional = gamma * oi * shares_per_contract * spot_squared * 0.01;

            // DEX (Delta-adjusted exposure): Delta × OI × shares × Spot
            let dex_notional = delta * oi * shares_per_contract * spot;

            // Dealer perspective: Assume dealers are SHORT options
            // Customer buys call -> Dealer sells call -> Dealer has -gamma
            // Customer buys put -> Dealer sells put -> Dealer has -gamma
            //
            // Convention: Display dealer GEX (negative = destabilizing)
            match contract.contract.contract_type {
                OptionType::Call => {
                    // Call gamma is positive for holders
                    // Dealers are short calls -> negative gamma for dealers
                    exposure.call_gamma += -gex_notional;
                    exposure.call_oi += contract.open_interest.unwrap_or(0);
                    exposure.call_dex += -dex_notional;
                }
                OptionType::Put => {
                    // Put gamma is positive for holders
                    // Dealers are short puts -> negative gamma for dealers
                    exposure.put_gamma += -gex_notional;
                    exposure.put_oi += contract.open_interest.unwrap_or(0);
                    exposure.put_dex += -dex_notional;
                }
                OptionType::Other => {}
            }
        }

        // Net gamma from dealer perspective
        // Negative total = dealers must chase (destabilizing)
        // Positive total = dealers provide support (stabilizing)
        exposure.net_gamma = exposure.call_gamma + exposure.put_gamma;
        exposure.total_gamma = exposure.call_gamma.abs() + exposure.put_gamma.abs();
        exposure.total_oi = exposure.call_oi + exposure.put_oi;

        exposure
    }

    /// Check if this is a significant gamma level (has substantial exposure)
    pub fn is_significant(&self, threshold: f64) -> bool {
        self.total_gamma.abs() > threshold
    }

    /// Check if this is a resistance level (net positive gamma)
    pub fn is_resistance(&self) -> bool {
        self.net_gamma > 0.0
    }

    /// Check if this is a support level (net negative gamma)
    pub fn is_support(&self) -> bool {
        self.net_gamma < 0.0
    }

    /// Get the dominant side (call or put)
    pub fn dominant_side(&self) -> &'static str {
        if self.call_gamma.abs() > self.put_gamma.abs() {
            "call"
        } else if self.put_gamma.abs() > self.call_gamma.abs() {
            "put"
        } else {
            "neutral"
        }
    }
}

/// Key gamma exposure level identified in the profile
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GexLevel {
    /// Strike price
    pub strike_price: Price,

    /// Gamma exposure value
    pub gamma: f64,

    /// Level type
    pub level_type: GexLevelType,

    /// Relative importance (0.0 to 1.0)
    pub importance: f64,
}

/// Type of GEX level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GexLevelType {
    /// Strong resistance (large positive net gamma)
    StrongResistance,

    /// Moderate resistance
    Resistance,

    /// Strong support (large negative net gamma)
    StrongSupport,

    /// Moderate support
    Support,

    /// Zero gamma level (potentially unstable)
    ZeroGamma,

    /// Maximum gamma level (highest absolute exposure)
    MaxGamma,
}

/// Complete gamma exposure profile for an underlying asset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GexProfile {
    /// Underlying asset ticker
    pub underlying_ticker: String,

    /// Profile date
    pub date: NaiveDate,

    /// Profile timestamp
    pub time: Timestamp,

    /// Current underlying price
    pub underlying_price: Option<Price>,

    /// Gamma exposure by strike price
    pub exposures: Vec<GammaExposure>,

    /// Total net gamma across all strikes
    pub total_net_gamma: f64,

    /// Total absolute gamma
    pub total_abs_gamma: f64,

    /// Key levels identified
    pub key_levels: Vec<GexLevel>,

    /// Zero gamma level (inflection point)
    pub zero_gamma_level: Option<Price>,

    /// Call/Put ratio based on gamma
    pub gamma_ratio: Option<f64>,

    /// Call wall (strike with highest call gamma concentration)
    pub call_wall: Option<Price>,

    /// Put wall (strike with highest put gamma concentration)
    pub put_wall: Option<Price>,

    /// Expected move based on gamma (±1 std deviation estimate)
    pub expected_move_pct: Option<f64>,
}

impl GexProfile {
    /// Create a new empty GEX profile
    pub fn new(underlying_ticker: String, date: NaiveDate, time: Timestamp) -> Self {
        Self {
            underlying_ticker,
            date,
            time,
            underlying_price: None,
            exposures: Vec::new(),
            total_net_gamma: 0.0,
            total_abs_gamma: 0.0,
            key_levels: Vec::new(),
            zero_gamma_level: None,
            gamma_ratio: None,
            call_wall: None,
            put_wall: None,
            expected_move_pct: None,
        }
    }

    /// Calculate GEX profile from an option chain
    ///
    /// # Errors
    /// Returns None if underlying_price is not available (required for accurate GEX)
    pub fn from_option_chain(chain: &OptionChain) -> Option<Self> {
        let mut profile = Self::new(
            chain.underlying_ticker.clone(),
            chain.date,
            chain.time,
        );

        // Underlying price is REQUIRED for accurate GEX calculation
        let underlying_price = chain.underlying_price?;
        profile.underlying_price = Some(underlying_price);

        // Filter contracts with complete Greeks data
        let contracts_with_greeks: Vec<&OptionSnapshot> = chain
            .contracts
            .iter()
            .filter(|c| c.greeks.gamma.is_some() && c.open_interest.is_some())
            .collect();

        if contracts_with_greeks.is_empty() {
            return Some(profile);
        }

        // Group contracts by strike price
        let mut by_strike: HashMap<Price, Vec<&OptionSnapshot>> = HashMap::new();
        for contract in contracts_with_greeks {
            by_strike
                .entry(contract.contract.strike_price)
                .or_default()
                .push(contract);
        }

        // Calculate gamma exposure for each strike (now with spot price)
        let mut exposures: Vec<GammaExposure> = by_strike
            .into_iter()
            .map(|(strike, contracts)| {
                GammaExposure::calculate_from_contracts(strike, &contracts, underlying_price)
            })
            .collect();

        // Sort by strike price
        exposures.sort_by_key(|e| e.strike_price.units());

        profile.exposures = exposures;

        // Calculate totals
        profile.calculate_totals();

        // Identify key levels
        profile.identify_key_levels();

        // Find zero gamma level
        profile.find_zero_gamma_level();

        // Calculate gamma ratio
        profile.calculate_gamma_ratio();

        // Identify call and put walls
        profile.identify_walls();

        // Calculate expected move
        profile.calculate_expected_move();

        Some(profile)
    }

    /// Calculate GEX profile from an option chain (fallible version)
    ///
    /// Returns Result instead of Option for better error handling
    pub fn try_from_option_chain(chain: &OptionChain) -> Result<Self, &'static str> {
        Self::from_option_chain(chain)
            .ok_or("Cannot calculate GEX: underlying price is required but not available")
    }

    /// Calculate total gamma values using Kahan summation for numerical stability
    fn calculate_totals(&mut self) {
        let mut net_sum = 0.0_f64;
        let mut net_comp = 0.0_f64;
        let mut abs_sum = 0.0_f64;
        let mut abs_comp = 0.0_f64;

        for exposure in &self.exposures {
            // Kahan sum for net gamma
            let y = exposure.net_gamma - net_comp;
            let t = net_sum + y;
            net_comp = (t - net_sum) - y;
            net_sum = t;

            // Kahan sum for absolute gamma
            let y = exposure.total_gamma - abs_comp;
            let t = abs_sum + y;
            abs_comp = (t - abs_sum) - y;
            abs_sum = t;
        }

        self.total_net_gamma = net_sum;
        self.total_abs_gamma = abs_sum;
    }

    /// Identify key gamma exposure levels
    fn identify_key_levels(&mut self) {
        if self.exposures.is_empty() {
            return;
        }

        // Find max absolute gamma
        let max_gamma = self
            .exposures
            .iter()
            .map(|e| e.total_gamma)
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or(0.0);

        if max_gamma == 0.0 {
            return;
        }

        // Identify significant levels (> 20% of max gamma)
        let significance_threshold = max_gamma * 0.2;

        for exposure in &self.exposures {
            if !exposure.is_significant(significance_threshold) {
                continue;
            }

            let importance = exposure.total_gamma / max_gamma;

            // Classify level type
            let level_type = if exposure.total_gamma == max_gamma {
                GexLevelType::MaxGamma
            } else if exposure.is_resistance() {
                if importance > 0.5 {
                    GexLevelType::StrongResistance
                } else {
                    GexLevelType::Resistance
                }
            } else if exposure.is_support() {
                if importance > 0.5 {
                    GexLevelType::StrongSupport
                } else {
                    GexLevelType::Support
                }
            } else {
                continue;
            };

            self.key_levels.push(GexLevel {
                strike_price: exposure.strike_price,
                gamma: exposure.total_gamma,
                level_type,
                importance,
            });
        }

        // Sort by importance
        self.key_levels
            .sort_by(|a, b| b.importance.partial_cmp(&a.importance).unwrap());
    }

    /// Find the zero gamma level (where net gamma crosses zero)
    fn find_zero_gamma_level(&mut self) {
        if self.exposures.len() < 2 {
            return;
        }

        // Look for sign change in net gamma
        for i in 0..self.exposures.len() - 1 {
            let current = &self.exposures[i];
            let next = &self.exposures[i + 1];

            // Check for sign change
            if (current.net_gamma > 0.0 && next.net_gamma < 0.0)
                || (current.net_gamma < 0.0 && next.net_gamma > 0.0)
            {
                // Linear interpolation to find zero crossing
                let ratio = current.net_gamma.abs()
                    / (current.net_gamma.abs() + next.net_gamma.abs());

                let zero_level_units = current.strike_price.units()
                    + ((next.strike_price.units() - current.strike_price.units()) as f64 * ratio)
                        as i64;

                self.zero_gamma_level = Some(Price::from_units(zero_level_units));

                // Add as key level
                self.key_levels.push(GexLevel {
                    strike_price: Price::from_units(zero_level_units),
                    gamma: 0.0,
                    level_type: GexLevelType::ZeroGamma,
                    importance: 0.9, // Very important level
                });

                break;
            }
        }
    }

    /// Calculate call/put gamma ratio using Kahan summation for numerical stability
    fn calculate_gamma_ratio(&mut self) {
        let mut call_sum = 0.0_f64;
        let mut call_comp = 0.0_f64;
        let mut put_sum = 0.0_f64;
        let mut put_comp = 0.0_f64;

        for exposure in &self.exposures {
            // Kahan sum for call gamma
            let y = exposure.call_gamma.abs() - call_comp;
            let t = call_sum + y;
            call_comp = (t - call_sum) - y;
            call_sum = t;

            // Kahan sum for put gamma
            let y = exposure.put_gamma.abs() - put_comp;
            let t = put_sum + y;
            put_comp = (t - put_sum) - y;
            put_sum = t;
        }

        if put_sum > 0.0 {
            self.gamma_ratio = Some(call_sum / put_sum);
        }
    }

    /// Identify call and put walls (highest gamma concentrations)
    fn identify_walls(&mut self) {
        if self.exposures.is_empty() {
            return;
        }

        // Call wall: Strike with highest absolute call gamma
        let call_wall_exposure = self
            .exposures
            .iter()
            .max_by(|a, b| {
                a.call_gamma
                    .abs()
                    .partial_cmp(&b.call_gamma.abs())
                    .unwrap()
            });

        if let Some(exposure) = call_wall_exposure
            && exposure.call_gamma.abs() > 0.0 {
                self.call_wall = Some(exposure.strike_price);
            }

        // Put wall: Strike with highest absolute put gamma
        let put_wall_exposure = self
            .exposures
            .iter()
            .max_by(|a, b| {
                a.put_gamma
                    .abs()
                    .partial_cmp(&b.put_gamma.abs())
                    .unwrap()
            });

        if let Some(exposure) = put_wall_exposure
            && exposure.put_gamma.abs() > 0.0 {
                self.put_wall = Some(exposure.strike_price);
            }
    }

    /// Calculate expected move based on gamma exposure
    ///
    /// Uses total absolute gamma to estimate ±1 std deviation move.
    /// Higher gamma = lower expected volatility = smaller expected move.
    ///
    /// This is a simplified heuristic. More accurate calculation would use:
    /// - Implied volatility weighted by volume
    /// - Time to expiration
    /// - Historical volatility correlation
    fn calculate_expected_move(&mut self) {
        let _spot = match self.underlying_price {
            Some(price) => price.to_f64(),
            None => return,
        };

        if self.total_abs_gamma == 0.0 {
            return;
        }

        // Heuristic: Expected move inversely proportional to gamma
        // High gamma (~50B) -> Low vol (~1-2%)
        // Low gamma (~5B) -> High vol (~3-5%)
        //
        // Formula: expected_move_pct ≈ base_vol / sqrt(gamma_billions)
        let gamma_billions = self.total_abs_gamma / 1_000_000_000.0;

        if gamma_billions > 0.0 {
            let base_volatility = 0.05; // 5% baseline
            let gamma_factor = 1.0 / gamma_billions.sqrt();
            self.expected_move_pct = Some(base_volatility * gamma_factor);
        }
    }

    /// Get the strongest resistance level
    pub fn strongest_resistance(&self) -> Option<&GexLevel> {
        self.key_levels
            .iter()
            .filter(|l| {
                matches!(
                    l.level_type,
                    GexLevelType::StrongResistance | GexLevelType::Resistance
                )
            })
            .max_by(|a, b| a.importance.partial_cmp(&b.importance).unwrap())
    }

    /// Get the strongest support level
    pub fn strongest_support(&self) -> Option<&GexLevel> {
        self.key_levels
            .iter()
            .filter(|l| {
                matches!(
                    l.level_type,
                    GexLevelType::StrongSupport | GexLevelType::Support
                )
            })
            .max_by(|a, b| a.importance.partial_cmp(&b.importance).unwrap())
    }

    /// Get exposure at a specific strike price
    pub fn exposure_at_strike(&self, strike: Price) -> Option<&GammaExposure> {
        self.exposures.iter().find(|e| e.strike_price == strike)
    }

    /// Get the nearest exposure level to a given price
    pub fn nearest_exposure(&self, price: Price) -> Option<&GammaExposure> {
        self.exposures.iter().min_by_key(|e| {
            let diff = e.strike_price.units() - price.units();
            diff.abs()
        })
    }

    /// Check if price is above zero gamma level (bullish zone)
    pub fn is_in_bullish_zone(&self, price: Price) -> Option<bool> {
        self.zero_gamma_level.map(|zero| price > zero)
    }

    /// Get all resistance levels above a price
    pub fn resistances_above(&self, price: Price) -> Vec<&GexLevel> {
        self.key_levels
            .iter()
            .filter(|l| {
                l.strike_price > price
                    && matches!(
                        l.level_type,
                        GexLevelType::StrongResistance | GexLevelType::Resistance
                    )
            })
            .collect()
    }

    /// Get all support levels below a price
    pub fn supports_below(&self, price: Price) -> Vec<&GexLevel> {
        self.key_levels
            .iter()
            .filter(|l| {
                l.strike_price < price
                    && matches!(
                        l.level_type,
                        GexLevelType::StrongSupport | GexLevelType::Support
                    )
            })
            .collect()
    }

    /// Get exposure count
    pub fn exposure_count(&self) -> usize {
        self.exposures.len()
    }

    /// Check if profile has data
    pub fn has_data(&self) -> bool {
        !self.exposures.is_empty()
    }

    /// Get expected move in dollars (±1 std dev)
    pub fn expected_move_dollars(&self) -> Option<(f64, f64)> {
        let spot = self.underlying_price?.to_f64();
        let move_pct = self.expected_move_pct?;

        let move_dollars = spot * move_pct;
        Some((spot - move_dollars, spot + move_dollars))
    }

    /// Get call wall details
    pub fn call_wall_details(&self) -> Option<&GammaExposure> {
        let call_wall = self.call_wall?;
        self.exposure_at_strike(call_wall)
    }

    /// Get put wall details
    pub fn put_wall_details(&self) -> Option<&GammaExposure> {
        let put_wall = self.put_wall?;
        self.exposure_at_strike(put_wall)
    }

    /// Check if market is in a high gamma environment (stabilizing)
    pub fn is_high_gamma_environment(&self) -> bool {
        // High gamma (> $20B absolute) indicates stabilizing conditions
        self.total_abs_gamma > 20_000_000_000.0
    }

    /// Check if market is in a low gamma environment (volatile)
    pub fn is_low_gamma_environment(&self) -> bool {
        // Low gamma (< $5B absolute) indicates volatile conditions
        self.total_abs_gamma < 5_000_000_000.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::options::{ExerciseStyle, Greek, OptionContract, OptionSnapshot};
    use chrono::Utc;

    fn create_test_snapshot(
        underlying: &str,
        strike: f64,
        contract_type: OptionType,
        gamma: f64,
        delta: f64,
        oi: u64,
    ) -> OptionSnapshot {
        let contract = OptionContract::new(
            format!("O:{}_{}", underlying, strike),
            underlying.to_string(),
            Price::from_f64(strike),
            NaiveDate::from_ymd_opt(2024, 12, 31).unwrap(),
            contract_type,
            ExerciseStyle::American,
        );

        let mut snapshot = OptionSnapshot::new(contract, Timestamp(Utc::now().timestamp_millis() as u64));
        snapshot.greeks = Greek {
            delta: Some(delta),
            gamma: Some(gamma),
            theta: Some(-0.05),
            vega: Some(0.2),
            rho: None,
        };
        snapshot.open_interest = Some(oi);

        snapshot
    }

    #[test]
    fn test_gamma_exposure_calculation() {
        let strike = Price::from_f64(450.0);
        let spot = Price::from_f64(450.0); // SPY at $450

        // Realistic values for SPY options
        let call = create_test_snapshot("SPY", 450.0, OptionType::Call, 0.05, 0.5, 10000);
        let put = create_test_snapshot("SPY", 450.0, OptionType::Put, 0.04, -0.5, 8000);

        let contracts = vec![&call, &put];
        let exposure = GammaExposure::calculate_from_contracts(strike, &contracts, spot);

        // With correct formula: GEX = Gamma × OI × 100 × Spot² × 0.01
        // Call: 0.05 × 10000 × 100 × 450² × 0.01 = -10,125,000 (dealer short)
        // Put: 0.04 × 8000 × 100 × 450² × 0.01 = -6,480,000 (dealer short)

        // Values should be negative (dealer perspective)
        assert!(exposure.call_gamma < 0.0, "Call gamma should be negative (dealer short)");
        assert!(exposure.put_gamma < 0.0, "Put gamma should be negative (dealer short)");

        // Check magnitudes are reasonable (in millions)
        assert!(exposure.call_gamma.abs() > 1_000_000.0);
        assert!(exposure.put_gamma.abs() > 1_000_000.0);

        // Check OI tracking
        assert_eq!(exposure.call_oi, 10000);
        assert_eq!(exposure.put_oi, 8000);
        assert_eq!(exposure.total_oi, 18000);
    }

    #[test]
    fn test_gex_level_classification() {
        // Realistic GEX values (dealer perspective, negative)
        let resistance = GammaExposure {
            strike_price: Price::from_f64(455.0),
            net_gamma: -5_000_000.0, // -$5M net (more call than put)
            call_gamma: -8_000_000.0, // -$8M calls
            put_gamma: -3_000_000.0, // -$3M puts
            ..GammaExposure::new(Price::from_f64(455.0))
        };

        // When net is negative and calls dominate: resistance
        assert!(resistance.call_gamma.abs() > resistance.put_gamma.abs());
        assert_eq!(resistance.dominant_side(), "call");

        let support = GammaExposure {
            strike_price: Price::from_f64(445.0),
            net_gamma: -6_000_000.0, // -$6M net (more put than call)
            call_gamma: -2_000_000.0, // -$2M calls
            put_gamma: -8_000_000.0, // -$8M puts
            ..GammaExposure::new(Price::from_f64(445.0))
        };

        // When puts dominate: support
        assert!(support.put_gamma.abs() > support.call_gamma.abs());
        assert_eq!(support.dominant_side(), "put");
    }

    #[test]
    fn test_gex_profile_from_chain() {
        let mut chain = OptionChain::new(
            "SPY".to_string(),
            NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            Timestamp(Utc::now().timestamp_millis() as u64),
        );

        // Realistic SPY at $450
        chain.underlying_price = Some(Price::from_f64(450.0));

        // Add calls and puts at different strikes with realistic Greeks/OI
        chain.add_contract(create_test_snapshot(
            "SPY",
            445.0,
            OptionType::Call,
            0.03,
            0.3,
            5000,
        ));
        chain.add_contract(create_test_snapshot(
            "SPY",
            445.0,
            OptionType::Put,
            0.05,
            -0.7,
            12000,
        ));

        chain.add_contract(create_test_snapshot(
            "SPY",
            450.0,
            OptionType::Call,
            0.05,
            0.5,
            15000,
        ));
        chain.add_contract(create_test_snapshot(
            "SPY",
            450.0,
            OptionType::Put,
            0.04,
            -0.5,
            10000,
        ));

        chain.add_contract(create_test_snapshot(
            "SPY",
            455.0,
            OptionType::Call,
            0.04,
            0.7,
            18000,
        ));
        chain.add_contract(create_test_snapshot(
            "SPY",
            455.0,
            OptionType::Put,
            0.02,
            -0.3,
            4000,
        ));

        let profile = GexProfile::from_option_chain(&chain).expect("Should calculate GEX");

        assert_eq!(profile.underlying_ticker, "SPY");
        assert_eq!(profile.exposure_count(), 3); // 3 strikes
        assert!(profile.has_data());

        // Total gamma should be in billions for SPY with realistic OI
        assert!(profile.total_abs_gamma > 100_000_000.0, "Total gamma should be > $100M");

        // Should have identified key levels
        assert!(!profile.key_levels.is_empty());

        // Should have call and put walls
        assert!(profile.call_wall.is_some(), "Should identify call wall");
        assert!(profile.put_wall.is_some(), "Should identify put wall");

        // Expected move should be calculated
        assert!(profile.expected_move_pct.is_some(), "Should calculate expected move");
    }
}
