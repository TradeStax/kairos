//! Stochastic Oscillator.
//!
//! Measures where the current close sits within the recent high-low range.
//! The raw %K value is `100 * (Close - Lowest Low) / (Highest High - Lowest Low)`
//! over the lookback period.
//!
//! The "slow" variant (default) applies an SMA smoothing to %K before
//! computing %D (an SMA of the smoothed %K). Traders watch for %K/%D
//! crossovers and readings above 80 (overbought) or below 20 (oversold).
//!
//! Output: `StudyOutput::Lines` with two series (%K solid, %D dashed).

mod params;
#[cfg(test)]
mod tests;

use std::collections::VecDeque;

use crate::config::{LineStyleValue, StudyConfig};
use crate::core::{
    Study, StudyCapabilities, StudyCategory, StudyInput, StudyMetadata, StudyPlacement, StudyResult,
};
use crate::error::StudyError;
use crate::output::{LineSeries, StudyOutput};
use crate::studies::trend::sma::compute_sma;
use crate::util::candle_key;
use params::{DEFAULT_D_COLOR, DEFAULT_K_COLOR, make_params};

/// Stochastic oscillator with %K and %D lines.
///
/// Raw %K locates the current close within the recent high-low range
/// (0 = at the lowest low, 100 = at the highest high). The slow
/// variant applies an SMA smoothing to %K, then derives %D as an SMA
/// of the smoothed %K. Renders in a separate panel with a solid %K
/// line and a dashed %D signal line.
pub struct StochasticStudy {
    metadata: StudyMetadata,
    config: StudyConfig,
    output: StudyOutput,
    params: Vec<crate::config::ParameterDef>,
}

impl StochasticStudy {
    /// Create a new Stochastic study with standard slow-stochastic
    /// defaults: %K lookback 14, %K smoothing 3, %D period 3,
    /// overbought 80, oversold 20.
    pub fn new() -> Self {
        let params = make_params();
        let config = StudyConfig::from_params("stochastic", &params);

        Self {
            metadata: StudyMetadata {
                name: "Stochastic Oscillator".to_string(),
                category: StudyCategory::Momentum,
                placement: StudyPlacement::Panel,
                description: "Stochastic oscillator with %K and %D lines".to_string(),
                config_version: 1,
                capabilities: StudyCapabilities::default(),
            },
            config,
            output: StudyOutput::Empty,
            params,
        }
    }
}

impl Default for StochasticStudy {
    fn default() -> Self {
        Self::new()
    }
}

impl Study for StochasticStudy {
    crate::impl_study_base!("stochastic");

