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

use crate::config::{
    DisplayFormat, ParameterDef, ParameterKind, ParameterTab, ParameterValue, StudyConfig,
    Visibility,
};
use crate::core::{
    Study, StudyCapabilities, StudyCategory, StudyInput, StudyMetadata, StudyPlacement, StudyResult,
};
use crate::error::StudyError;
use crate::output::{LineSeries, StudyOutput};
use crate::util::{candle_key, source_value};
use data::SerializableColor;

const SOURCE_OPTIONS: &[&str] = &["Close", "Open", "High", "Low", "HL2", "HLC3", "OHLC4"];

fn make_params() -> Vec<ParameterDef> {
    vec![
        ParameterDef {
            key: "period".into(),
            label: "Period".into(),
            description: "Number of candles for the moving average".into(),
            kind: ParameterKind::Integer { min: 2, max: 500 },
            default: ParameterValue::Integer(20),
            tab: ParameterTab::Parameters,
            section: None,
            order: 0,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "color".into(),
            label: "Color".into(),
            description: "Line color".into(),
            kind: ParameterKind::Color,
            default: ParameterValue::Color(SerializableColor {
                r: 0.2,
                g: 0.6,
                b: 1.0,
                a: 1.0,
            }),
            tab: ParameterTab::Style,
            section: None,
            order: 0,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "width".into(),
            label: "Width".into(),
            description: "Line width".into(),
            kind: ParameterKind::Float {
                min: 0.5,
                max: 5.0,
                step: 0.5,
            },
            default: ParameterValue::Float(1.5),
            tab: ParameterTab::Style,
            section: None,
            order: 1,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "source".into(),
            label: "Source".into(),
            description: "Price source for calculation".into(),
            kind: ParameterKind::Choice {
                options: SOURCE_OPTIONS,
            },
            default: ParameterValue::Choice("Close".to_string()),
            tab: ParameterTab::Parameters,
            section: None,
            order: 1,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
    ]
}

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
    params: Vec<ParameterDef>,
}

impl SmaStudy {
    /// Create a new SMA study with default parameters.
    ///
    /// Defaults: period = 20, source = Close, color = blue, width = 1.5.
    pub fn new() -> Self {
        let params = make_params();
        let mut config = StudyConfig::new("sma");
        for p in &params {
            config.set(p.key.clone(), p.default.clone());
        }

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
    fn id(&self) -> &str {
        "sma"
    }

    fn metadata(&self) -> &StudyMetadata {
        &self.metadata
    }

    fn parameters(&self) -> &[ParameterDef] {
        &self.params
    }

    fn config(&self) -> &StudyConfig {
        &self.config
    }

    fn config_mut(&mut self) -> &mut StudyConfig {
        &mut self.config
    }

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

    fn output(&self) -> &StudyOutput {
        &self.output
    }

    fn reset(&mut self) {
        self.output = StudyOutput::Empty;
    }

    fn clone_study(&self) -> Box<dyn Study> {
        Box::new(SmaStudy {
            metadata: self.metadata.clone(),
            config: self.config.clone(),
            output: self.output.clone(),
            params: self.params.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::test_helpers::{make_candle, make_input};
    use data::Candle;

    #[test]
    fn test_empty_candles() {
        let mut study = SmaStudy::new();
        let input = make_input(&[]);
        study.compute(&input).unwrap();
        assert!(matches!(study.output(), StudyOutput::Empty));
    }

    #[test]
    fn test_insufficient_candles() {
        let mut study = SmaStudy::new();
        // Default period is 20, so 5 candles is insufficient
        let candles: Vec<Candle> = (0..5).map(|i| make_candle(i * 60000, 100.0)).collect();
        let input = make_input(&candles);
        study.compute(&input).unwrap();
        assert!(matches!(study.output(), StudyOutput::Empty));
    }

    #[test]
    fn test_sma_calculation() {
        let mut study = SmaStudy::new();
        study
            .set_parameter("period", ParameterValue::Integer(3))
            .unwrap();

        let candles = vec![
            make_candle(1000, 10.0),
            make_candle(2000, 20.0),
            make_candle(3000, 30.0),
            make_candle(4000, 40.0),
            make_candle(5000, 50.0),
        ];
        let input = make_input(&candles);
        study.compute(&input).unwrap();

        let output = study.output();
        assert!(
            matches!(output, StudyOutput::Lines(_)),
            "expected Lines output"
        );
        let StudyOutput::Lines(lines) = output else {
            unreachable!()
        };
        assert_eq!(lines.len(), 1);
        let points = &lines[0].points;
        assert_eq!(points.len(), 3);
        // SMA(3) of [10, 20, 30] = 20.0
        assert!((points[0].1 - 20.0).abs() < 0.01);
        // SMA(3) of [20, 30, 40] = 30.0
        assert!((points[1].1 - 30.0).abs() < 0.01);
        // SMA(3) of [30, 40, 50] = 40.0
        assert!((points[2].1 - 40.0).abs() < 0.01);
    }

    #[test]
    fn test_set_parameter_valid() {
        let mut study = SmaStudy::new();
        assert!(
            study
                .set_parameter("period", ParameterValue::Integer(50))
                .is_ok()
        );
    }

    #[test]
    fn test_set_parameter_invalid_range() {
        let mut study = SmaStudy::new();
        assert!(
            study
                .set_parameter("period", ParameterValue::Integer(0))
                .is_err()
        );
        assert!(
            study
                .set_parameter("period", ParameterValue::Integer(501))
                .is_err()
        );
    }

    #[test]
    fn test_set_parameter_wrong_type() {
        let mut study = SmaStudy::new();
        assert!(
            study
                .set_parameter("period", ParameterValue::Float(5.0))
                .is_err()
        );
    }

    #[test]
    fn test_set_parameter_unknown() {
        let mut study = SmaStudy::new();
        assert!(
            study
                .set_parameter("unknown", ParameterValue::Integer(5))
                .is_err()
        );
    }
}
