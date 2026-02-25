//! Exponential Moving Average (EMA).
//!
//! Applies a decreasing weight to older values using the multiplier
//! `k = 2 / (period + 1)`. Formula: `EMA(t) = P(t) * k + EMA(t-1) * (1 - k)`.
//!
//! Seeded with the SMA of the first `period` values. Output starts at
//! index `period - 1`. Responds faster to recent price changes than SMA.

use crate::config::{
    DisplayFormat, ParameterDef, ParameterKind, ParameterTab, ParameterValue, StudyConfig,
    Visibility,
};
use crate::error::StudyError;
use crate::output::{LineSeries, StudyOutput};
use crate::core::{Study, StudyCategory, StudyInput, StudyPlacement};
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
            default: ParameterValue::Integer(9),
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
                r: 1.0,
                g: 0.6,
                b: 0.2,
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

pub struct EmaStudy {
    config: StudyConfig,
    output: StudyOutput,
    params: Vec<ParameterDef>,
}

impl EmaStudy {
    pub fn new() -> Self {
        let params = make_params();
        let mut config = StudyConfig::new("ema");
        for p in &params {
            config.set(p.key.clone(), p.default.clone());
        }
        config.set("source", ParameterValue::Choice("Close".to_string()));

        Self {
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

    let multiplier = 2.0 / (period + 1) as f64;
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
    fn id(&self) -> &str {
        "ema"
    }

    fn name(&self) -> &str {
        "Exponential Moving Average"
    }

    fn category(&self) -> StudyCategory {
        StudyCategory::Trend
    }

    fn placement(&self) -> StudyPlacement {
        StudyPlacement::Overlay
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

    fn compute(&mut self, input: &StudyInput) -> Result<(), StudyError> {
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
            self.output = StudyOutput::Empty;
            return Ok(());
        }

        let multiplier = 2.0 / (period + 1) as f64;
        let mut points = Vec::with_capacity(candles.len() - period + 1);

        // Seed EMA with SMA of first `period` candles
        let mut sum: f64 = 0.0;
        for candle in &candles[..period] {
            sum += source_value(candle, &source) as f64;
        }
        let mut ema = sum / period as f64;
        points.push((
            candle_key(&candles[period - 1], period - 1, candles.len(), &input.basis),
            ema as f32,
        ));

        // EMA from period onward
        for (i, candle) in candles.iter().enumerate().skip(period) {
            let val = source_value(candle, &source) as f64;
            ema = val * multiplier + ema * (1.0 - multiplier);
            points.push((candle_key(candle, i, candles.len(), &input.basis), ema as f32));
        }

        self.output = StudyOutput::Lines(vec![LineSeries {
            label: format!("EMA({})", period),
            color,
            width,
            style: crate::config::LineStyleValue::Solid,
            points,
        }]);
        Ok(())
    }

    fn output(&self) -> &StudyOutput {
        &self.output
    }

    fn reset(&mut self) {
        self.output = StudyOutput::Empty;
    }

    fn clone_study(&self) -> Box<dyn Study> {
        Box::new(EmaStudy {
            config: self.config.clone(),
            output: self.output.clone(),
            params: self.params.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use data::{Candle, ChartBasis, Price, Timeframe, Timestamp, Volume};

    fn make_candle(time: u64, close: f32) -> Candle {
        Candle::new(
            Timestamp(time),
            Price::from_f32(close),
            Price::from_f32(close),
            Price::from_f32(close),
            Price::from_f32(close),
            Volume(0.0),
            Volume(0.0),
        )
        .expect("test: valid candle")
    }

    fn make_input(candles: &[Candle]) -> StudyInput<'_> {
        StudyInput {
            candles,
            trades: None,
            basis: ChartBasis::Time(Timeframe::M1),
            tick_size: Price::from_f32(0.25),
            visible_range: None,
        }
    }

    #[test]
    fn test_empty_candles() {
        let mut study = EmaStudy::new();
        let input = make_input(&[]);
        study.compute(&input).unwrap();
        assert!(matches!(study.output(), StudyOutput::Empty));
    }

    #[test]
    fn test_insufficient_candles() {
        let mut study = EmaStudy::new();
        // Default period is 9
        let candles: Vec<Candle> = (0..5).map(|i| make_candle(i * 60000, 100.0)).collect();
        let input = make_input(&candles);
        study.compute(&input).unwrap();
        assert!(matches!(study.output(), StudyOutput::Empty));
    }

    #[test]
    fn test_ema_calculation() {
        let mut study = EmaStudy::new();
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
        assert!(matches!(output, StudyOutput::Lines(_)), "expected Lines output");
        let StudyOutput::Lines(lines) = output else { unreachable!() };
        assert_eq!(lines.len(), 1);
        let points = &lines[0].points;
        assert_eq!(points.len(), 3);

        // Seed: SMA(3) of [10, 20, 30] = 20.0
        assert!((points[0].1 - 20.0).abs() < 0.01);

        // multiplier = 2/(3+1) = 0.5
        // EMA = 40 * 0.5 + 20 * 0.5 = 30.0
        assert!((points[1].1 - 30.0).abs() < 0.01);

        // EMA = 50 * 0.5 + 30 * 0.5 = 40.0
        assert!((points[2].1 - 40.0).abs() < 0.01);
    }

    #[test]
    fn test_set_parameter_valid() {
        let mut study = EmaStudy::new();
        assert!(
            study
                .set_parameter("period", ParameterValue::Integer(50))
                .is_ok()
        );
    }

    #[test]
    fn test_set_parameter_invalid_range() {
        let mut study = EmaStudy::new();
        assert!(
            study
                .set_parameter("period", ParameterValue::Integer(1))
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
        let mut study = EmaStudy::new();
        assert!(
            study
                .set_parameter("period", ParameterValue::Float(5.0))
                .is_err()
        );
    }

    #[test]
    fn test_set_parameter_unknown() {
        let mut study = EmaStudy::new();
        assert!(
            study
                .set_parameter("unknown", ParameterValue::Integer(5))
                .is_err()
        );
    }
}
