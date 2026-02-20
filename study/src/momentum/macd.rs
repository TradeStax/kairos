use crate::config::{LineStyleValue, ParameterDef, ParameterKind, ParameterValue, StudyConfig};
use crate::error::StudyError;
use crate::output::{HistogramBar, LineSeries, StudyOutput};
use crate::traits::{Study, StudyCategory, StudyInput, StudyPlacement};
use crate::trend::sma::candle_key;
use data::SerializableColor;

const PARAMS: &[ParameterDef] = &[
    ParameterDef {
        key: "fast_period",
        label: "Fast Period",
        description: "Fast EMA period",
        kind: ParameterKind::Integer { min: 2, max: 100 },
        default: ParameterValue::Integer(12),
    },
    ParameterDef {
        key: "slow_period",
        label: "Slow Period",
        description: "Slow EMA period",
        kind: ParameterKind::Integer { min: 2, max: 200 },
        default: ParameterValue::Integer(26),
    },
    ParameterDef {
        key: "signal_period",
        label: "Signal Period",
        description: "Signal line EMA period",
        kind: ParameterKind::Integer { min: 2, max: 100 },
        default: ParameterValue::Integer(9),
    },
    ParameterDef {
        key: "macd_color",
        label: "MACD Color",
        description: "MACD line color",
        kind: ParameterKind::Color,
        default: ParameterValue::Color(SerializableColor {
            r: 0.2,
            g: 0.6,
            b: 1.0,
            a: 1.0,
        }),
    },
    ParameterDef {
        key: "signal_color",
        label: "Signal Color",
        description: "Signal line color",
        kind: ParameterKind::Color,
        default: ParameterValue::Color(SerializableColor {
            r: 1.0,
            g: 0.6,
            b: 0.2,
            a: 1.0,
        }),
    },
    ParameterDef {
        key: "hist_positive_color",
        label: "Histogram +",
        description: "Histogram positive color",
        kind: ParameterKind::Color,
        default: ParameterValue::Color(SerializableColor {
            r: 0.2,
            g: 0.8,
            b: 0.4,
            a: 0.7,
        }),
    },
    ParameterDef {
        key: "hist_negative_color",
        label: "Histogram -",
        description: "Histogram negative color",
        kind: ParameterKind::Color,
        default: ParameterValue::Color(SerializableColor {
            r: 0.9,
            g: 0.2,
            b: 0.2,
            a: 0.7,
        }),
    },
];

pub struct MacdStudy {
    config: StudyConfig,
    output: StudyOutput,
    /// Histogram output (rendered alongside lines by the chart layer)
    pub histogram: Vec<HistogramBar>,
}

impl MacdStudy {
    pub fn new() -> Self {
        let mut config = StudyConfig::new("macd");
        for p in PARAMS {
            config.set(p.key, p.default.clone());
        }

        Self {
            config,
            output: StudyOutput::Empty,
            histogram: Vec::new(),
        }
    }
}

impl Default for MacdStudy {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute EMA values from a slice of f64 values.
/// Returns EMA values starting from index `period - 1`.
fn compute_ema(values: &[f64], period: usize) -> Vec<f64> {
    if values.len() < period {
        return vec![];
    }

    let multiplier = 2.0 / (period + 1) as f64;
    let mut result = Vec::with_capacity(values.len() - period + 1);

    // Seed with SMA
    let sma: f64 = values[..period].iter().sum::<f64>() / period as f64;
    result.push(sma);

    // Apply EMA formula
    for &val in &values[period..] {
        let prev = *result.last().unwrap();
        result.push(val * multiplier + prev * (1.0 - multiplier));
    }

    result
}

impl Study for MacdStudy {
    fn id(&self) -> &str {
        "macd"
    }

