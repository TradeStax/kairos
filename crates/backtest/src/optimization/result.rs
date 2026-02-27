//! Result types for walk-forward optimization.
//!
//! Stores per-window and aggregate outcomes so the caller can
//! inspect in-sample vs out-of-sample performance and detect
//! overfitting.

use crate::optimization::objective::ObjectiveFunction;
use crate::output::metrics::PerformanceMetrics;
use kairos_study::ParameterValue;
use std::collections::HashMap;

/// Performance result for a single walk-forward window.
#[derive(Debug, Clone)]
pub struct WindowResult {
    /// Metrics computed on the in-sample (training) period using
    /// the best parameter set.
    pub in_sample_metrics: PerformanceMetrics,
    /// Metrics computed on the out-of-sample (validation) period
    /// using the same parameter set selected in-sample.
    pub out_of_sample_metrics: PerformanceMetrics,
    /// The parameter combination that scored highest in-sample.
    pub best_params: HashMap<String, ParameterValue>,
    /// Objective value achieved on the in-sample period.
    pub in_sample_objective: f64,
    /// Objective value achieved on the out-of-sample period.
    pub out_of_sample_objective: f64,
}

/// Aggregate result across all walk-forward windows.
///
/// Provides summary statistics to evaluate whether the strategy
/// generalizes beyond its training data or is overfit.
#[derive(Debug, Clone)]
pub struct WalkForwardResult {
    /// The objective function that was maximized.
    pub objective: ObjectiveFunction,
    /// Per-window results in chronological order.
    pub windows: Vec<WindowResult>,
    /// Mean out-of-sample objective value across all windows.
    pub aggregate_oos_objective: f64,
    /// Total out-of-sample net PnL (USD) summed across windows.
    pub aggregate_oos_pnl: f64,
    /// Number of parameter combinations tested in each window.
    pub combinations_per_window: usize,
}

impl WalkForwardResult {
    /// Computes aggregate statistics from individual window
    /// results.
    ///
    /// The aggregate out-of-sample objective is the arithmetic
    /// mean across windows, and the aggregate PnL is the sum.
    #[must_use]
    pub fn compute_aggregate(
        windows: Vec<WindowResult>,
        objective: ObjectiveFunction,
        combinations: usize,
    ) -> Self {
        let n = windows.len().max(1) as f64;
        let aggregate_oos_objective = windows
            .iter()
            .map(|w| w.out_of_sample_objective)
            .sum::<f64>()
            / n;
        let aggregate_oos_pnl = windows
            .iter()
            .map(|w| w.out_of_sample_metrics.net_pnl_usd)
            .sum();
        Self {
            objective,
            windows,
            aggregate_oos_objective,
            aggregate_oos_pnl,
            combinations_per_window: combinations,
        }
    }
}
