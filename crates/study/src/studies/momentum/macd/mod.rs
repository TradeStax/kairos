//! Moving Average Convergence Divergence (MACD).
//!
//! MACD tracks the relationship between two exponential moving averages
//! of closing prices. It consists of three components:
//!
//! - **MACD line** — difference between the fast and slow EMAs.
//! - **Signal line** — EMA of the MACD line itself.
//! - **Histogram** — MACD minus Signal, visualising momentum shifts.
//!
//! Traders use MACD crossovers (MACD crossing the signal line), histogram
//! direction changes, and divergences from price to gauge trend momentum
//! and potential reversals.
//!
//! Output: `StudyOutput::Composite` containing two line series (MACD and
//! Signal) and a histogram.

mod params;
#[cfg(test)]
mod tests;

use crate::config::{LineStyleValue, StudyConfig};
use crate::core::{
    Study, StudyCapabilities, StudyCategory, StudyInput, StudyMetadata, StudyPlacement, StudyResult,
};
use crate::error::StudyError;
use crate::output::{HistogramBar, LineSeries, StudyOutput};
use crate::studies::trend::ema::compute_ema;
use crate::util::candle_key;
use crate::{BEARISH_COLOR, BULLISH_COLOR};
use data::SerializableColor;
use params::make_params;

/// MACD oscillator with signal line and histogram.
///
/// The MACD line is the difference between a fast and slow EMA of
/// closing prices. The signal line is an EMA of the MACD line, and the
/// histogram visualises MACD minus signal. Renders in a separate panel
/// with two overlaid lines and a colored divergence histogram.
pub struct MacdStudy {
    metadata: StudyMetadata,
    config: StudyConfig,
    output: StudyOutput,
    params: Vec<crate::config::ParameterDef>,
}

impl MacdStudy {
    /// Create a new MACD study with the classic default periods:
    /// fast EMA 12, slow EMA 26, signal EMA 9.
    pub fn new() -> Self {
        let params = make_params();
        let config = StudyConfig::from_params("macd", &params);

        Self {
            metadata: StudyMetadata {
                name: "MACD".to_string(),
                category: StudyCategory::Momentum,
                placement: StudyPlacement::Panel,
                description: "Moving Average Convergence Divergence".to_string(),
                config_version: 1,
                capabilities: StudyCapabilities::default(),
            },
            config,
            output: StudyOutput::Empty,
            params,
        }
    }
}

impl Default for MacdStudy {
    fn default() -> Self {
        Self::new()
    }
}

impl Study for MacdStudy {
    crate::impl_study_base!("macd");

    fn compute(&mut self, input: &StudyInput) -> Result<StudyResult, StudyError> {
        let fast_period = self.config.get_int("fast_period", 12) as usize;
        let slow_period = self.config.get_int("slow_period", 26) as usize;
        let signal_period = self.config.get_int("signal_period", 9) as usize;
        let macd_color = self.config.get_color(
            "macd_color",
            SerializableColor {
                r: 0.2,
                g: 0.6,
                b: 1.0,
                a: 1.0,
            },
        );
        let signal_color = self.config.get_color(
            "signal_color",
            SerializableColor {
                r: 1.0,
                g: 0.6,
                b: 0.2,
                a: 1.0,
            },
        );
        let hist_pos_color = self
            .config
            .get_color("hist_positive_color", BULLISH_COLOR.with_alpha(0.7));
        let hist_neg_color = self
            .config
            .get_color("hist_negative_color", BEARISH_COLOR.with_alpha(0.7));
        let candles = input.candles;
        let max_period = fast_period.max(slow_period);
        if candles.len() < max_period {
            log::debug!(
                "{}: insufficient data ({} candles, need {})",
                self.id(),
                candles.len(),
                max_period
            );
            self.output = StudyOutput::Empty;
            return Ok(StudyResult::ok());
        }

        let closes: Vec<f64> = candles.iter().map(|c| c.close.to_f64()).collect();

        // Compute fast and slow EMAs
        let fast_ema = compute_ema(&closes, fast_period);
        let slow_ema = compute_ema(&closes, slow_period);

        // Align fast and slow EMAs.
        // compute_ema() returns (N - period + 1) values, so:
        //   fast_ema[0] corresponds to candle index (fast_period - 1)
        //   slow_ema[0] corresponds to candle index (slow_period - 1)
        // To subtract element-wise we need to align them. Since
        // slow_period >= fast_period, fast_ema has more elements.
        // We skip the first `fast_offset` entries of fast_ema so
        // that fast_ema[fast_offset + i] and slow_ema[i] both
        // correspond to the same candle index (slow_period - 1 + i).
        //
        // Example: fast=12, slow=26
        //   fast_ema has (N - 11) values, slow_ema has (N - 25)
        //   fast_offset = 26 - 12 = 14
        //   fast_ema[14] = candle index 25, slow_ema[0] = candle index 25
        let fast_offset = slow_period.saturating_sub(fast_period);
        let macd_len = fast_ema
            .len()
            .saturating_sub(fast_offset)
            .min(slow_ema.len());
        if macd_len == 0 {
            self.output = StudyOutput::Empty;
            return Ok(StudyResult::ok());
        }

        let macd_values: Vec<f64> = (0..macd_len)
            .map(|i| fast_ema[i + fast_offset] - slow_ema[i])
            .collect();

        // Compute signal line (EMA of MACD values)
        let signal_ema = compute_ema(&macd_values, signal_period);
        if signal_ema.is_empty() {
            self.output = StudyOutput::Empty;
            return Ok(StudyResult::ok());
        }

        // The signal line starts at (signal_period - 1) within macd_values
        // In candle space, MACD starts at (slow_period - 1), signal starts
        // at (slow_period - 1 + signal_period - 1)
        let signal_start_in_macd = signal_period - 1;
        let candle_offset_macd = slow_period - 1;
        let candle_offset_signal = candle_offset_macd + signal_start_in_macd;

        // Build MACD line points (from where signal starts, for alignment)
        let mut macd_points = Vec::with_capacity(signal_ema.len());
        let mut signal_points = Vec::with_capacity(signal_ema.len());
        let mut histogram = Vec::with_capacity(signal_ema.len());

        for (i, &sig) in signal_ema.iter().enumerate() {
            let candle_idx = candle_offset_signal + i;
            if candle_idx >= candles.len() {
                break;
            }
            let key = candle_key(
                &candles[candle_idx],
                candle_idx,
                candles.len(),
                &input.basis,
            );
            let macd_val = macd_values[signal_start_in_macd + i];
            let hist_val = macd_val - sig;

            macd_points.push((key, macd_val as f32));
            signal_points.push((key, sig as f32));
            histogram.push(HistogramBar {
                x: key,
                value: hist_val as f32,
                color: if hist_val >= 0.0 {
                    hist_pos_color
                } else {
                    hist_neg_color
                },
            });
        }

        self.output = StudyOutput::Composite(vec![
            StudyOutput::Lines(vec![
                LineSeries {
                    label: "MACD".to_string(),
                    color: macd_color,
                    width: 1.5,
                    style: LineStyleValue::Solid,
                    points: macd_points,
                },
                LineSeries {
                    label: "Signal".to_string(),
                    color: signal_color,
                    width: 1.5,
                    style: LineStyleValue::Solid,
                    points: signal_points,
                },
            ]),
            StudyOutput::Histogram(histogram),
        ]);
        Ok(StudyResult::ok())
    }
}
