use crate::optimization::objective::ObjectiveFunction;
use crate::output::metrics::PerformanceMetrics;
use kairos_study::ParameterValue;
use std::collections::HashMap;

/// Result of a single optimization window.
#[derive(Debug, Clone)]
pub struct WindowResult {
    /// In-sample metrics.
    pub in_sample_metrics: PerformanceMetrics,
    /// Out-of-sample metrics.
    pub out_of_sample_metrics: PerformanceMetrics,
    /// Best parameter set found in-sample.
    pub best_params: HashMap<String, ParameterValue>,
    /// Objective value achieved in-sample.
    pub in_sample_objective: f64,
    /// Objective value achieved out-of-sample.
    pub out_of_sample_objective: f64,
}

/// Aggregate result of walk-forward optimization.
#[derive(Debug, Clone)]
pub struct WalkForwardResult {
    pub objective: ObjectiveFunction,
    pub windows: Vec<WindowResult>,
    /// Aggregate out-of-sample objective
    /// (mean across windows).
    pub aggregate_oos_objective: f64,
    /// Aggregate out-of-sample net PnL.
    pub aggregate_oos_pnl: f64,
    /// Total parameter combinations tested per window.
    pub combinations_per_window: usize,
}

impl WalkForwardResult {
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
