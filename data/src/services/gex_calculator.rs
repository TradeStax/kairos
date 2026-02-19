use crate::domain::{GexProfile, OptionChain, Price};

/// GEX (Gamma Exposure) calculation service
///
/// Provides utilities for calculating and analyzing gamma exposure
/// from option chains. Pure computational service with no I/O.
pub struct GexCalculationService;

impl GexCalculationService {
    /// Create a new GEX calculation service
    pub fn new() -> Self {
        Self
    }

    /// Calculate GEX profile from an option chain
    ///
    /// This is a convenience method that delegates to GexProfile::from_option_chain.
    /// Provided for consistency with service-oriented architecture.
    ///
    /// # Returns
    /// - `Some(GexProfile)` if underlying price is available
    /// - `None` if underlying price is missing (required for accurate GEX)
    pub fn calculate_profile(&self, chain: &OptionChain) -> Option<GexProfile> {
        GexProfile::from_option_chain(chain)
    }

    /// Calculate GEX profile from an option chain (fallible)
    ///
    /// Returns Result for better error handling.
    pub fn try_calculate_profile(&self, chain: &OptionChain) -> Result<GexProfile, &'static str> {
        GexProfile::try_from_option_chain(chain)
    }

    /// Identify the zero gamma level (inflection point)
    ///
    /// Returns the price level where gamma exposure crosses zero.
    /// Above this level: positive gamma (resistance)
    /// Below this level: negative gamma (support)
    pub fn find_zero_gamma_level(&self, profile: &GexProfile) -> Option<Price> {
        profile.zero_gamma_level
    }

    /// Find the nearest significant resistance level above a price
    pub fn nearest_resistance_above(&self, profile: &GexProfile, price: Price) -> Option<Price> {
        profile
            .resistances_above(price)
            .first()
            .map(|level| level.strike_price)
    }

    /// Find the nearest significant support level below a price
    pub fn nearest_support_below(&self, profile: &GexProfile, price: Price) -> Option<Price> {
        profile
            .supports_below(price)
            .last()
            .map(|level| level.strike_price)
    }

    /// Calculate expected move range based on gamma walls
    ///
    /// Returns (support_level, resistance_level) representing the expected trading range
    /// based on significant gamma concentrations.
    pub fn calculate_expected_range(&self, profile: &GexProfile) -> Option<(Price, Price)> {
        let strongest_support = profile.strongest_support()?;
        let strongest_resistance = profile.strongest_resistance()?;

        Some((
            strongest_support.strike_price,
            strongest_resistance.strike_price,
        ))
    }

    /// Determine market regime based on GEX profile
    ///
    /// Returns a description of the current gamma regime:
    /// - "Positive Gamma": Market makers long gamma (low volatility expected)
    /// - "Negative Gamma": Market makers short gamma (high volatility expected)
    /// - "Neutral Gamma": Balanced gamma exposure
    pub fn determine_market_regime(&self, profile: &GexProfile) -> &'static str {
        if profile.total_net_gamma > 1_000_000.0 {
            "Positive Gamma"
        } else if profile.total_net_gamma < -1_000_000.0 {
            "Negative Gamma"
        } else {
            "Neutral Gamma"
        }
    }

    /// Calculate volatility expectation based on GEX
    ///
    /// Higher absolute gamma typically indicates lower expected volatility
    /// as market makers hedge more actively.
    ///
    /// Returns a volatility score (0.0 = low vol, 1.0 = high vol)
    pub fn calculate_volatility_expectation(&self, profile: &GexProfile) -> f64 {
        // Inverse relationship: more gamma = less volatility
        let abs_gamma = profile.total_abs_gamma;

        if abs_gamma == 0.0 {
            return 0.5; // Neutral
        }

        // Normalize based on typical ranges
        // High gamma (>10M): low volatility (0.0-0.3)
        // Medium gamma (1M-10M): moderate volatility (0.3-0.7)
        // Low gamma (<1M): high volatility (0.7-1.0)

        if abs_gamma > 10_000_000.0 {
            0.2
        } else if abs_gamma > 5_000_000.0 {
            0.4
        } else if abs_gamma > 1_000_000.0 {
            0.6
        } else {
            0.8
        }
    }

    /// Identify gamma squeeze potential
    ///
    /// A gamma squeeze occurs when price moves through high gamma strikes,
    /// forcing market makers to chase the market.
    ///
    /// Returns true if conditions are present for a potential squeeze.
    pub fn has_squeeze_potential(&self, profile: &GexProfile, current_price: Price) -> bool {
        // Check if price is near a high gamma concentration
        if let Some(nearest_exposure) = profile.nearest_exposure(current_price) {
            // High open interest + high gamma = squeeze potential
            if nearest_exposure.total_oi > 10_000 && nearest_exposure.total_gamma > 500_000.0 {
                // Check if price is within 2% of the strike
                let strike_distance = (nearest_exposure.strike_price.to_f64()
                    - current_price.to_f64())
                .abs();
                let price_pct = strike_distance / current_price.to_f64();

                return price_pct < 0.02;
            }
        }

        false
    }

    /// Calculate gamma-weighted average strike
    ///
    /// Returns the gamma-weighted center of the option chain,
    /// which can indicate where market makers are most exposed.
    pub fn gamma_weighted_strike(&self, profile: &GexProfile) -> Option<f64> {
        if profile.exposures.is_empty() {
            return None;
        }

        let mut total_weighted_strike = 0.0;
        let mut total_gamma = 0.0;

        for exposure in &profile.exposures {
            let weight = exposure.total_gamma;
            total_weighted_strike += exposure.strike_price.to_f64() * weight;
            total_gamma += weight;
        }

        if total_gamma > 0.0 {
            Some(total_weighted_strike / total_gamma)
        } else {
            None
        }
    }

    /// Analyze put/call skew from gamma distribution
    ///
    /// Positive skew: More call gamma (bullish sentiment)
    /// Negative skew: More put gamma (bearish sentiment)
    pub fn analyze_gamma_skew(&self, profile: &GexProfile) -> (&'static str, f64) {
        let ratio = profile.gamma_ratio.unwrap_or(1.0);

        let sentiment = if ratio > 1.5 {
            "Bullish"
        } else if ratio < 0.67 {
            "Bearish"
        } else {
            "Neutral"
        };

        (sentiment, ratio)
    }
}

