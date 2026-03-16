//! Empirical distribution for extension ratio analysis.

use crate::util::math;

/// Common interface for IVB distribution types (weighted and
/// unweighted). Allows `mod.rs` to use either interchangeably.
pub trait IvbDistribution {
    fn protection(&self) -> f64;
    fn average(&self) -> f64;
    fn raw_mean(&self) -> f64;
    fn projection(&self) -> f64;
    fn percentile(&self, p: f64) -> f64;
    fn sample_count(&self) -> usize;
}

impl IvbDistribution for EmpiricalDistribution {
    fn protection(&self) -> f64 {
        self.protection()
    }
    fn average(&self) -> f64 {
        self.average()
    }
    fn raw_mean(&self) -> f64 {
        self.raw_mean()
    }
    fn projection(&self) -> f64 {
        self.projection()
    }
    fn percentile(&self, p: f64) -> f64 {
        self.percentile(p)
    }
    fn sample_count(&self) -> usize {
        self.sample_count()
    }
}

impl IvbDistribution for WeightedEmpiricalDistribution {
    fn protection(&self) -> f64 {
        self.protection()
    }
    fn average(&self) -> f64 {
        self.average()
    }
    fn raw_mean(&self) -> f64 {
        self.raw_mean()
    }
    fn projection(&self) -> f64 {
        self.projection()
    }
    fn percentile(&self, p: f64) -> f64 {
        self.percentile(p)
    }
    fn sample_count(&self) -> usize {
        self.sample_count()
    }
}

/// Empirical distribution built from historical extension ratios.
#[derive(Debug, Clone)]
pub struct EmpiricalDistribution {
    sorted_samples: Vec<f64>,
    mean: f64,
    trimmed_mean: f64,
    sample_std_dev: f64,
}

impl EmpiricalDistribution {
    /// Build from a slice of extension ratios, filtering out
    /// values below `min_extension`. Returns `None` if empty
    /// after filtering.
    pub fn from_ratios(ratios: &[f64], min_extension: f64) -> Option<Self> {
        let filtered: Vec<f64> = ratios
            .iter()
            .copied()
            .filter(|&r| r >= min_extension)
            .collect();
        if filtered.is_empty() {
            return None;
        }
        let mut sorted = filtered.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let mean = math::mean(&filtered);
        let trimmed_mean = math::trimmed_mean(&filtered, 0.1);
        let sample_std_dev = math::sample_standard_deviation(&filtered);
        Some(Self {
            sorted_samples: sorted,
            mean,
            trimmed_mean,
            sample_std_dev,
        })
    }

    /// Median (p50) — the "protection" level.
    pub fn protection(&self) -> f64 {
        math::percentile(&self.sorted_samples, 0.5)
    }

    /// Trimmed mean — the "average" level (robust to outliers).
    pub fn average(&self) -> f64 {
        self.trimmed_mean
    }

    /// Raw (untrimmed) mean for comparison/testing.
    pub fn raw_mean(&self) -> f64 {
        self.mean
    }

    /// Trimmed mean + sample std dev — the "projection" level.
    pub fn projection(&self) -> f64 {
        self.trimmed_mean + self.sample_std_dev
    }

    /// Arbitrary percentile (p in 0.0..1.0).
    pub fn percentile(&self, p: f64) -> f64 {
        math::percentile(&self.sorted_samples, p)
    }

    /// Number of samples.
    pub fn sample_count(&self) -> usize {
        self.sorted_samples.len()
    }

    /// Fraction of samples >= threshold.
    #[allow(dead_code)]
    pub fn hit_rate_above(&self, threshold: f64) -> f64 {
        let count = self
            .sorted_samples
            .iter()
            .filter(|&&v| v >= threshold)
            .count();
        count as f64 / self.sorted_samples.len() as f64
    }
}

// ── Weighted empirical distribution ─────────────────────────

/// Empirical distribution with per-sample weights (e.g.
/// exponential recency decay). Provides the same output API as
/// `EmpiricalDistribution`.
#[derive(Debug, Clone)]
pub struct WeightedEmpiricalDistribution {
    /// (value, normalized_weight) sorted by value.
    sorted_entries: Vec<(f64, f64)>,
    weighted_mean: f64,
    weighted_trimmed_mean: f64,
    weighted_sample_std_dev: f64,
    count: usize,
}

impl WeightedEmpiricalDistribution {
    /// Build from (value, raw_weight) pairs, filtering values
    /// below `min_extension`. Returns `None` if empty after
    /// filtering.
    pub fn from_weighted_ratios(entries: &[(f64, f64)], min_extension: f64) -> Option<Self> {
        let filtered: Vec<(f64, f64)> = entries
            .iter()
            .copied()
            .filter(|&(v, _)| v >= min_extension)
            .collect();
        if filtered.is_empty() {
            return None;
        }

        let count = filtered.len();

        // Normalize weights
        let w_sum: f64 = filtered.iter().map(|(_, w)| w).sum();
        if w_sum <= 0.0 {
            return None;
        }
        let mut sorted: Vec<(f64, f64)> = filtered.iter().map(|&(v, w)| (v, w / w_sum)).collect();
        sorted.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

        let wm = w_mean(&sorted);
        let wtm = w_trimmed_mean(&sorted, 0.1);
        let wsd = w_sample_std_dev(&sorted, wm);

        Some(Self {
            sorted_entries: sorted,
            weighted_mean: wm,
            weighted_trimmed_mean: wtm,
            weighted_sample_std_dev: wsd,
            count,
        })
    }

