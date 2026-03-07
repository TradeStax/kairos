//! Bollinger Bands.
//!
//! A volatility envelope around a Simple Moving Average (SMA). The upper
//! and lower bands are placed at `multiplier * stddev` above and below
//! the SMA, where the standard deviation is computed over the same
//! rolling window as the SMA (population stddev, dividing by *N*).
//!
//! # Formula
//!
//! ```text
//! Middle = SMA(close, period)
//! Upper  = Middle + multiplier * stddev(close, period)
//! Lower  = Middle - multiplier * stddev(close, period)
//! ```
//!
//! Default: 20-period SMA with bands at +/- 2 standard deviations.
//!
//! # Trading use
//!
//! - **Squeeze / breakout**: when the bands contract to their narrowest
//!   width, a large directional move often follows. Traders watch for
//!   the "squeeze" and enter on the subsequent expansion.
//! - **Overbought / oversold**: a close above the upper band or below
//!   the lower band can signal an extended move. In mean-reverting
//!   markets this suggests a potential pullback; in trending markets it
//!   confirms momentum.
//! - **Band walk**: in strong trends, price rides along the upper or
//!   lower band for sustained periods. Falling back inside the bands
//!   signals weakening momentum.
//! - **Bandwidth indicator**: the distance between bands (bandwidth)
//!   is itself a volatility measure that can be charted separately.
//!
//! # Output
//!
//! Produces [`StudyOutput::Band`] with upper, middle (SMA), and lower
//! [`LineSeries`] plus a configurable fill opacity between the bands.
//! Mean and standard deviation are computed via the shared helpers in
//! [`crate::util::math`].

mod params;
#[cfg(test)]
mod tests;

use crate::config::{LineStyleValue, ParameterDef, StudyConfig};
use crate::core::{
    Study, StudyCapabilities, StudyCategory, StudyInput, StudyMetadata, StudyPlacement, StudyResult,
};
use crate::error::StudyError;
use crate::output::{LineSeries, StudyOutput};
use crate::util::math;
use crate::util::{candle_key, source_value};
use params::{DEFAULT_LOWER_COLOR, DEFAULT_MIDDLE_COLOR, DEFAULT_UPPER_COLOR, make_params};

/// Bollinger Bands study.
///
/// Renders three overlay lines (upper, middle SMA, lower) with a
/// semi-transparent fill between the bands. The fill opacity is
/// configurable. Each band line has independent color control.
///
/// Configurable parameters: look-back period, standard-deviation
/// multiplier, upper/middle/lower colors, and fill opacity. The study
/// produces [`StudyOutput::Band`].
pub struct BollingerStudy {
    metadata: StudyMetadata,
    config: StudyConfig,
    output: StudyOutput,
    params: Vec<ParameterDef>,
}

impl BollingerStudy {
    /// Create a new Bollinger Bands study with default parameters.
    ///
    /// Defaults: period = 20, std_dev multiplier = 2.0, blue band
    /// colors, fill opacity = 0.1.
    pub fn new() -> Self {
        let params = make_params();
        let config = StudyConfig::from_params("bollinger", &params);

        Self {
            metadata: StudyMetadata {
                name: "Bollinger Bands".to_string(),
                category: StudyCategory::Volatility,
                placement: StudyPlacement::Overlay,
                description: "SMA with standard deviation bands".to_string(),
                config_version: 1,
                capabilities: StudyCapabilities::default(),
            },
            config,
            output: StudyOutput::Empty,
            params,
        }
    }
}

impl Default for BollingerStudy {
    fn default() -> Self {
        Self::new()
    }
}

impl Study for BollingerStudy {
    crate::impl_study_base!("bollinger");

    fn compute(&mut self, input: &StudyInput) -> Result<StudyResult, StudyError> {
        let period = self.config.get_int("period", 20) as usize;
        let std_mult = self.config.get_float("std_dev", 2.0);
        let upper_color = self.config.get_color("upper_color", DEFAULT_UPPER_COLOR);
        let middle_color = self.config.get_color("middle_color", DEFAULT_MIDDLE_COLOR);
        let lower_color = self.config.get_color("lower_color", DEFAULT_LOWER_COLOR);
        let fill_opacity = self.config.get_float("fill_opacity", 0.1) as f32;

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

        let count = candles.len() - period + 1;
        let mut upper_points = Vec::with_capacity(count);
        let mut middle_points = Vec::with_capacity(count);
        let mut lower_points = Vec::with_capacity(count);

        // Extract all close values
        let values: Vec<f64> = candles
            .iter()
            .map(|c| source_value(c, "Close") as f64)
            .collect();

        for i in (period - 1)..candles.len() {
            let start = i + 1 - period;
            let window = &values[start..=i];

            let avg = math::mean(window);
            let stddev = math::standard_deviation_with_mean(window, avg);

            let key = candle_key(&candles[i], i, candles.len(), &input.basis);
            upper_points.push((key, (avg + std_mult * stddev) as f32));
            middle_points.push((key, avg as f32));
            lower_points.push((key, (avg - std_mult * stddev) as f32));
        }

        self.output = StudyOutput::Band {
            upper: LineSeries {
                label: "Upper".to_string(),
                color: upper_color,
                width: 1.0,
                style: LineStyleValue::Solid,
                points: upper_points,
            },
            middle: Some(LineSeries {
                label: format!("BB({})", period),
                color: middle_color,
                width: 1.0,
                style: LineStyleValue::Solid,
                points: middle_points,
            }),
            lower: LineSeries {
                label: "Lower".to_string(),
                color: lower_color,
                width: 1.0,
                style: LineStyleValue::Solid,
                points: lower_points,
            },
            fill_opacity,
        };
        Ok(StudyResult::ok())
    }
}
