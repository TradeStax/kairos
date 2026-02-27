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