    fn name(&self) -> &str {
        "MACD"
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
            "fast_period" => {
                if let ParameterValue::Integer(v) = &value {
                    if *v < 2 || *v > 100 {
                        return Err(StudyError::InvalidParameter {
                            key: key.to_string(),
                            reason: "fast_period must be between 2 and 100".to_string(),
                        });
                    }
                } else {
                    return Err(StudyError::InvalidParameter {
                        key: key.to_string(),
                        reason: "expected integer".to_string(),
                    });
                }
            }
            "slow_period" => {
                if let ParameterValue::Integer(v) = &value {
                    if *v < 2 || *v > 200 {
                        return Err(StudyError::InvalidParameter {
                            key: key.to_string(),
                            reason: "slow_period must be between 2 and 200".to_string(),
                        });
                    }
                } else {
                    return Err(StudyError::InvalidParameter {
                        key: key.to_string(),
                        reason: "expected integer".to_string(),
                    });
                }
            }
            "signal_period" => {
                if let ParameterValue::Integer(v) = &value {
                    if *v < 2 || *v > 100 {
                        return Err(StudyError::InvalidParameter {
                            key: key.to_string(),
                            reason: "signal_period must be between 2 and 100".to_string(),
                        });
                    }
                } else {
                    return Err(StudyError::InvalidParameter {
                        key: key.to_string(),
                        reason: "expected integer".to_string(),
                    });
                }
            }
            "macd_color" | "signal_color" | "hist_positive_color" | "hist_negative_color" => {
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

    fn compute(&mut self, input: &StudyInput) {
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
        let hist_pos_color = self.config.get_color(
            "hist_positive_color",
            SerializableColor {
                r: 0.2,
                g: 0.8,
                b: 0.4,
                a: 0.7,
            },
        );
        let hist_neg_color = self.config.get_color(
            "hist_negative_color",
            SerializableColor {
                r: 0.9,
                g: 0.2,
                b: 0.2,
                a: 0.7,
            },
        );
        let candles = input.candles;
        let max_period = fast_period.max(slow_period);
        if candles.len() < max_period {
            self.output = StudyOutput::Empty;
            return;
        }

        let closes: Vec<f64> = candles.iter().map(|c| c.close.to_f64()).collect();

        // Compute fast and slow EMAs
        let fast_ema = compute_ema(&closes, fast_period);
        let slow_ema = compute_ema(&closes, slow_period);

        // Align fast and slow EMAs
        // fast_ema starts at index (fast_period - 1)
        // slow_ema starts at index (slow_period - 1)
        // MACD line starts at index (slow_period - 1) in candle space
        let fast_offset = slow_period.saturating_sub(fast_period);
        let macd_len = fast_ema
            .len()
            .saturating_sub(fast_offset)
            .min(slow_ema.len());
        if macd_len == 0 {
            self.output = StudyOutput::Empty;
            return;
        }

        let macd_values: Vec<f64> = (0..macd_len)
            .map(|i| fast_ema[i + fast_offset] - slow_ema[i])
            .collect();

        // Compute signal line (EMA of MACD values)
        let signal_ema = compute_ema(&macd_values, signal_period);
        if signal_ema.is_empty() {
            self.output = StudyOutput::Empty;
            return;
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
            let key = candle_key(&candles[candle_idx], candle_idx, &input.basis);
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

        self.histogram = histogram;
        self.output = StudyOutput::Lines(vec![
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
        ]);
    }

    fn output(&self) -> &StudyOutput {
        &self.output
    }

    fn reset(&mut self) {
        self.output = StudyOutput::Empty;
        self.histogram.clear();
    }

    fn clone_study(&self) -> Box<dyn Study> {
        Box::new(MacdStudy {
            config: self.config.clone(),
            output: self.output.clone(),
            histogram: self.histogram.clone(),
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
        let mut study = MacdStudy::new();
        let input = make_input(&[]);
        study.compute(&input);
        assert!(matches!(study.output(), StudyOutput::Empty));
    }

    #[test]
    fn test_insufficient_candles() {
        let mut study = MacdStudy::new();
        let candles: Vec<Candle> = (0..20).map(|i| make_candle(i * 60000, 100.0)).collect();
        let input = make_input(&candles);
        study.compute(&input);
        assert!(matches!(study.output(), StudyOutput::Empty));
    }

    #[test]
    fn test_macd_constant_price() {
        let mut study = MacdStudy::new();
        study
            .set_parameter("fast_period", ParameterValue::Integer(3))
            .unwrap();
        study
            .set_parameter("slow_period", ParameterValue::Integer(5))
            .unwrap();
        study
            .set_parameter("signal_period", ParameterValue::Integer(2))
            .unwrap();

        // With constant prices, MACD should be ~0
        let candles: Vec<Candle> = (0..20).map(|i| make_candle(i * 60000, 100.0)).collect();
        let input = make_input(&candles);
        study.compute(&input);

        if let StudyOutput::Lines(lines) = study.output() {
            assert_eq!(lines.len(), 2);
            // MACD line should be near 0 for constant prices
            for point in &lines[0].points {
                assert!(point.1.abs() < 0.01, "MACD should be ~0, got {}", point.1);
            }
        } else {
            panic!("expected Lines output");
        }
    }

    #[test]
    fn test_macd_trending_price() {
        let mut study = MacdStudy::new();
        study
            .set_parameter("fast_period", ParameterValue::Integer(3))
            .unwrap();
        study
            .set_parameter("slow_period", ParameterValue::Integer(5))
            .unwrap();
        study
            .set_parameter("signal_period", ParameterValue::Integer(2))
            .unwrap();

        // Rising prices: fast EMA > slow EMA, so MACD > 0
        let candles: Vec<Candle> = (0..20)
            .map(|i| make_candle(i * 60000, 100.0 + i as f32 * 10.0))
            .collect();
        let input = make_input(&candles);
        study.compute(&input);

        if let StudyOutput::Lines(lines) = study.output() {
            assert_eq!(lines.len(), 2);
            // In a strong uptrend, MACD should be positive
            for point in &lines[0].points {
                assert!(point.1 > 0.0, "MACD should be positive in uptrend");
            }
        } else {
            panic!("expected Lines output");
        }
    }

    #[test]
    fn test_set_parameter_valid() {
        let mut study = MacdStudy::new();
        assert!(
            study
                .set_parameter("fast_period", ParameterValue::Integer(8))
                .is_ok()
        );
        assert!(
            study
                .set_parameter("slow_period", ParameterValue::Integer(21))
                .is_ok()
        );
        assert!(
            study
                .set_parameter("signal_period", ParameterValue::Integer(5))
                .is_ok()
        );
    }

    #[test]
    fn test_set_parameter_invalid() {
        let mut study = MacdStudy::new();
        assert!(
            study
                .set_parameter("fast_period", ParameterValue::Integer(1))
                .is_err()
        );
        assert!(
            study
                .set_parameter("slow_period", ParameterValue::Integer(201))
                .is_err()
        );
        assert!(
            study
                .set_parameter("unknown", ParameterValue::Integer(5))
                .is_err()
        );
    }
}
