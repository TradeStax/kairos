//! Bollinger Bands.
//!
//! A volatility envelope around a Simple Moving Average (SMA). The upper and
//! lower bands are placed at `multiplier * stddev` above and below the SMA.
//! Default: 20-period SMA ± 2 standard deviations.
//!
//! Output: `StudyOutput::Band` with upper, middle (SMA), and lower lines.

use crate::config::{
    DisplayFormat, LineStyleValue, ParameterDef, ParameterKind, ParameterTab,
    ParameterValue, StudyConfig, Visibility,
};
use crate::error::StudyError;
use crate::output::{LineSeries, StudyOutput};
use crate::core::{Study, StudyCategory, StudyInput, StudyPlacement};
use crate::util::{candle_key, source_value};
use data::SerializableColor;

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
            key: "std_dev".into(),
            label: "Std Dev".into(),
            description: "Standard deviation multiplier for bands".into(),
            kind: ParameterKind::Float {
                min: 0.5,
                max: 5.0,
                step: 0.5,
            },
            default: ParameterValue::Float(2.0),
            tab: ParameterTab::Parameters,
            section: None,
            order: 1,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "upper_color".into(),
            label: "Upper Color".into(),
            description: "Upper band color".into(),
            kind: ParameterKind::Color,
            default: ParameterValue::Color(SerializableColor {
                r: 0.2,
                g: 0.6,
                b: 1.0,
                a: 0.6,
            }),
            tab: ParameterTab::Style,
            section: None,
            order: 0,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "middle_color".into(),
            label: "Middle Color".into(),
            description: "Middle band (SMA) color".into(),
            kind: ParameterKind::Color,
            default: ParameterValue::Color(SerializableColor {
                r: 0.2,
                g: 0.6,
                b: 1.0,
                a: 1.0,
            }),
            tab: ParameterTab::Style,
            section: None,
            order: 1,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "lower_color".into(),
            label: "Lower Color".into(),
            description: "Lower band color".into(),
            kind: ParameterKind::Color,
            default: ParameterValue::Color(SerializableColor {
                r: 0.2,
                g: 0.6,
                b: 1.0,
                a: 0.6,
            }),
            tab: ParameterTab::Style,
            section: None,
            order: 2,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "fill_opacity".into(),
            label: "Fill Opacity".into(),
            description: "Opacity of the band fill".into(),
            kind: ParameterKind::Float {
                min: 0.0,
                max: 1.0,
                step: 0.05,
            },
            default: ParameterValue::Float(0.1),
            tab: ParameterTab::Style,
            section: None,
            order: 3,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
    ]
}

pub struct BollingerStudy {
    config: StudyConfig,
    output: StudyOutput,
    params: Vec<ParameterDef>,
}

