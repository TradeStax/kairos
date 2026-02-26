//! Math utilities for study calculations.

/// Calculate the mean of a slice of f64 values.
pub fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

/// Calculate the population variance (N divisor) of a slice of f64 values.
///
/// Uses population variance (divides by N, not N-1) which is the standard
/// convention for technical indicators like Bollinger Bands and standard
/// deviation calculations in financial charting.
pub fn variance(values: &[f64]) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }
    let avg = mean(values);
    let sum_sq: f64 = values.iter().map(|v| (v - avg).powi(2)).sum();
    sum_sq / values.len() as f64
}

/// Calculate the standard deviation of a slice of f64 values.
pub fn standard_deviation(values: &[f64]) -> f64 {
    variance(values).sqrt()
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
}
