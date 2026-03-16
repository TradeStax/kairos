//! Exponential Moving Average (EMA).
//!
//! The Exponential Moving Average gives progressively more weight to recent
//! prices, allowing it to react faster to new information than an equal-weight
//! SMA. The smoothing factor `k = 2 / (period + 1)` controls how quickly old
//! data decays: a shorter period means a larger `k` and a more responsive
//! line.
//!
//! # Formula
//!
//! ```text
//! EMA(t) = P(t) * k + EMA(t-1) * (1 - k)
//! ```
//!
//! The first EMA value is seeded with the Simple Moving Average of the first
//! `period` values. Output starts at index `period - 1`.
//!
//! # Trading use
//!
//! - **Trend detection**: the slope and position of the EMA relative to
//!   price quickly reveals the short-term trend direction.
//! - **Crossover systems**: MACD is built on EMA crossovers (12 vs 26).
//!   A fast EMA crossing above a slow EMA signals bullish momentum.
//! - **Dynamic support/resistance**: like the SMA, the EMA acts as a
//!   moving reference level, but hugs price more closely.
//! - Common periods: 9 (very short-term), 12, 21, 26 (MACD components).
//!
//! # Implementation
//!
//! Seeded with SMA, then iteratively applies the multiplier. O(n) time
//! complexity.

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

/// Exponential Moving Average study.
///
/// Renders a single line on the price chart showing the
/// exponentially-weighted average of the last `period` candle values.
/// Configurable parameters include the look-back period, the price
/// source (Close, Open, High, Low, HL2, HLC3, OHLC4), and visual
/// styling (color, line width).
///
/// The study produces [`StudyOutput::Lines`] with a single
/// [`LineSeries`] labeled `EMA(<period>)`.
pub struct EmaStudy {
    metadata: StudyMetadata,
    config: StudyConfig,
    output: StudyOutput,
    params: Vec<crate::config::ParameterDef>,
}

impl EmaStudy {
    /// Create a new EMA study with default parameters.
    ///
    /// Defaults: period = 9, source = Close, color = orange, width = 1.5.
    pub fn new() -> Self {
        let params = make_params();
        let config = StudyConfig::from_params("ema", &params);

        Self {
            metadata: StudyMetadata {
                name: "Exponential Moving Average".to_string(),
                category: StudyCategory::Trend,
                placement: StudyPlacement::Overlay,
                description: "Exponential moving average of price".to_string(),
                config_version: 1,
                capabilities: StudyCapabilities::default(),
            },
            config,
            output: StudyOutput::Empty,
            params,
        }
    }
}

impl Default for EmaStudy {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute EMA values from a slice of f64 values.
///
/// Returns EMA values starting from index `period - 1` (length =
/// `values.len() - period + 1`). Seeds with SMA, then applies the
/// standard multiplier `2 / (period + 1)`.
pub fn compute_ema(values: &[f64], period: usize) -> Vec<f64> {
    if values.len() < period || period == 0 {
        return vec![];
    }

    let multiplier = crate::util::math::ema_multiplier(period);
    let mut result = Vec::with_capacity(values.len() - period + 1);

    let sma: f64 = values[..period].iter().sum::<f64>() / period as f64;
    result.push(sma);

    for &val in &values[period..] {
        let prev = *result.last().unwrap();
        result.push(val * multiplier + prev * (1.0 - multiplier));
    }

    result
}

impl Study for EmaStudy {
    crate::impl_study_base!("ema");

    fn compute(&mut self, input: &StudyInput) -> Result<StudyResult, StudyError> {
        let period = self.config.get_int("period", 9) as usize;
        let color = self.config.get_color(
            "color",
            SerializableColor {
                r: 1.0,
                g: 0.6,
                b: 0.2,
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

        let multiplier = crate::util::math::ema_multiplier(period);
        let mut points = Vec::with_capacity(candles.len() - period + 1);

        // Seed EMA with SMA of first `period` candles
        let mut sum: f64 = 0.0;
        for candle in &candles[..period] {
            sum += source_value(candle, &source) as f64;
        }
        let mut ema = sum / period as f64;
        points.push((
            candle_key(
                &candles[period - 1],
                period - 1,
                candles.len(),
                &input.basis,
            ),
            ema as f32,
        ));

        // EMA from period onward
        for (i, candle) in candles.iter().enumerate().skip(period) {
            let val = source_value(candle, &source) as f64;
            ema = val * multiplier + ema * (1.0 - multiplier);
            points.push((
                candle_key(candle, i, candles.len(), &input.basis),
                ema as f32,
            ));
        }

        self.output = StudyOutput::Lines(vec![LineSeries {
            label: format!("EMA({})", period),
            color,
            width,
            style: crate::config::LineStyleValue::Solid,
            points,
        }]);
        Ok(StudyResult::ok())
    }
}
