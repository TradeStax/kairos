use crate::optimization::objective::ObjectiveFunction;
use crate::optimization::parameter_space::ParameterGrid;
use kairos_data::DateRange;

/// Configuration for walk-forward optimization.
pub struct WalkForwardConfig {
    /// Full date range to optimize over.
    pub date_range: DateRange,
    /// Fraction of each window used for in-sample
    /// (e.g. 0.7 = 70%).
    pub in_sample_ratio: f64,
    /// Number of windows.
    pub num_windows: usize,
    /// Objective function to maximize.
    pub objective: ObjectiveFunction,
    /// Parameter grid to search.
    pub grid: ParameterGrid,
}

/// A single time window (in-sample + out-of-sample
/// date ranges).
#[derive(Debug, Clone)]
pub struct TimeWindow {
    pub in_sample: DateRange,
    pub out_of_sample: DateRange,
}

/// Walk-forward optimizer that splits date ranges into
/// rolling in-sample / out-of-sample windows.
pub struct WalkForwardOptimizer;

impl WalkForwardOptimizer {
    /// Split the full date range into walk-forward windows.
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

            // Don't exceed the overall date range
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
