//! Classical statistical tests for evaluating trading strategy returns.
//!
//! Contains a one-sample t-test for mean returns and a bootstrap
//! confidence interval estimator, both commonly used to determine
//! whether observed backtest performance is statistically significant.

/// Performs a one-sample t-test on daily returns against a null
/// hypothesis of zero mean.
///
/// Tests whether the mean of `daily_returns` is significantly
/// different from zero using a two-sided test. For large sample
/// sizes the normal approximation is used; for small samples a
/// Student-t distribution would be more precise.
///
/// Returns `(t_statistic, p_value)`. A low p-value (e.g. < 0.05)
/// indicates the mean return is statistically different from zero.
///
/// Returns `(0.0, 1.0)` when fewer than 2 observations are provided
/// or when the standard error is effectively zero.
#[must_use]
pub fn t_test_mean_returns(daily_returns: &[f64]) -> (f64, f64) {
    let n = daily_returns.len();
    if n < 2 {
        return (0.0, 1.0);
    }
    let mean = daily_returns.iter().sum::<f64>() / n as f64;
    let variance = daily_returns
        .iter()
        .map(|r| (r - mean).powi(2))
        .sum::<f64>()
        / (n - 1) as f64;
    let se = (variance / n as f64).sqrt();
    if se < 1e-15 {
        return (0.0, 1.0);
    }
    let t = mean / se;
    // Approximate two-sided p-value using normal distribution
    let p = 2.0 * (1.0 - approx_normal_cdf(t.abs()));
    (t, p)
}

/// Computes a bootstrap confidence interval for a given metric.
///
/// Resamples `values` with replacement `iterations` times, applies
/// `metric_fn` to each resample, and returns `(lower, upper)` bounds
/// at the specified `confidence` level (e.g. 0.95 for a 95% CI).
///
/// Uses the percentile method: the lower bound is the
/// `(alpha/2)`-th percentile and the upper bound is the
/// `(1 - alpha/2)`-th percentile of the bootstrap distribution,
/// where `alpha = 1 - confidence`.
///
/// The resampling uses a deterministic linear congruential generator
/// (LCG) seeded at 42 for reproducibility.
///
/// Returns `(0.0, 0.0)` if `values` is empty or `iterations` is 0.
#[must_use]
pub fn bootstrap_confidence_interval<F>(
    values: &[f64],
    iterations: usize,
    confidence: f64,
    metric_fn: F,
) -> (f64, f64)
where
    F: Fn(&[f64]) -> f64,
{
    if values.is_empty() || iterations == 0 {
        return (0.0, 0.0);
    }

    let mut results: Vec<f64> = Vec::with_capacity(iterations);
    let n = values.len();

    // Deterministic LCG for reproducible resampling
    let mut rng_state: u64 = 42;

    for _ in 0..iterations {
        let mut sample: Vec<f64> = Vec::with_capacity(n);
        for _ in 0..n {
            rng_state = rng_state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let idx = (rng_state >> 33) as usize % n;
            sample.push(values[idx]);
        }
        results.push(metric_fn(&sample));
    }

    results.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let alpha = 1.0 - confidence;
    let lower_idx = ((alpha / 2.0) * results.len() as f64) as usize;
    let upper_idx = ((1.0 - alpha / 2.0) * results.len() as f64).ceil() as usize;

    let lower = results.get(lower_idx).copied().unwrap_or(0.0);
    let upper = results
        .get(upper_idx.min(results.len() - 1))
        .copied()
        .unwrap_or(0.0);

    (lower, upper)
}

