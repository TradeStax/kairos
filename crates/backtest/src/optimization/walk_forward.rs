//! Walk-forward optimization with rolling in-sample / out-of-sample
//! window splits.
//!
//! Walk-forward analysis divides the historical date range into
//! consecutive windows, each containing an in-sample period for
//! parameter fitting and an out-of-sample period for validation.
//! This guards against curve-fitting by ensuring the strategy is
//! always evaluated on unseen data.

use crate::optimization::objective::ObjectiveFunction;
use crate::optimization::parameter_space::ParameterGrid;
use kairos_data::DateRange;

/// Configuration for a walk-forward optimization run.
pub struct WalkForwardConfig {
    /// Full calendar date range to partition into windows.
    pub date_range: DateRange,
    /// Fraction of each window allocated to in-sample training
    /// (e.g. 0.7 means 70% in-sample, 30% out-of-sample).
    pub in_sample_ratio: f64,
    /// Number of rolling windows to create.
    pub num_windows: usize,
    /// Objective function used to select the best parameters
    /// in each in-sample period.
    pub objective: ObjectiveFunction,
    /// Parameter grid to search exhaustively within each window.
    pub grid: ParameterGrid,
}

/// A single time window consisting of an in-sample training period
/// and a contiguous out-of-sample validation period.
#[derive(Debug, Clone)]
pub struct TimeWindow {
    /// Date range used for parameter optimization (training).
    pub in_sample: DateRange,
    /// Date range used for unbiased performance evaluation.
    pub out_of_sample: DateRange,
}

/// Splits a date range into rolling walk-forward windows.
///
/// The optimizer itself does not run backtests — it only partitions
/// the time axis. The caller is responsible for executing backtests
/// on each window's in-sample and out-of-sample periods.
pub struct WalkForwardOptimizer;

impl WalkForwardOptimizer {
    /// Divides `date_range` into `num_windows` consecutive
    /// walk-forward windows.
    ///
    /// Each window has length `total_days / num_windows`. Within
    /// each window, the first `in_sample_ratio` fraction is
    /// assigned to in-sample and the remainder to out-of-sample.
    ///
    /// Returns an empty vec if:
    /// - `num_windows` is 0
    /// - the total span is less than 2 days
    /// - per-window size is less than 2 days
    /// - the out-of-sample portion would be 0 days
    #[must_use]
    pub fn split_windows(
        date_range: &DateRange,
        num_windows: usize,
        in_sample_ratio: f64,
    ) -> Vec<TimeWindow> {
        if num_windows == 0 {
            return vec![];
        }

        let total_days = date_range.num_days() as usize;
        if total_days < 2 {
            return vec![];
        }

        let window_size = total_days / num_windows;
        if window_size < 2 {
            return vec![];
        }

        let is_days = (window_size as f64 * in_sample_ratio) as usize;
        let is_days = is_days.max(1);
        let oos_days = window_size - is_days;
        if oos_days == 0 {
            return vec![];
        }

        let mut windows = Vec::with_capacity(num_windows);
        let start = date_range.start;

        for i in 0..num_windows {
            let offset = i * window_size;
            let is_start = start + chrono::Duration::days(offset as i64);
            let is_end = is_start + chrono::Duration::days(is_days as i64 - 1);
            let oos_start = is_end + chrono::Duration::days(1);
            let oos_end = oos_start + chrono::Duration::days(oos_days as i64 - 1);

            // Clamp to the overall date range
            let oos_end = oos_end.min(date_range.end);

            windows.push(TimeWindow {
                in_sample: DateRange {
                    start: is_start,
                    end: is_end,
                },
                out_of_sample: DateRange {
                    start: oos_start,
                    end: oos_end,
                },
            });
        }

        windows
    }
}