impl Default for GexCalculationService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{ExerciseStyle, Greek, OptionContract, OptionSnapshot, OptionType};
    use crate::domain::Timestamp;
    use chrono::{NaiveDate, Utc};

    fn create_test_chain_with_gamma() -> OptionChain {
        let mut chain = OptionChain::new(
            "SPY".to_string(),
            NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            Timestamp(Utc::now().timestamp_millis() as u64),
        );

        chain.underlying_price = Some(Price::from_f64(450.0));

        // Add options with significant gamma at different strikes
        for strike in [440.0, 445.0, 450.0, 455.0, 460.0] {
            // Call
            let call = OptionContract::new(
                format!("O:SPY_C_{}", strike),
                "SPY".to_string(),
                Price::from_f64(strike),
                NaiveDate::from_ymd_opt(2024, 3, 15).unwrap(),
                OptionType::Call,
                ExerciseStyle::American,
            );
            let mut call_snapshot = OptionSnapshot::new(call, chain.time);
            call_snapshot.greeks = Greek::new(0.5, 0.05, -0.02, 0.15);
            call_snapshot.open_interest = Some(10000);
            call_snapshot.underlying_price = Some(Price::from_f64(450.0));
            chain.add_contract(call_snapshot);

            // Put
            let put = OptionContract::new(
                format!("O:SPY_P_{}", strike),
                "SPY".to_string(),
                Price::from_f64(strike),
                NaiveDate::from_ymd_opt(2024, 3, 15).unwrap(),
                OptionType::Put,
                ExerciseStyle::American,
            );
            let mut put_snapshot = OptionSnapshot::new(put, chain.time);
            put_snapshot.greeks = Greek::new(-0.5, 0.04, -0.02, 0.15);
            put_snapshot.open_interest = Some(8000);
            put_snapshot.underlying_price = Some(Price::from_f64(450.0));
            chain.add_contract(put_snapshot);
        }

        chain
    }

    #[test]
    fn test_calculate_profile() {
        let service = GexCalculationService::new();
        let chain = create_test_chain_with_gamma();
        let profile = service.calculate_profile(&chain).expect("Should calculate GEX with underlying price");

        assert!(profile.has_data());
        assert_eq!(profile.underlying_ticker, "SPY");
        assert!(!profile.exposures.is_empty());

        // Check call and put walls are identified
        assert!(profile.call_wall.is_some());
        assert!(profile.put_wall.is_some());
    }

    #[test]
    fn test_find_zero_gamma_level() {
        let service = GexCalculationService::new();
        let chain = create_test_chain_with_gamma();
        let profile = service.calculate_profile(&chain).expect("Should calculate GEX");

        // All strikes have identical gamma/OI, so net_gamma is the same negative value
        // at every strike. No sign change means no zero-gamma crossing.
        let zero_level = service.find_zero_gamma_level(&profile);
        assert!(
            zero_level.is_none(),
            "Symmetric chain with uniform negative net gamma should have no zero-gamma crossing"
        );
    }

    #[test]
    fn test_calculate_expected_range() {
        let service = GexCalculationService::new();
        let chain = create_test_chain_with_gamma();
        let profile = service.calculate_profile(&chain).expect("Should calculate GEX");

        if let Some((support, resistance)) = service.calculate_expected_range(&profile) {
            assert!(support < resistance);
        }
    }

    #[test]
    fn test_market_regime() {
        let service = GexCalculationService::new();
        let chain = create_test_chain_with_gamma();
        let profile = service.calculate_profile(&chain).expect("Should calculate GEX");

        // total_net_gamma = 5 strikes * (-166,050,000) = -830,250,000
        // This is < -1,000,000 so the regime is "Negative Gamma"
        let regime = service.determine_market_regime(&profile);
        assert_eq!(
            regime, "Negative Gamma",
            "Dealer-short chain with large negative net gamma should be Negative Gamma regime"
        );
    }

    #[test]
    fn test_volatility_expectation() {
        let service = GexCalculationService::new();
        let chain = create_test_chain_with_gamma();
        let profile = service.calculate_profile(&chain).expect("Should calculate GEX");

        // total_abs_gamma = 5 * 166,050,000 = 830,250,000
        // 830,250,000 > 10,000,000 so the function returns 0.2
        let vol_score = service.calculate_volatility_expectation(&profile);
        assert!(
            (vol_score - 0.2).abs() < f64::EPSILON,
            "High absolute gamma (830M > 10M threshold) should yield low volatility score of 0.2, got {}",
            vol_score
        );
    }

    #[test]
    fn test_gamma_weighted_strike() {
        let service = GexCalculationService::new();
        let chain = create_test_chain_with_gamma();
        let profile = service.calculate_profile(&chain).expect("Should calculate GEX");

        if let Some(weighted_strike) = service.gamma_weighted_strike(&profile) {
            // Should be near the underlying price (450)
            assert!(weighted_strike > 440.0 && weighted_strike < 460.0);
        }
    }

    #[test]
    fn test_analyze_gamma_skew() {
        let service = GexCalculationService::new();
        let chain = create_test_chain_with_gamma();
        let profile = service.calculate_profile(&chain).expect("Should calculate GEX");

        // Call gamma per strike: 0.05 * 10000 * 100 * 450^2 * 0.01 = 101,250,000
        // Put gamma per strike:  0.04 * 8000  * 100 * 450^2 * 0.01 = 64,800,000
        // gamma_ratio = (5 * 101,250,000) / (5 * 64,800,000) = 1.5625
        // 1.5625 > 1.5 threshold -> "Bullish"
        let (sentiment, ratio) = service.analyze_gamma_skew(&profile);
        assert_eq!(
            sentiment, "Bullish",
            "Call gamma dominance (ratio 1.5625 > 1.5) should be Bullish"
        );

        let expected_ratio = 101_250_000.0 / 64_800_000.0; // 1.5625
        assert!(
            (ratio - expected_ratio).abs() < 0.001,
            "Gamma ratio should be ~1.5625, got {}",
            ratio
        );
    }

    #[test]
    fn test_has_squeeze_potential() {
        let service = GexCalculationService::new();
        let chain = create_test_chain_with_gamma();
        let profile = service.calculate_profile(&chain).expect("Should calculate GEX");

        // Price exactly at the 450 strike: nearest exposure has total_oi=18000 (>10k),
        // total_gamma=166,050,000 (>500k), and distance is 0% (<2%), so squeeze = true
        let current_price = Price::from_f64(450.0);
        let has_squeeze = service.has_squeeze_potential(&profile, current_price);
        assert!(has_squeeze, "Should detect squeeze potential when price is at a high-gamma strike");

        // Price far from any strike (e.g. 500.0): nearest strike is 460, distance ~8% (>2%)
        let far_price = Price::from_f64(500.0);
        let no_squeeze = service.has_squeeze_potential(&profile, far_price);
        assert!(!no_squeeze, "Should not detect squeeze when price is far from strikes");
    }
}