    /// Weighted median (p50) — the "protection" level.
    pub fn protection(&self) -> f64 {
        w_percentile(&self.sorted_entries, 0.5)
    }

    /// Weighted trimmed mean — the "average" level.
    pub fn average(&self) -> f64 {
        self.weighted_trimmed_mean
    }

    /// Raw weighted mean.
    pub fn raw_mean(&self) -> f64 {
        self.weighted_mean
    }

    /// Weighted trimmed mean + weighted std dev.
    pub fn projection(&self) -> f64 {
        self.weighted_trimmed_mean + self.weighted_sample_std_dev
    }

    /// Arbitrary weighted percentile (p in 0.0..1.0).
    pub fn percentile(&self, p: f64) -> f64 {
        w_percentile(&self.sorted_entries, p)
    }

    /// Number of samples (unweighted count).
    pub fn sample_count(&self) -> usize {
        self.count
    }
}

/// Compute exponential decay weights for `count` items.
///
/// `decay_rate`: higher = more aggressive recency bias.
/// `newest_first`: if true, index 0 is newest (age=0).
/// Returns raw (un-normalized) weights.
pub fn exponential_weights(count: usize, decay_rate: f64, newest_first: bool) -> Vec<f64> {
    (0..count)
        .map(|i| {
            let age = if newest_first {
                i as f64
            } else {
                (count - 1 - i) as f64
            };
            (-decay_rate * age).exp()
        })
        .collect()
}

// ── Private weighted stat helpers ───────────────────────────

/// Weighted mean. Entries must have normalized weights (sum ≈ 1).
fn w_mean(entries: &[(f64, f64)]) -> f64 {
    entries.iter().map(|&(v, w)| w * v).sum()
}

/// Weighted percentile via cumulative weight interpolation.
/// `entries` must be sorted by value with normalized weights.
fn w_percentile(entries: &[(f64, f64)], p: f64) -> f64 {
    if entries.is_empty() {
        return 0.0;
    }
    if entries.len() == 1 {
        return entries[0].0;
    }
    let p = p.clamp(0.0, 1.0);

    // Build cumulative weight midpoints
    let mut cum = 0.0;
    let midpoints: Vec<(f64, f64)> = entries
        .iter()
        .map(|&(v, w)| {
            let mid = cum + w / 2.0;
            cum += w;
            (v, mid)
        })
        .collect();

    // Clamp to range
    if p <= midpoints[0].1 {
        return midpoints[0].0;
    }
    let last = midpoints.len() - 1;
    if p >= midpoints[last].1 {
        return midpoints[last].0;
    }

    // Linear interpolation between surrounding midpoints
    for i in 0..last {
        let (v0, m0) = midpoints[i];
        let (v1, m1) = midpoints[i + 1];
        if p >= m0 && p <= m1 {
            let frac = if (m1 - m0).abs() < f64::EPSILON {
                0.0
            } else {
                (p - m0) / (m1 - m0)
            };
            return v0 + frac * (v1 - v0);
        }
    }

    midpoints[last].0
}

/// Weighted trimmed mean: trim by weight mass, not count.
/// `entries` must be sorted by value with normalized weights.
fn w_trimmed_mean(entries: &[(f64, f64)], trim_fraction: f64) -> f64 {
    if entries.is_empty() {
        return 0.0;
    }
    let lo = trim_fraction;
    let hi = 1.0 - trim_fraction;

    let mut cum = 0.0;
    let mut sum = 0.0;
    let mut w_sum = 0.0;

    for &(v, w) in entries {
        let prev_cum = cum;
        cum += w;
        // Portion of this entry within [lo, hi]
        let start = prev_cum.max(lo);
        let end = cum.min(hi);
        if end > start {
            let portion = end - start;
            sum += v * portion;
            w_sum += portion;
        }
    }

    if w_sum <= 0.0 {
        w_mean(entries)
    } else {
        sum / w_sum
    }
}

/// Weighted sample std dev using reliability weights formula:
/// `sqrt( Σ(w·(x-μ)²) / (Σw - Σw²/Σw) )`
/// Entries must have normalized weights (Σw = 1).
fn w_sample_std_dev(entries: &[(f64, f64)], mean: f64) -> f64 {
    if entries.len() < 2 {
        return 0.0;
    }
    let w_sum: f64 = entries.iter().map(|(_, w)| w).sum();
    let w2_sum: f64 = entries.iter().map(|(_, w)| w * w).sum();
    let denom = w_sum - w2_sum / w_sum;
    if denom <= 0.0 {
        return 0.0;
    }
    let var: f64 = entries.iter().map(|&(v, w)| w * (v - mean).powi(2)).sum();
    (var / denom).sqrt()
}
