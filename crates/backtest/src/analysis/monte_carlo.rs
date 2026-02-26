use crate::output::trade_record::TradeRecord;

/// Results of a Monte Carlo simulation.
#[derive(Debug, Clone)]
pub struct MonteCarloResult {
    /// Final equity for each simulation run.
    pub final_equities: Vec<f64>,
    /// Max drawdown for each simulation run.
    pub max_drawdowns: Vec<f64>,
    /// Percentile values for final equity.
    pub equity_percentiles: Percentiles,
    /// Percentile values for max drawdown.
    pub drawdown_percentiles: Percentiles,
    /// Probability of ending with a loss.
    pub probability_of_loss: f64,
}

#[derive(Debug, Clone)]
pub struct Percentiles {
    pub p5: f64,
    pub p25: f64,
    pub p50: f64,
    pub p75: f64,
    pub p95: f64,
}

/// Monte Carlo simulator that resamples trades with replacement.
pub struct MonteCarloSimulator {
    iterations: usize,
    seed: u64,
}

impl MonteCarloSimulator {
    pub fn new(iterations: usize) -> Self {
        Self {
            iterations,
            seed: 42,
        }
    }

    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }

    /// Run Monte Carlo simulation on completed trades.
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
    let idx_95 = ((0.95 * n as f64) as usize).min(n - 1);
    Percentiles {
        p5: data[(0.05 * n as f64) as usize],
        p25: data[(0.25 * n as f64) as usize],
        p50: data[(0.50 * n as f64) as usize],
        p75: data[(0.75 * n as f64) as usize],
        p95: data[idx_95],
    }
}
