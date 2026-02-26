/// T-test: test whether the mean of daily returns is significantly
/// different from zero.
///
/// Returns (t_statistic, p_value_approx).
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
    // (valid for large n; for small n, use Student-t)
    let p = 2.0 * (1.0 - approx_normal_cdf(t.abs()));
    (t, p)
}

/// Bootstrap confidence interval for a metric.
///
/// Resamples `values` with replacement `iterations` times,
/// computes `metric_fn` on each resample, and returns
/// (lower_bound, upper_bound) at the given confidence level
/// (e.g. 0.95 for 95% CI).
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

    // Simple LCG for deterministic resampling
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

/// Approximate standard normal CDF using Abramowitz & Stegun.
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
