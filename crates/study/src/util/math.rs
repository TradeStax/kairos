//! Statistical math utilities for study computations.
//!
//! Provides [`mean`], [`variance`] (population), and [`standard_deviation`]
//! — the building blocks for indicators like Bollinger Bands and ATR.

/// Arithmetic mean of a slice. Returns `0.0` for an empty slice.
pub fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

/// Population variance (divides by N, not N-1).
///
/// This is the standard convention for technical indicators like
/// Bollinger Bands. Returns `0.0` for slices with fewer than 2 elements.
pub fn variance(values: &[f64]) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }
    let avg = mean(values);
    let sum_sq: f64 = values.iter().map(|v| (v - avg).powi(2)).sum();
    sum_sq / values.len() as f64
}

/// Population standard deviation (square root of [`variance`]).
pub fn standard_deviation(values: &[f64]) -> f64 {
    variance(values).sqrt()
}

/// Population variance given a pre-computed mean.
///
/// Avoids recomputing the mean when the caller already has it
/// (e.g. Bollinger Bands computes `mean` for the SMA, then needs
/// the standard deviation over the same window).
pub fn variance_with_mean(values: &[f64], avg: f64) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }
    let sum_sq: f64 = values.iter().map(|v| (v - avg).powi(2)).sum();
    sum_sq / values.len() as f64
}

/// Population standard deviation given a pre-computed mean.
pub fn standard_deviation_with_mean(values: &[f64], avg: f64) -> f64 {
    variance_with_mean(values, avg).sqrt()
}

/// EMA smoothing multiplier: `2 / (period + 1)`.
///
/// Used by EMA, MACD (which chains EMAs), and Bollinger Bands
/// (when configured with EMA mode).
pub fn ema_multiplier(period: usize) -> f64 {
    2.0 / (period + 1) as f64
}

/// Wilder's smoothing multiplier: `1 / period`.
///
/// Used by RSI and ATR which use Wilder's exponential smoothing
/// (equivalent to a `2×period − 1` EMA).
pub fn wilder_multiplier(period: usize) -> f64 {
    1.0 / period as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mean() {
        assert_eq!(mean(&[1.0, 2.0, 3.0, 4.0, 5.0]), 3.0);
        assert_eq!(mean(&[]), 0.0);
        assert_eq!(mean(&[42.0]), 42.0);
    }

    #[test]
    fn test_variance() {
        let vals = [2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0];
        let v = variance(&vals);
        assert!((v - 4.0).abs() < 1e-10);
    }

    #[test]
    fn test_standard_deviation() {
        let vals = [2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0];
        let sd = standard_deviation(&vals);
        assert!((sd - 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_variance_single_value() {
        assert_eq!(variance(&[5.0]), 0.0);
    }

    #[test]
    fn test_variance_with_mean() {
        let vals = [2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0];
        let avg = mean(&vals);
        let v = variance_with_mean(&vals, avg);
        assert!((v - 4.0).abs() < 1e-10);
    }

    #[test]
    fn test_standard_deviation_with_mean() {
        let vals = [2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0];
        let avg = mean(&vals);
        let sd = standard_deviation_with_mean(&vals, avg);
        assert!((sd - 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_with_mean_matches_without() {
        let vals = [1.0, 3.0, 5.0, 7.0, 9.0, 11.0];
        let avg = mean(&vals);
        let v1 = variance(&vals);
        let v2 = variance_with_mean(&vals, avg);
        assert!((v1 - v2).abs() < 1e-10);
        let sd1 = standard_deviation(&vals);
        let sd2 = standard_deviation_with_mean(&vals, avg);
        assert!((sd1 - sd2).abs() < 1e-10);
    }

    #[test]
    fn test_with_mean_edge_cases() {
        assert_eq!(variance_with_mean(&[], 0.0), 0.0);
        assert_eq!(variance_with_mean(&[5.0], 5.0), 0.0);
        assert_eq!(standard_deviation_with_mean(&[], 0.0), 0.0);
    }

    #[test]
    fn test_ema_multiplier() {
        // Standard EMA: 2 / (period + 1)
        assert!((ema_multiplier(10) - 2.0 / 11.0).abs() < 1e-10);
        assert!((ema_multiplier(20) - 2.0 / 21.0).abs() < 1e-10);
        assert!((ema_multiplier(1) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_wilder_multiplier() {
        // Wilder's smoothing: 1 / period
        assert!((wilder_multiplier(14) - 1.0 / 14.0).abs() < 1e-10);
        assert!((wilder_multiplier(1) - 1.0).abs() < 1e-10);
    }
}