    fn compute(&mut self, input: &StudyInput) -> Result<StudyResult, StudyError> {
        let k_period = self.config.get_int("k_period", 14) as usize;
        let d_period = self.config.get_int("d_period", 3) as usize;
        let smooth = self.config.get_int("smooth", 3) as usize;
        let k_color = self.config.get_color("k_color", DEFAULT_K_COLOR);
        let d_color = self.config.get_color("d_color", DEFAULT_D_COLOR);

        let candles = input.candles;
        if candles.len() < k_period {
            log::debug!(
                "{}: insufficient data ({} candles, need {})",
                self.id(),
                candles.len(),
                k_period
            );
            self.output = StudyOutput::Empty;
            return Ok(StudyResult::ok());
        }

        // Step 1: Compute raw %K (fast stochastic) using O(N)
        // monotonic deques for sliding-window min/max.
        let highs: Vec<f32> = candles.iter().map(|c| c.high.to_f32()).collect();
        let lows: Vec<f32> = candles.iter().map(|c| c.low.to_f32()).collect();

        let mut raw_k = Vec::with_capacity(candles.len() - k_period + 1);

        // max_deque: indices of decreasing highs (front = max)
        let mut max_deque: VecDeque<usize> = VecDeque::new();
        // min_deque: indices of increasing lows (front = min)
        let mut min_deque: VecDeque<usize> = VecDeque::new();

        for i in 0..candles.len() {
            // Maintain max deque (decreasing highs)
            while max_deque.back().is_some_and(|&j| highs[j] <= highs[i]) {
                max_deque.pop_back();
            }
            max_deque.push_back(i);

            // Maintain min deque (increasing lows)
            while min_deque.back().is_some_and(|&j| lows[j] >= lows[i]) {
                min_deque.pop_back();
            }
            min_deque.push_back(i);

            // Remove elements outside the window
            let window_start = i + 1 - k_period.min(i + 1);
            while max_deque.front().is_some_and(|&j| j < window_start) {
                max_deque.pop_front();
            }
            while min_deque.front().is_some_and(|&j| j < window_start) {
                min_deque.pop_front();
            }

            if i >= k_period - 1 {
                let highest = highs[max_deque[0]];
                let lowest = lows[min_deque[0]];
                let close = candles[i].close.to_f32();
                let range = highest - lowest;
                let k_val = if range > 0.0 {
                    100.0 * (close - lowest) as f64 / range as f64
                } else {
                    50.0 // Flat market, default to midpoint
                };
                raw_k.push(k_val);
            }
        }

        // Step 2: Smooth %K with SMA
        let smoothed_k = if smooth > 1 {
            compute_sma(&raw_k, smooth)
        } else {
            raw_k.clone()
        };

        if smoothed_k.is_empty() {
            self.output = StudyOutput::Empty;
            return Ok(StudyResult::ok());
        }

        // Step 3: %D = SMA of smoothed %K
        let d_values = compute_sma(&smoothed_k, d_period);

        if d_values.is_empty() {
            self.output = StudyOutput::Empty;
            return Ok(StudyResult::ok());
        }

        // Build points for %K (aligned with %D).
        //
        // Offset derivation (candle indices, 0-based):
        //   raw_k[0]     -> candle index (k_period - 1)
        //   smoothed_k   = SMA(raw_k, smooth) -> first valid at
        //                   raw_k index (smooth - 1), which is
        //                   candle index (k_period - 1) + (smooth - 1)
        //   k_offset     = k_period - 1 + (smooth - 1)
        //
        //   d_values     = SMA(smoothed_k, d_period) -> first valid at
        //                   smoothed_k index (d_period - 1), which is
        //                   candle index k_offset + (d_period - 1)
        //   d_offset     = k_offset + (d_period - 1)
        //
        // Example: k_period=14, smooth=3, d_period=3
        //   k_offset = 13 + 2 = 15
        //   d_offset = 15 + 2 = 17
        //   -> first output at candle index 17
        let k_offset = k_period - 1 + (smooth.max(1) - 1);
        let d_offset = k_offset + (d_period - 1);

        // We output %K from d_offset onward so both lines are aligned
        let k_start_in_smoothed = d_period - 1; // index into smoothed_k

        let mut k_points = Vec::with_capacity(d_values.len());
        let mut d_points = Vec::with_capacity(d_values.len());

        for (j, d_val) in d_values.iter().enumerate() {
            let candle_idx = d_offset + j;
            if candle_idx >= candles.len() {
                break;
            }
            let key = candle_key(
                &candles[candle_idx],
                candle_idx,
                candles.len(),
                &input.basis,
            );
            let k_val = smoothed_k[k_start_in_smoothed + j];
            k_points.push((key, k_val as f32));
            d_points.push((key, *d_val as f32));
        }

        self.output = StudyOutput::Lines(vec![
            LineSeries {
                label: "%K".to_string(),
                color: k_color,
                width: 1.5,
                style: LineStyleValue::Solid,
                points: k_points,
            },
            LineSeries {
                label: "%D".to_string(),
                color: d_color,
                width: 1.5,
                style: LineStyleValue::Dashed,
                points: d_points,
            },
        ]);
        Ok(StudyResult::ok())
    }
}
