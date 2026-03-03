//! Monte Carlo simulation for trade-sequence robustness analysis.
//!
//! Shuffles the order of completed trades to estimate the distribution
//! of possible equity outcomes and drawdowns, quantifying the role of
//! luck versus edge in a strategy's historical performance.

use crate::output::trade_record::TradeRecord;

/// Aggregate results from a Monte Carlo simulation run.
///
/// Contains the full distribution of final equities and maximum
/// drawdowns across all iterations, along with summary percentiles
/// and the estimated probability of ending at a loss.
#[derive(Debug, Clone)]
pub struct MonteCarloResult {
    /// Final equity value for each simulation iteration.
    pub final_equities: Vec<f64>,
    /// Maximum drawdown percentage for each simulation iteration.
    pub max_drawdowns: Vec<f64>,
    /// Key percentile values of the final equity distribution.
    pub equity_percentiles: Percentiles,
    /// Key percentile values of the max drawdown distribution.
    pub drawdown_percentiles: Percentiles,
    /// Fraction of iterations that ended below the initial capital
    /// (range 0.0 to 1.0).
    pub probability_of_loss: f64,
}

/// Standard percentile breakpoints for a distribution.
///
/// Stores the 5th, 25th, 50th (median), 75th, and 95th percentile
/// values, providing a concise summary of a distribution's shape
/// and spread.
#[derive(Debug, Clone, Copy)]
pub struct Percentiles {
    /// 5th percentile (lower tail).
    pub p5: f64,
    /// 25th percentile (first quartile).
    pub p25: f64,
    /// 50th percentile (median).
    pub p50: f64,
    /// 75th percentile (third quartile).
    pub p75: f64,
    /// 95th percentile (upper tail).
    pub p95: f64,
}

/// Monte Carlo simulator that resamples completed trades with
/// replacement to produce a distribution of possible equity paths.
///
/// Each iteration draws `N` trades (where `N` is the original trade
/// count) uniformly at random with replacement, replays them
/// sequentially against the initial capital, and records the final
/// equity and peak-to-trough drawdown.
///
/// Uses a deterministic linear congruential generator (LCG) so
/// results are reproducible for a given seed.
pub struct MonteCarloSimulator {
    /// Number of simulation iterations to run.
    iterations: usize,
    /// Seed for the deterministic pseudo-random number generator.
    seed: u64,
}

impl MonteCarloSimulator {
    /// Creates a new simulator with the given number of iterations.
    ///
    /// Uses a default seed of 42 for reproducibility.
    #[must_use]
    pub fn new(iterations: usize) -> Self {
        Self {
            iterations,
            seed: 42,
        }
    }

    /// Sets a custom PRNG seed for the simulation.
    #[must_use]
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }

    /// Runs the Monte Carlo simulation on a set of completed trades.
    ///
    /// For each iteration, draws `trades.len()` trades uniformly at
    /// random with replacement, replays them against
    /// `initial_capital`, and records the final equity and maximum
    /// percentage drawdown.
    ///
    /// Returns a [`MonteCarloResult`] containing the full
    /// distributions and summary statistics. If `trades` is empty,
    /// returns a degenerate result with no variation.
    #[must_use]
    pub fn simulate(&self, trades: &[TradeRecord], initial_capital: f64) -> MonteCarloResult {
        if trades.is_empty() {
            return MonteCarloResult {
                final_equities: vec![initial_capital],
                max_drawdowns: vec![0.0],
                equity_percentiles: Percentiles {
                    p5: initial_capital,
                    p25: initial_capital,
                    p50: initial_capital,
                    p75: initial_capital,
                    p95: initial_capital,
                },
                drawdown_percentiles: Percentiles {
                    p5: 0.0,
                    p25: 0.0,
                    p50: 0.0,
                    p75: 0.0,
                    p95: 0.0,
                },
                probability_of_loss: 0.0,
            };
        }

        let n = trades.len();
        let mut final_equities = Vec::with_capacity(self.iterations);
        let mut max_drawdowns = Vec::with_capacity(self.iterations);
        let mut rng_state = self.seed;

        for _ in 0..self.iterations {
            let mut equity = initial_capital;
            let mut peak = initial_capital;
            let mut max_dd = 0.0_f64;

            for _ in 0..n {
                rng_state = rng_state
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                let idx = (rng_state >> 33) as usize % n;
                equity += trades[idx].pnl_net_usd;
                if equity > peak {
                    peak = equity;
                }
                let dd = if peak > 0.0 {
                    (peak - equity) / peak * 100.0
                } else {
                    0.0
                };
                max_dd = max_dd.max(dd);
            }

            final_equities.push(equity);
            max_drawdowns.push(max_dd);
        }

        let probability_of_loss = final_equities
            .iter()
            .filter(|e| **e < initial_capital)
            .count() as f64
            / self.iterations as f64;

        let equity_percentiles = compute_percentiles(&mut final_equities);
        let drawdown_percentiles = compute_percentiles(&mut max_drawdowns);

        MonteCarloResult {
            final_equities,
            max_drawdowns,
            equity_percentiles,
            drawdown_percentiles,
            probability_of_loss,
        }
    }
}