impl BollingerStudy {
    pub fn new() -> Self {
        let params = make_params();
        let mut config = StudyConfig::new("bollinger");
        for p in &params {
            config.set(p.key.clone(), p.default.clone());
        }

        Self {
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
    fn id(&self) -> &str {
        "bollinger"
    }

    fn name(&self) -> &str {
        "Bollinger Bands"
    }

    fn category(&self) -> StudyCategory {
        StudyCategory::Volatility
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
        let period = self.config.get_int("period", 20) as usize;
        let std_mult = self.config.get_float("std_dev", 2.0);
        let upper_color = self.config.get_color(
            "upper_color",
            SerializableColor {
                r: 0.2,
                g: 0.6,
                b: 1.0,
                a: 0.6,
            },
        );
        let middle_color = self.config.get_color(
            "middle_color",
            SerializableColor {
                r: 0.2,
                g: 0.6,
                b: 1.0,
                a: 1.0,
            },
        );
        let lower_color = self.config.get_color(
            "lower_color",
            SerializableColor {
                r: 0.2,
                g: 0.6,
                b: 1.0,
                a: 0.6,
            },
        );
        let fill_opacity = self.config.get_float("fill_opacity", 0.1) as f32;

        let candles = input.candles;
        if candles.len() < period {
            self.output = StudyOutput::Empty;
            return Ok(());
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

            let mean = window.iter().sum::<f64>() / period as f64;

            let variance = window.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / period as f64;
            let stddev = variance.sqrt();

            let key = candle_key(&candles[i], i, candles.len(), &input.basis);
            let upper = (mean + std_mult * stddev) as f32;
            let lower = (mean - std_mult * stddev) as f32;
            let mid = mean as f32;

            upper_points.push((key, upper));
            middle_points.push((key, mid));
            lower_points.push((key, lower));
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
        Ok(())
    }

    fn output(&self) -> &StudyOutput {
        &self.output
    }

    fn reset(&mut self) {
        self.output = StudyOutput::Empty;
    }

    fn clone_study(&self) -> Box<dyn Study> {
        Box::new(BollingerStudy {
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
        let mut study = BollingerStudy::new();
        let input = make_input(&[]);
        study.compute(&input).unwrap();
        assert!(matches!(study.output(), StudyOutput::Empty));
    }

    #[test]
    fn test_bollinger_calculation() {
        let mut study = BollingerStudy::new();
        study
            .set_parameter("period", ParameterValue::Integer(3))
            .unwrap();
        study
            .set_parameter("std_dev", ParameterValue::Float(1.0))
            .unwrap();

        // Use constant values so stddev = 0
        let candles = vec![
            make_candle(1000, 100.0),
            make_candle(2000, 100.0),
            make_candle(3000, 100.0),
        ];
        let input = make_input(&candles);
        study.compute(&input).unwrap();

        let output = study.output();
        assert!(matches!(output, StudyOutput::Band { .. }), "expected Band output");
        let StudyOutput::Band {
            upper,
            middle,
            lower,
            ..
        } = output
        else {
            unreachable!()
        };
        assert_eq!(upper.points.len(), 1);
        assert_eq!(lower.points.len(), 1);
        let mid = middle.as_ref().unwrap();
        // All same price: mean = 100, stddev = 0
        assert!((mid.points[0].1 - 100.0).abs() < 0.01);
        assert!((upper.points[0].1 - 100.0).abs() < 0.01);
        assert!((lower.points[0].1 - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_bollinger_with_variance() {
        let mut study = BollingerStudy::new();
        study
            .set_parameter("period", ParameterValue::Integer(3))
            .unwrap();
        study
            .set_parameter("std_dev", ParameterValue::Float(2.0))
            .unwrap();

        let candles = vec![
            make_candle(1000, 10.0),
            make_candle(2000, 20.0),
            make_candle(3000, 30.0),
        ];
        let input = make_input(&candles);
        study.compute(&input).unwrap();

        let output = study.output();
        assert!(matches!(output, StudyOutput::Band { .. }), "expected Band output");
        let StudyOutput::Band {
            upper,
            middle,
            lower,
            ..
        } = output
        else {
            unreachable!()
        };
        let mid = middle.as_ref().unwrap();
        // mean = 20.0, variance = ((10-20)^2 + (20-20)^2 + (30-20)^2) / 3
        //       = (100 + 0 + 100) / 3 = 66.67, stddev ~ 8.165
        assert!((mid.points[0].1 - 20.0).abs() < 0.1);
        assert!(upper.points[0].1 > 35.0); // 20 + 2*8.165 ~ 36.33
        assert!(lower.points[0].1 < 5.0); // 20 - 2*8.165 ~ 3.67
    }

    #[test]
    fn test_set_parameter_valid() {
        let mut study = BollingerStudy::new();
        assert!(
            study
                .set_parameter("std_dev", ParameterValue::Float(3.0))
                .is_ok()
        );
    }

    #[test]
    fn test_set_parameter_invalid() {
        let mut study = BollingerStudy::new();
        assert!(
            study
                .set_parameter("std_dev", ParameterValue::Float(6.0))
                .is_err()
        );
        assert!(
            study
                .set_parameter("unknown", ParameterValue::Integer(1))
                .is_err()
        );
    }
}
