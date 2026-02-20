use crate::config::{LineStyleValue, ParameterDef, ParameterKind, ParameterValue, StudyConfig};
use crate::error::StudyError;
use crate::output::{LineSeries, StudyOutput};
use crate::traits::{Study, StudyCategory, StudyInput, StudyPlacement};
use crate::trend::sma::candle_key;
use data::SerializableColor;

const PARAMS: &[ParameterDef] = &[
    ParameterDef {
        key: "period",
        label: "Period",
        description: "RSI lookback period",
        kind: ParameterKind::Integer { min: 2, max: 100 },
        default: ParameterValue::Integer(14),
    },
    ParameterDef {
        key: "overbought",
        label: "Overbought",
        description: "Overbought level",
        kind: ParameterKind::Float {
            min: 50.0,
            max: 100.0,
            step: 5.0,
        },
        default: ParameterValue::Float(70.0),
    },
    ParameterDef {
        key: "oversold",
        label: "Oversold",
        description: "Oversold level",
        kind: ParameterKind::Float {
            min: 0.0,
            max: 50.0,
            step: 5.0,
        },
        default: ParameterValue::Float(30.0),
    },
    ParameterDef {
        key: "color",
        label: "Color",
        description: "RSI line color",
        kind: ParameterKind::Color,
        default: ParameterValue::Color(SerializableColor {
            r: 1.0,
            g: 0.85,
            b: 0.2,
            a: 1.0,
        }),
    },
];

pub struct RsiStudy {
    config: StudyConfig,
    output: StudyOutput,
}

impl RsiStudy {
    pub fn new() -> Self {
        let mut config = StudyConfig::new("rsi");
        for p in PARAMS {
            config.set(p.key, p.default.clone());
        }

        Self {
            config,
            output: StudyOutput::Empty,
        }
    }
}

impl Default for RsiStudy {
    fn default() -> Self {
        Self::new()
    }
}

impl Study for RsiStudy {
    fn id(&self) -> &str {
        "rsi"
    }

    fn name(&self) -> &str {
        "Relative Strength Index"
    }

    fn category(&self) -> StudyCategory {
        StudyCategory::Momentum
    }