/// Approximates the standard normal CDF using the Abramowitz & Stegun
/// rational polynomial (formula 26.2.17).
///
/// Accurate to approximately 1e-7. Clamps extreme inputs to avoid
/// floating-point issues for |x| > 8.
fn approx_normal_cdf(x: f64) -> f64 {
    if x < -8.0 {
        return 0.0;
    }
    if x > 8.0 {
        return 1.0;
    }
    let t = 1.0 / (1.0 + 0.2316419 * x.abs());
    let d = 0.3989422804014327; // 1/sqrt(2*pi)
    let p = d * (-x * x / 2.0).exp();
    let c = t
        * (0.319381530
            + t * (-0.356563782 + t * (1.781477937 + t * (-1.821255978 + t * 1.330274429))));
    if x >= 0.0 { 1.0 - p * c } else { p * c }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── t-test ───────────────────────────────────────────────────

    #[test]
    fn test_t_test_empty_returns_default() {
        let (t, p) = t_test_mean_returns(&[]);
        assert!((t - 0.0).abs() < 1e-15);
        assert!((p - 1.0).abs() < 1e-15);
    }

    #[test]
    fn test_t_test_single_value_returns_default() {
        let (t, p) = t_test_mean_returns(&[0.01]);
        assert!((t - 0.0).abs() < 1e-15);
        assert!((p - 1.0).abs() < 1e-15);
    }

    #[test]
    fn test_t_test_zero_variance_returns_default() {
        // All identical values => se = 0
        let (t, p) = t_test_mean_returns(&[0.01, 0.01, 0.01, 0.01]);
        assert!((t - 0.0).abs() < 1e-15);
        assert!((p - 1.0).abs() < 1e-15);
    }

    #[test]
    fn test_t_test_positive_mean() {
        // Simple data with clearly positive mean
        let data = vec![0.01, 0.02, 0.015, 0.005, 0.03, 0.01, 0.02, 0.025];
        let (t, p) = t_test_mean_returns(&data);

        // Mean is positive, so t should be positive
        assert!(t > 0.0);
        // With these values, should be statistically significant
        assert!(p < 1.0);
    }

    #[test]
    fn test_t_test_negative_mean() {
        let data = vec![-0.01, -0.02, -0.015, -0.005, -0.03, -0.01];
        let (t, p) = t_test_mean_returns(&data);

        assert!(t < 0.0);
        assert!(p < 1.0);
    }

    #[test]
    fn test_t_test_known_values() {
        // Hand-computed: data = [1, 2, 3, 4, 5]
        // mean = 3, var = 2.5, se = sqrt(2.5/5) = sqrt(0.5) = 0.7071
        // t = 3 / 0.7071 = 4.2426
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let (t, _p) = t_test_mean_returns(&data);

        let expected_t = 3.0 / (2.5_f64 / 5.0).sqrt();
        assert!((t - expected_t).abs() < 1e-10);
    }

    // ── Normal CDF approximation ─────────────────────────────────

    #[test]
    fn test_normal_cdf_at_zero() {
        let cdf = approx_normal_cdf(0.0);
        assert!((cdf - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_normal_cdf_at_large_positive() {
        let cdf = approx_normal_cdf(10.0);
        assert!((cdf - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_normal_cdf_at_large_negative() {
        let cdf = approx_normal_cdf(-10.0);
        assert!((cdf - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_normal_cdf_at_1_96() {
        // CDF(1.96) ~= 0.975
        let cdf = approx_normal_cdf(1.96);
        assert!((cdf - 0.975).abs() < 0.001);
    }

    // ── Bootstrap CI ─────────────────────────────────────────────

    #[test]
    fn test_bootstrap_empty_values() {
        let (lower, upper) = bootstrap_confidence_interval(&[], 1000, 0.95, |s| {
            s.iter().sum::<f64>() / s.len() as f64
        });
        assert!((lower - 0.0).abs() < 1e-15);
        assert!((upper - 0.0).abs() < 1e-15);
    }

    #[test]
    fn test_bootstrap_zero_iterations() {
        let (lower, upper) = bootstrap_confidence_interval(&[1.0, 2.0, 3.0], 0, 0.95, |s| {
            s.iter().sum::<f64>() / s.len() as f64
        });
        assert!((lower - 0.0).abs() < 1e-15);
        assert!((upper - 0.0).abs() < 1e-15);
    }

    #[test]
    fn test_bootstrap_deterministic() {
        // Same seed should produce same results
        let mean_fn = |s: &[f64]| s.iter().sum::<f64>() / s.len() as f64;
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];

        let (l1, u1) = bootstrap_confidence_interval(&data, 1000, 0.95, mean_fn);
        let (l2, u2) = bootstrap_confidence_interval(&data, 1000, 0.95, mean_fn);

        assert!((l1 - l2).abs() < 1e-15);
        assert!((u1 - u2).abs() < 1e-15);
    }

    #[test]
    fn test_bootstrap_ci_contains_sample_mean() {
        let data = vec![10.0, 12.0, 11.0, 13.0, 9.0, 11.0, 10.0, 12.0];
        let sample_mean = data.iter().sum::<f64>() / data.len() as f64;
        let mean_fn = |s: &[f64]| s.iter().sum::<f64>() / s.len() as f64;

        let (lower, upper) = bootstrap_confidence_interval(&data, 5000, 0.95, mean_fn);

        assert!(
            lower <= sample_mean,
            "lower={lower} > sample_mean={sample_mean}"
        );
        assert!(
            upper >= sample_mean,
            "upper={upper} < sample_mean={sample_mean}"
        );
    }

    #[test]
    fn test_bootstrap_wider_ci_at_higher_confidence() {
        let data = vec![1.0, 5.0, 2.0, 8.0, 3.0, 7.0, 4.0, 6.0];
        let mean_fn = |s: &[f64]| s.iter().sum::<f64>() / s.len() as f64;

        let (l90, u90) = bootstrap_confidence_interval(&data, 5000, 0.90, mean_fn);
        let (l99, u99) = bootstrap_confidence_interval(&data, 5000, 0.99, mean_fn);

        // 99% CI should be wider than 90% CI
        let width_90 = u90 - l90;
        let width_99 = u99 - l99;
        assert!(
            width_99 >= width_90,
            "99% CI ({width_99}) should be >= 90% CI ({width_90})"
        );
    }
}
