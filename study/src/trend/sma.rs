use crate::config::{ParameterDef, ParameterKind, ParameterValue, StudyConfig};
use crate::error::StudyError;
use crate::output::{LineSeries, StudyOutput};
use crate::traits::{Study, StudyCategory, StudyInput, StudyPlacement};
use data::{ChartBasis, SerializableColor};

const SOURCE_OPTIONS: &[&str] = &["Close", "Open", "High", "Low", "HL2", "HLC3", "OHLC4"];

const PARAMS: &[ParameterDef] = &[
    ParameterDef {
        key: "period",
        label: "Period",
        description: "Number of candles for the moving average",
        kind: ParameterKind::Integer { min: 2, max: 500 },
        default: ParameterValue::Integer(20),
    },
    ParameterDef {
        key: "color",
        label: "Color",
        description: "Line color",
        kind: ParameterKind::Color,
        default: ParameterValue::Color(SerializableColor {
            r: 0.2,
            g: 0.6,
            b: 1.0,
            a: 1.0,
        }),
    },
    ParameterDef {
        key: "width",
        label: "Width",
        description: "Line width",
        kind: ParameterKind::Float {
            min: 0.5,
            max: 5.0,
            step: 0.5,
        },
        default: ParameterValue::Float(1.5),
    },
    ParameterDef {
        key: "source",
        label: "Source",
        description: "Price source for calculation",
        kind: ParameterKind::Choice {
            options: SOURCE_OPTIONS,
        },
        default: ParameterValue::Choice(String::new()), // overridden in new()
    },
];

pub struct SmaStudy {
    config: StudyConfig,
    output: StudyOutput,
}

impl SmaStudy {
    pub fn new() -> Self {
        let mut config = StudyConfig::new("sma");
        for p in PARAMS {
            config.set(p.key, p.default.clone());
        }
        // Set the actual default for source (can't use String::new in const)
        config.set("source", ParameterValue::Choice("Close".to_string()));

        Self {
            config,
            output: StudyOutput::Empty,
        }
    }
}

impl Default for SmaStudy {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract price value from a candle based on the source parameter.
pub(crate) fn source_value(candle: &data::Candle, source: &str) -> f32 {
    match source {
        "Open" => candle.open.to_f32(),
        "High" => candle.high.to_f32(),
        "Low" => candle.low.to_f32(),
        "HL2" => (candle.high.to_f32() + candle.low.to_f32()) / 2.0,
        "HLC3" => (candle.high.to_f32() + candle.low.to_f32() + candle.close.to_f32()) / 3.0,
        "OHLC4" => {
            (candle.open.to_f32()
                + candle.high.to_f32()
                + candle.low.to_f32()
                + candle.close.to_f32())
                / 4.0
        }
        _ => candle.close.to_f32(),
    }
}

/// Get the x-key for a candle based on chart basis.
pub(crate) fn candle_key(candle: &data::Candle, index: usize, basis: &ChartBasis) -> u64 {
    match basis {
        ChartBasis::Time(_) => candle.time.0,
        ChartBasis::Tick(_) => index as u64,
    }
}

impl Study for SmaStudy {
    fn id(&self) -> &str {
        "sma"
    }

    fn name(&self) -> &str {
        "Simple Moving Average"
    }

    fn category(&self) -> StudyCategory {
        StudyCategory::Trend
    }

    fn placement(&self) -> StudyPlacement {
        StudyPlacement::Overlay
    }

    fn parameters(&self) -> &[ParameterDef] {
        PARAMS
    }

    fn config(&self) -> &StudyConfig {
        &self.config
    }

    fn set_parameter(&mut self, key: &str, value: ParameterValue) -> Result<(), StudyError> {
        match key {
            "period" => {
                if let ParameterValue::Integer(v) = &value {
                    if *v < 2 || *v > 500 {
                        return Err(StudyError::InvalidParameter {
                            key: key.to_string(),
                            reason: "period must be between 2 and 500".to_string(),
                        });
                    }
                } else {
                    return Err(StudyError::InvalidParameter {
                        key: key.to_string(),
                        reason: "expected integer".to_string(),
                    });
                }
            }
            "color" => {
                if !matches!(&value, ParameterValue::Color(_)) {
                    return Err(StudyError::InvalidParameter {
                        key: key.to_string(),
                        reason: "expected color".to_string(),
                    });
                }
            }
            "width" => {
                if let ParameterValue::Float(v) = &value {
                    if *v < 0.5 || *v > 5.0 {
                        return Err(StudyError::InvalidParameter {
                            key: key.to_string(),
                            reason: "width must be between 0.5 and 5.0".to_string(),
                        });
                    }
                } else {
                    return Err(StudyError::InvalidParameter {
                        key: key.to_string(),
                        reason: "expected float".to_string(),
                    });
                }
            }
            "source" => {
                if let ParameterValue::Choice(s) = &value {
                    if !SOURCE_OPTIONS.contains(&s.as_str()) {
                        return Err(StudyError::InvalidParameter {
                            key: key.to_string(),
                            reason: format!("invalid source: {s}"),
                        });
                    }
                } else {
                    return Err(StudyError::InvalidParameter {
                        key: key.to_string(),
                        reason: "expected choice".to_string(),
                    });
                }
            }
            _ => {
                return Err(StudyError::InvalidParameter {
                    key: key.to_string(),
                    reason: "unknown parameter".to_string(),
                });
            }
        }
        self.config.set(key, value);
        Ok(())
    }

    fn compute(&mut self, input: &StudyInput) {
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
            self.output = StudyOutput::Empty;
            return;
        }

        let mut points = Vec::with_capacity(candles.len() - period + 1);

        // Calculate initial sum
        let mut sum: f64 = 0.0;
        for candle in &candles[..period] {
            sum += source_value(candle, &source) as f64;
        }
        points.push((
            candle_key(&candles[period - 1], period - 1, &input.basis),
            (sum / period as f64) as f32,
        ));

        // Sliding window
        for i in period..candles.len() {
            sum += source_value(&candles[i], &source) as f64;
            sum -= source_value(&candles[i - period], &source) as f64;
            points.push((
                candle_key(&candles[i], i, &input.basis),
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
    }

    fn output(&self) -> &StudyOutput {
        &self.output
    }

    fn reset(&mut self) {
        self.output = StudyOutput::Empty;
    }

    fn clone_study(&self) -> Box<dyn Study> {
        Box::new(SmaStudy {
            config: self.config.clone(),
            output: self.output.clone(),
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
        let mut study = SmaStudy::new();
        let input = make_input(&[]);
        study.compute(&input);
        assert!(matches!(study.output(), StudyOutput::Empty));
    }

    #[test]
    fn test_insufficient_candles() {
        let mut study = SmaStudy::new();
        // Default period is 20, so 5 candles is insufficient
        let candles: Vec<Candle> = (0..5).map(|i| make_candle(i * 60000, 100.0)).collect();
        let input = make_input(&candles);
        study.compute(&input);
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
        study.compute(&input);

        if let StudyOutput::Lines(lines) = study.output() {
            assert_eq!(lines.len(), 1);
            let points = &lines[0].points;
            assert_eq!(points.len(), 3);
            // SMA(3) of [10, 20, 30] = 20.0
            assert!((points[0].1 - 20.0).abs() < 0.01);
            // SMA(3) of [20, 30, 40] = 30.0
            assert!((points[1].1 - 30.0).abs() < 0.01);
            // SMA(3) of [30, 40, 50] = 40.0
            assert!((points[2].1 - 40.0).abs() < 0.01);
        } else {
            panic!("expected Lines output");
        }
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
