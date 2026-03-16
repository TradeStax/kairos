//! Simple Moving Average (SMA).
//!
//! The Simple Moving Average smooths price data by computing an equal-weight
//! arithmetic mean of the last `period` candle values. Because every value in
//! the window contributes equally, the SMA changes only when a new value
//! enters or an old value leaves the window -- making it a stable, low-noise
//! trend filter.
//!
//! # Formula
//!
//! ```text
//! SMA(t) = (P(t) + P(t-1) + ... + P(t-n+1)) / n
//! ```
//!
//! where `P` is the chosen price source (Close by default) and `n` is the
//! period.
//!
//! # Trading use
//!
//! - **Trend direction**: a rising SMA suggests an uptrend; a falling SMA
//!   suggests a downtrend.
//! - **Dynamic support/resistance**: price often bounces off the SMA line,
//!   especially on longer periods (50, 100, 200).
//! - **Crossover signals**: a short-period SMA crossing above a long-period
//!   SMA is a classic bullish signal (and vice versa).
//! - Common periods: 20 (short-term), 50 (medium), 200 (long-term).
//!
//! # Implementation
//!
//! Implemented as an efficient O(n) sliding-window sum. Output starts at
//! index `period - 1` (the first candle for which a full window exists).

mod params;
#[cfg(test)]
mod tests;

use crate::config::StudyConfig;
use crate::core::{
    Study, StudyCapabilities, StudyCategory, StudyInput, StudyMetadata, StudyPlacement, StudyResult,
};
use crate::error::StudyError;
use crate::output::{LineSeries, StudyOutput};
use crate::util::{candle_key, source_value};
use data::SerializableColor;

use params::make_params;

/// Simple Moving Average study.
///
/// Renders a single line on the price chart showing the equal-weight
/// average of the last `period` candle values. Configurable parameters
/// include the look-back period, the price source (Close, Open, High,
/// Low, HL2, HLC3, OHLC4), and visual styling (color, line width).
///
/// The study produces [`StudyOutput::Lines`] with a single
/// [`LineSeries`] labeled `SMA(<period>)`.
pub struct SmaStudy {
    metadata: StudyMetadata,
    config: StudyConfig,
    output: StudyOutput,
    params: Vec<crate::config::ParameterDef>,
}

impl SmaStudy {
    /// Create a new SMA study with default parameters.
    ///
    /// Defaults: period = 20, source = Close, color = blue, width = 1.5.
    pub fn new() -> Self {
        let params = make_params();
        let config = StudyConfig::from_params("sma", &params);

        Self {
            metadata: StudyMetadata {
                name: "Simple Moving Average".to_string(),
                category: StudyCategory::Trend,
                placement: StudyPlacement::Overlay,
                description: "Simple moving average of price".to_string(),
                config_version: 1,
                capabilities: StudyCapabilities::default(),
            },
            config,
            output: StudyOutput::Empty,
            params,
        }
    }
}

impl Default for SmaStudy {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute a simple moving average over a slice.
///
/// Returns a vec of length `values.len() - period + 1` (starting at index
/// `period - 1` in the original series). Returns empty if insufficient data.
pub fn compute_sma(values: &[f64], period: usize) -> Vec<f64> {
    if values.len() < period || period == 0 {
        return vec![];
    }
    let mut result = Vec::with_capacity(values.len() - period + 1);
    let mut sum: f64 = values[..period].iter().sum();
    result.push(sum / period as f64);
    for i in period..values.len() {
        sum += values[i] - values[i - period];
        result.push(sum / period as f64);
    }
    result
}

impl Study for SmaStudy {
    crate::impl_study_base!("sma");

    fn compute(&mut self, input: &StudyInput) -> Result<StudyResult, StudyError> {
        let period = self.config.get_int("period", 20) as usize;
        let color = self.config.get_color(
            "color",
            SerializableColor {
                r: 0.2,
                g: 0.6,
                b: 1.0,
                a: 1.0,
            },
        );
        let width = self.config.get_float("width", 1.5) as f32;
        let source = self.config.get_choice("source", "Close").to_string();

        let candles = input.candles;
        if candles.len() < period {
            log::debug!(
                "{}: insufficient data ({} candles, need {})",
                self.id(),
                candles.len(),
                period
            );
            self.output = StudyOutput::Empty;
            return Ok(StudyResult::ok());
        }

        let total = candles.len();
        let mut points = Vec::with_capacity(total - period + 1);

        // Calculate initial sum
        let mut sum: f64 = 0.0;
        for candle in &candles[..period] {
            sum += source_value(candle, &source) as f64;
        }
        points.push((
            candle_key(&candles[period - 1], period - 1, total, &input.basis),
            (sum / period as f64) as f32,
        ));

        // Sliding window
        for i in period..total {
            sum += source_value(&candles[i], &source) as f64;
            sum -= source_value(&candles[i - period], &source) as f64;
            points.push((
                candle_key(&candles[i], i, total, &input.basis),
                (sum / period as f64) as f32,
            ));
        }

        self.output = StudyOutput::Lines(vec![LineSeries {
            label: format!("SMA({})", period),
            color,
            width,
            style: crate::config::LineStyleValue::Solid,
            points,
        }]);
        Ok(StudyResult::ok())
    }
}