/// Computes the standard percentile breakpoints from a mutable slice.
///
/// Sorts `data` in ascending order and extracts the 5th, 25th, 50th,
/// 75th, and 95th percentile values using nearest-rank interpolation.
///
/// Returns all-zero percentiles if `data` is empty.
fn compute_percentiles(data: &mut [f64]) -> Percentiles {
    data.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = data.len();
    if n == 0 {
        return Percentiles {
            p5: 0.0,
            p25: 0.0,
            p50: 0.0,
            p75: 0.0,
            p95: 0.0,
        };
    }
    Percentiles {
        p5: data[(0.05 * n as f64) as usize],
        p25: data[(0.25 * n as f64) as usize],
        p50: data[(0.50 * n as f64) as usize],
        p75: data[(0.75 * n as f64) as usize],
        p95: data[((0.95 * n as f64) as usize).min(n - 1)],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::trade_record::{ExitReason, TradeRecord};
    use kairos_data::{Price, Side, Timestamp};

    fn make_trade_record(pnl_net: f64) -> TradeRecord {
        TradeRecord {
            index: 1,
            entry_time: Timestamp(1000),
            exit_time: Timestamp(2000),
            side: Side::Buy,
            quantity: 1.0,
            entry_price: Price::from_f64(5000.0),
            exit_price: Price::from_f64(5010.0),
            initial_stop_loss: Price::from_f64(4990.0),
            initial_take_profit: None,
            pnl_ticks: 0,
            pnl_gross_usd: pnl_net,
            commission_usd: 0.0,
            pnl_net_usd: pnl_net,
            rr_ratio: 0.0,
            mae_ticks: 0,
            mfe_ticks: 0,
            exit_reason: ExitReason::Manual,
            label: None,
            instrument: None,
            duration_ms: Some(1000),
        }
    }

    // ── Empty trades ─────────────────────────────────────────────

    #[test]
    fn test_empty_trades() {
        let sim = MonteCarloSimulator::new(100);
        let result = sim.simulate(&[], 100_000.0);

        assert_eq!(result.final_equities.len(), 1);
        assert!((result.final_equities[0] - 100_000.0).abs() < 1e-10);
        assert!((result.probability_of_loss - 0.0).abs() < 1e-10);
    }

    // ── Deterministic with seed ──────────────────────────────────

    #[test]
    fn test_deterministic_same_seed() {
        let trades = vec![
            make_trade_record(100.0),
            make_trade_record(-50.0),
            make_trade_record(200.0),
            make_trade_record(-30.0),
        ];

        let sim = MonteCarloSimulator::new(500).with_seed(42);
        let r1 = sim.simulate(&trades, 100_000.0);

        let sim2 = MonteCarloSimulator::new(500).with_seed(42);
        let r2 = sim2.simulate(&trades, 100_000.0);

        assert_eq!(r1.final_equities, r2.final_equities);
        assert_eq!(r1.max_drawdowns, r2.max_drawdowns);
    }

    #[test]
    fn test_different_seed_different_results() {
        let trades = vec![
            make_trade_record(100.0),
            make_trade_record(-50.0),
            make_trade_record(200.0),
        ];

        let r1 = MonteCarloSimulator::new(100)
            .with_seed(42)
            .simulate(&trades, 100_000.0);
        let r2 = MonteCarloSimulator::new(100)
            .with_seed(99)
            .simulate(&trades, 100_000.0);

        // Very unlikely to be exactly equal with different seeds
        assert_ne!(r1.final_equities, r2.final_equities);
    }

    // ── All winning trades ───────────────────────────────────────

    #[test]
    fn test_all_winning_zero_probability_of_loss() {
        let trades = vec![
            make_trade_record(100.0),
            make_trade_record(200.0),
            make_trade_record(150.0),
        ];

        let sim = MonteCarloSimulator::new(1000);
        let result = sim.simulate(&trades, 100_000.0);

        // All trades are winners, so no iteration should end below initial
        assert!((result.probability_of_loss - 0.0).abs() < 1e-10);
        // All final equities should be > initial capital
        for &eq in &result.final_equities {
            assert!(eq > 100_000.0);
        }
    }

    // ── All losing trades ────────────────────────────────────────

    #[test]
    fn test_all_losing_full_probability_of_loss() {
        let trades = vec![
            make_trade_record(-100.0),
            make_trade_record(-200.0),
            make_trade_record(-150.0),
        ];

        let sim = MonteCarloSimulator::new(1000);
        let result = sim.simulate(&trades, 100_000.0);

        // Every iteration should end below initial capital
        assert!((result.probability_of_loss - 1.0).abs() < 1e-10);
    }

    // ── Iteration count ──────────────────────────────────────────

    #[test]
    fn test_iteration_count_matches() {
        let trades = vec![make_trade_record(50.0), make_trade_record(-30.0)];

        let sim = MonteCarloSimulator::new(500);
        let result = sim.simulate(&trades, 100_000.0);

        assert_eq!(result.final_equities.len(), 500);
        assert_eq!(result.max_drawdowns.len(), 500);
    }

    // ── Percentile ordering ──────────────────────────────────────

    #[test]
    fn test_percentiles_are_ordered() {
        let trades = vec![
            make_trade_record(100.0),
            make_trade_record(-80.0),
            make_trade_record(200.0),
            make_trade_record(-50.0),
            make_trade_record(150.0),
        ];

        let sim = MonteCarloSimulator::new(2000);
        let result = sim.simulate(&trades, 100_000.0);

        let ep = &result.equity_percentiles;
        assert!(ep.p5 <= ep.p25);
        assert!(ep.p25 <= ep.p50);
        assert!(ep.p50 <= ep.p75);
        assert!(ep.p75 <= ep.p95);

        let dp = &result.drawdown_percentiles;
        assert!(dp.p5 <= dp.p25);
        assert!(dp.p25 <= dp.p50);
        assert!(dp.p50 <= dp.p75);
        assert!(dp.p75 <= dp.p95);
    }

    // ── Max drawdown always non-negative ─────────────────────────

    #[test]
    fn test_max_drawdowns_non_negative() {
        let trades = vec![
            make_trade_record(100.0),
            make_trade_record(-80.0),
            make_trade_record(50.0),
        ];

        let sim = MonteCarloSimulator::new(500);
        let result = sim.simulate(&trades, 100_000.0);

        for &dd in &result.max_drawdowns {
            assert!(dd >= 0.0, "Drawdown should be non-negative, got {dd}");
        }
    }

    // ── Single trade ─────────────────────────────────────────────

    #[test]
    fn test_single_trade() {
        let trades = vec![make_trade_record(500.0)];

        let sim = MonteCarloSimulator::new(100);
        let result = sim.simulate(&trades, 100_000.0);

        // Every iteration resamples exactly 1 trade (always the same one)
        for &eq in &result.final_equities {
            assert!((eq - 100_500.0).abs() < 1e-10);
        }
        assert!((result.probability_of_loss - 0.0).abs() < 1e-10);
    }

    // ── compute_percentiles helper ───────────────────────────────

    #[test]
    fn test_compute_percentiles_empty() {
        let pct = compute_percentiles(&mut []);
        assert!((pct.p50 - 0.0).abs() < 1e-15);
    }

    #[test]
    fn test_compute_percentiles_sorted() {
        let mut data: Vec<f64> = (1..=100).map(|i| i as f64).collect();
        let pct = compute_percentiles(&mut data);

        // p50 should be around 50 (nearest-rank)
        assert!((pct.p50 - 50.0).abs() <= 1.0);
        assert!(pct.p5 < pct.p50);
        assert!(pct.p50 < pct.p95);
    }
}