    fn placement(&self) -> StudyPlacement {
        StudyPlacement::Panel
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
                    if *v < 2 || *v > 100 {
                        return Err(StudyError::InvalidParameter {
                            key: key.to_string(),
                            reason: "period must be between 2 and 100".to_string(),
                        });
                    }
                } else {
                    return Err(StudyError::InvalidParameter {
                        key: key.to_string(),
                        reason: "expected integer".to_string(),
                    });
                }
            }
            "overbought" => {
                if let ParameterValue::Float(v) = &value {
                    if *v < 50.0 || *v > 100.0 {
                        return Err(StudyError::InvalidParameter {
                            key: key.to_string(),
                            reason: "overbought must be between 50 and 100".to_string(),
                        });
                    }
                } else {
                    return Err(StudyError::InvalidParameter {
                        key: key.to_string(),
                        reason: "expected float".to_string(),
                    });
                }
            }
            "oversold" => {
                if let ParameterValue::Float(v) = &value {
                    if *v < 0.0 || *v > 50.0 {
                        return Err(StudyError::InvalidParameter {
                            key: key.to_string(),
                            reason: "oversold must be between 0 and 50".to_string(),
                        });
                    }
                } else {
                    return Err(StudyError::InvalidParameter {
                        key: key.to_string(),
                        reason: "expected float".to_string(),
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

    fn compute(&mut self, input: &StudyInput) -> Result<(), StudyError> {
        let period = self.config.get_int("period", 14) as usize;
        let color = self.config.get_color(
            "color",
            SerializableColor {
                r: 1.0,
                g: 0.85,
                b: 0.2,
                a: 1.0,
            },
        );

        let candles = input.candles;
        // Need at least period + 1 candles to compute one RSI value
        if candles.len() < period + 1 {
            self.output = StudyOutput::Empty;
            return Ok(());
        }

        let closes: Vec<f64> = candles.iter().map(|c| c.close.to_f64()).collect();

        // Calculate initial average gain and loss from first `period` changes
        let mut avg_gain: f64 = 0.0;
        let mut avg_loss: f64 = 0.0;
        for i in 1..=period {
            let change = closes[i] - closes[i - 1];
            if change > 0.0 {
                avg_gain += change;
            } else {
                avg_loss += -change;
            }
        }
        avg_gain /= period as f64;
        avg_loss /= period as f64;

        let mut points = Vec::with_capacity(candles.len() - period);

        // First RSI value
        let rsi = if avg_loss == 0.0 {
            100.0
        } else {
            100.0 - 100.0 / (1.0 + avg_gain / avg_loss)
        };
        points.push((
            candle_key(&candles[period], period, candles.len(), &input.basis),
            rsi as f32,
        ));

        // Wilder's smoothing for subsequent values
        for i in (period + 1)..candles.len() {
            let change = closes[i] - closes[i - 1];
            let (gain, loss) = if change > 0.0 {
                (change, 0.0)
            } else {
                (0.0, -change)
            };

            avg_gain = (avg_gain * (period as f64 - 1.0) + gain) / period as f64;
            avg_loss = (avg_loss * (period as f64 - 1.0) + loss) / period as f64;

            let rsi = if avg_loss == 0.0 {
                100.0
            } else {
                100.0 - 100.0 / (1.0 + avg_gain / avg_loss)
            };
            points.push((candle_key(&candles[i], i, candles.len(), &input.basis), rsi as f32));
        }

        self.output = StudyOutput::Lines(vec![LineSeries {
            label: format!("RSI({})", period),
            color,
            width: 1.5,
            style: LineStyleValue::Solid,
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
        Box::new(RsiStudy {
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
        let mut study = RsiStudy::new();
        let input = make_input(&[]);
        study.compute(&input).unwrap();
        assert!(matches!(study.output(), StudyOutput::Empty));
    }

    #[test]
    fn test_insufficient_candles() {
        let mut study = RsiStudy::new();
        let candles: Vec<Candle> = (0..10).map(|i| make_candle(i * 60000, 100.0)).collect();
        let input = make_input(&candles);
        study.compute(&input).unwrap();
        assert!(matches!(study.output(), StudyOutput::Empty));
    }

    #[test]
    fn test_rsi_all_gains() {
        let mut study = RsiStudy::new();
        study
            .set_parameter("period", ParameterValue::Integer(3))
            .unwrap();

        // Strictly increasing prices: RSI should be 100
        let candles = vec![
            make_candle(1000, 10.0),
            make_candle(2000, 20.0),
            make_candle(3000, 30.0),
            make_candle(4000, 40.0),
        ];
        let input = make_input(&candles);
        study.compute(&input).unwrap();

        let output = study.output();
        assert!(matches!(output, StudyOutput::Lines(_)), "expected Lines output");
        let StudyOutput::Lines(lines) = output else { unreachable!() };
        assert_eq!(lines[0].points.len(), 1);
        assert!((lines[0].points[0].1 - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_rsi_all_losses() {
        let mut study = RsiStudy::new();
        study
            .set_parameter("period", ParameterValue::Integer(3))
            .unwrap();

        // Strictly decreasing prices: RSI should be 0
        let candles = vec![
            make_candle(1000, 40.0),
            make_candle(2000, 30.0),
            make_candle(3000, 20.0),
            make_candle(4000, 10.0),
        ];
        let input = make_input(&candles);
        study.compute(&input).unwrap();

        let output = study.output();
        assert!(matches!(output, StudyOutput::Lines(_)), "expected Lines output");
        let StudyOutput::Lines(lines) = output else { unreachable!() };
        assert_eq!(lines[0].points.len(), 1);
        assert!(lines[0].points[0].1.abs() < 0.01);
    }

    #[test]
    fn test_rsi_range() {
        let mut study = RsiStudy::new();
        study
            .set_parameter("period", ParameterValue::Integer(3))
            .unwrap();

        let candles = vec![
            make_candle(1000, 44.0),
            make_candle(2000, 44.25),
            make_candle(3000, 44.5),
            make_candle(4000, 43.75),
            make_candle(5000, 44.5),
            make_candle(6000, 44.25),
        ];
        let input = make_input(&candles);
        study.compute(&input).unwrap();

        let output = study.output();
        assert!(matches!(output, StudyOutput::Lines(_)), "expected Lines output");
        let StudyOutput::Lines(lines) = output else { unreachable!() };
        for point in &lines[0].points {
            assert!(point.1 >= 0.0 && point.1 <= 100.0);
        }
    }

    #[test]
    fn test_set_parameter_valid() {
        let mut study = RsiStudy::new();
        assert!(
            study
                .set_parameter("period", ParameterValue::Integer(21))
                .is_ok()
        );
        assert!(
            study
                .set_parameter("overbought", ParameterValue::Float(80.0))
                .is_ok()
        );
    }

    #[test]
    fn test_set_parameter_invalid() {
        let mut study = RsiStudy::new();
        assert!(
            study
                .set_parameter("period", ParameterValue::Integer(1))
                .is_err()
        );
        assert!(
            study
                .set_parameter("period", ParameterValue::Integer(101))
                .is_err()
        );
        assert!(
            study
                .set_parameter("overbought", ParameterValue::Float(40.0))
                .is_err()
        );
        assert!(
            study
                .set_parameter("unknown", ParameterValue::Integer(1))
                .is_err()
        );
    }
}
