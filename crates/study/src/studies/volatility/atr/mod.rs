//! Average True Range (ATR).
//!
//! The ATR measures market volatility by averaging the True Range over a
//! given look-back period. True Range captures the full extent of each
//! bar's price movement, including any gap from the previous close, so
//! it reflects volatility more accurately than a simple High-Low range.
//!
//! # Formulas
//!
//! ```text
//! True Range = max(High - Low, |High - PrevClose|, |Low - PrevClose|)
//! ```
//!
//! Smoothing uses Wilder's method (equivalent to an EMA with
//! `k = 1/period`):
//!
//! ```text
//! ATR(t) = (ATR(t-1) * (period - 1) + TR(t)) / period
//! ```
//!
//! The initial ATR is seeded with the simple average of the first `period`
//! True Range values.
//!
//! # Trading use
//!
//! - **Position sizing**: ATR-based sizing (e.g. risking 1-2 ATR per
//!   trade) normalizes risk across instruments of different volatility.
//! - **Stop-loss placement**: trailing stops set at a multiple of ATR
//!   (e.g. 1.5x or 2x) adapt to current market conditions.
//! - **Volatility regime detection**: rising ATR signals an expansion
//!   phase (trend or panic); falling ATR signals contraction and
//!   potential consolidation.
//! - The standard period is 14.
//!
//! # Output
//!
//! Rendered as a single line in a separate panel below the price chart,
//! since ATR values are in price-difference units rather than absolute
//! price levels.

mod params;
#[cfg(test)]
mod tests;

use crate::config::{ParameterDef, StudyConfig};
use crate::core::{
    Study, StudyCapabilities, StudyCategory, StudyInput, StudyMetadata, StudyPlacement, StudyResult,
};
use crate::error::StudyError;
use crate::output::{LineSeries, StudyOutput};
use crate::util::candle_key;
use params::{DEFAULT_COLOR, make_params};

/// Average True Range study.
///
/// Renders a single volatility line in a panel below the price chart.
/// Requires at least `period + 1` candles (one extra for the initial
/// previous-close reference needed to compute the first True Range).
///
/// Configurable parameters: look-back period, line color, and line
/// width. The study produces [`StudyOutput::Lines`] with a single
/// [`LineSeries`] labeled `ATR(<period>)`.
pub struct AtrStudy {
    metadata: StudyMetadata,
    config: StudyConfig,
    output: StudyOutput,
    params: Vec<ParameterDef>,
}

impl AtrStudy {
    /// Create a new ATR study with default parameters.
    ///
    /// Defaults: period = 14, color = orange, width = 1.5.
    pub fn new() -> Self {
        let params = make_params();
        let config = StudyConfig::from_params("atr", &params);

        Self {
            metadata: StudyMetadata {
                name: "Average True Range".to_string(),
                category: StudyCategory::Volatility,
                placement: StudyPlacement::Panel,
                description: "Average true range using Wilder's smoothing".to_string(),
                config_version: 1,
                capabilities: StudyCapabilities::default(),
            },
            config,
            output: StudyOutput::Empty,
            params,
        }
    }
}

impl Default for AtrStudy {
    fn default() -> Self {
        Self::new()
    }
}

impl Study for AtrStudy {
    crate::impl_study_base!("atr");

    fn compute(&mut self, input: &StudyInput) -> Result<StudyResult, StudyError> {
        let period = self.config.get_int("period", 14) as usize;
        let color = self.config.get_color("color", DEFAULT_COLOR);
        let width = self.config.get_float("width", 1.5) as f32;

        let candles = input.candles;
        if candles.len() < period + 1 {
            log::debug!(
                "{}: insufficient data ({} candles, need {})",
                self.id(),
                candles.len(),
                period + 1
            );
            self.output = StudyOutput::Empty;
            return Ok(StudyResult::ok());
        }

        // Calculate True Range for each candle (starting from index 1)
        let mut tr_values = Vec::with_capacity(candles.len() - 1);
        for i in 1..candles.len() {
            let high = candles[i].high.to_f32() as f64;
            let low = candles[i].low.to_f32() as f64;
            let prev_close = candles[i - 1].close.to_f32() as f64;

            let tr = (high - low)
                .max((high - prev_close).abs())
                .max((low - prev_close).abs());
            tr_values.push(tr);
        }

        if tr_values.len() < period {
            self.output = StudyOutput::Empty;
            return Ok(StudyResult::ok());
        }

        let mut points = Vec::with_capacity(tr_values.len() - period + 1);

        // Initial ATR: simple average of first `period` TR values
        let mut atr: f64 = tr_values[..period].iter().sum::<f64>() / period as f64;
        // The first ATR corresponds to candle at index period (since TR
        // starts at index 1)
        let candle_idx = period;
        points.push((
            candle_key(
                &candles[candle_idx],
                candle_idx,
                candles.len(),
                &input.basis,
            ),
            atr as f32,
        ));

        // Wilder's smoothing: ATR = (prev_ATR * (period-1) + TR) / period
        for (i, tr) in tr_values.iter().enumerate().skip(period) {
            atr = (atr * (period as f64 - 1.0) + tr) / period as f64;
            let candle_idx = i + 1; // offset by 1 since TR starts at index 1
            points.push((
                candle_key(
                    &candles[candle_idx],
                    candle_idx,
                    candles.len(),
                    &input.basis,
                ),
                atr as f32,
            ));
        }

        self.output = StudyOutput::Lines(vec![LineSeries {
            label: format!("ATR({})", period),
            color,
            width,
            style: crate::config::LineStyleValue::Solid,
            points,
        }]);
        Ok(StudyResult::ok())
    }
}
