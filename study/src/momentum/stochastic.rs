use crate::config::{ParameterDef, ParameterKind, ParameterValue, StudyConfig};
use crate::error::StudyError;
use crate::output::{LineSeries, StudyOutput};
use crate::traits::{Study, StudyCategory, StudyInput, StudyPlacement};
use crate::trend::sma::candle_key;
use data::SerializableColor;

const DEFAULT_K_COLOR: SerializableColor = SerializableColor {
    r: 0.2,
    g: 0.6,
    b: 1.0,
    a: 1.0,
};

const DEFAULT_D_COLOR: SerializableColor = SerializableColor {
    r: 1.0,
    g: 0.4,
    b: 0.4,
    a: 1.0,
};

pub struct StochasticStudy {
    config: StudyConfig,
    output: StudyOutput,
    params: Vec<ParameterDef>,
}

impl StochasticStudy {
    pub fn new() -> Self {
        let params = vec![
            ParameterDef {
                key: "k_period",
                label: "%K Period",
                description: "Lookback period for %K calculation",
                kind: ParameterKind::Integer { min: 5, max: 50 },
                default: ParameterValue::Integer(14),
            },
            ParameterDef {
                key: "d_period",
                label: "%D Period",
                description: "Smoothing period for %D (signal line)",
                kind: ParameterKind::Integer { min: 1, max: 20 },
                default: ParameterValue::Integer(3),
            },
            ParameterDef {
                key: "smooth",
                label: "Smooth",
                description: "Smoothing period for %K",
                kind: ParameterKind::Integer { min: 1, max: 10 },
                default: ParameterValue::Integer(3),
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
                default: ParameterValue::Float(80.0),
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
                default: ParameterValue::Float(20.0),
            },
            ParameterDef {
                key: "k_color",
                label: "%K Color",
                description: "%K line color",
                kind: ParameterKind::Color,
                default: ParameterValue::Color(DEFAULT_K_COLOR),
            },
            ParameterDef {
                key: "d_color",
                label: "%D Color",
                description: "%D line color",
                kind: ParameterKind::Color,
                default: ParameterValue::Color(DEFAULT_D_COLOR),
            },
        ];

        let mut config = StudyConfig::new("stochastic");
        for p in &params {
            config.set(p.key, p.default.clone());
        }

        Self {
            config,
            output: StudyOutput::Empty,
            params,
        }
    }
}

impl Default for StochasticStudy {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute a simple moving average over a slice.
fn sma(values: &[f64], period: usize) -> Vec<f64> {
    if values.len() < period {
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

impl Study for StochasticStudy {
    fn id(&self) -> &str {
        "stochastic"
    }

    fn name(&self) -> &str {
        "Stochastic Oscillator"
    }

    fn category(&self) -> StudyCategory {
        StudyCategory::Momentum
    }

    fn placement(&self) -> StudyPlacement {
        StudyPlacement::Panel
    }

    fn parameters(&self) -> &[ParameterDef] {
        &self.params
    }

    fn config(&self) -> &StudyConfig {
        &self.config
    }

    fn set_parameter(&mut self, key: &str, value: ParameterValue) -> Result<(), StudyError> {
        if !self.params.iter().any(|p| p.key == key) {
            return Err(StudyError::InvalidParameter {
                key: key.to_string(),
                reason: "unknown parameter".to_string(),
            });
        }
        self.config.set(key, value);
        Ok(())
    }

    fn compute(&mut self, input: &StudyInput) -> Result<(), StudyError> {
        let k_period = self.config.get_int("k_period", 14) as usize;
        let d_period = self.config.get_int("d_period", 3) as usize;
        let smooth = self.config.get_int("smooth", 3) as usize;
        let k_color = self.config.get_color("k_color", DEFAULT_K_COLOR);
        let d_color = self.config.get_color("d_color", DEFAULT_D_COLOR);

        let candles = input.candles;
        if candles.len() < k_period {
            self.output = StudyOutput::Empty;
            return Ok(());
        }

        // Step 1: Compute raw %K (fast stochastic)
        let mut raw_k = Vec::with_capacity(candles.len() - k_period + 1);
        for i in (k_period - 1)..candles.len() {
            let window = &candles[(i + 1 - k_period)..=i];
            let lowest = window
                .iter()
                .map(|c| c.low.to_f32())
                .fold(f32::MAX, f32::min);
            let highest = window
                .iter()
                .map(|c| c.high.to_f32())
                .fold(f32::MIN, f32::max);

            let close = candles[i].close.to_f32();
            let range = highest - lowest;
            let k_val = if range > 0.0 {
                100.0 * (close - lowest) as f64 / range as f64
            } else {
                50.0 // Flat market, default to midpoint
            };
            raw_k.push(k_val);
        }

        // Step 2: Smooth %K with SMA
        let smoothed_k = if smooth > 1 {
            sma(&raw_k, smooth)
        } else {
            raw_k.clone()
        };

        if smoothed_k.is_empty() {
            self.output = StudyOutput::Empty;
            return Ok(());
        }

        // Step 3: %D = SMA of smoothed %K
        let d_values = sma(&smoothed_k, d_period);

        if d_values.is_empty() {
            self.output = StudyOutput::Empty;
            return Ok(());
        }

        // Build points for %K (aligned with %D)
        let k_offset = k_period - 1 + (smooth.max(1) - 1);
        let d_offset = k_offset + (d_period - 1);

        // %K starts at k_offset, %D starts at d_offset
        // We output %K from d_offset onward so both lines are aligned
        let k_start_in_smoothed = d_period - 1; // index into smoothed_k

        let mut k_points = Vec::with_capacity(d_values.len());
        let mut d_points = Vec::with_capacity(d_values.len());

        for (j, d_val) in d_values.iter().enumerate() {
            let candle_idx = d_offset + j;
            if candle_idx >= candles.len() {
                break;
            }
            let key = candle_key(&candles[candle_idx], candle_idx, candles.len(), &input.basis);
            let k_val = smoothed_k[k_start_in_smoothed + j];
            k_points.push((key, k_val as f32));
            d_points.push((key, *d_val as f32));
        }

        self.output = StudyOutput::Lines(vec![
            LineSeries {
                label: "%K".to_string(),
                color: k_color,
                width: 1.5,
                style: crate::config::LineStyleValue::Solid,
                points: k_points,
            },
            LineSeries {
                label: "%D".to_string(),
                color: d_color,
                width: 1.5,
                style: crate::config::LineStyleValue::Dashed,
                points: d_points,
            },
        ]);
        Ok(())
    }

    fn output(&self) -> &StudyOutput {
        &self.output
    }

    fn reset(&mut self) {
        self.output = StudyOutput::Empty;
    }

    fn clone_study(&self) -> Box<dyn Study> {
        Box::new(Self {
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

    fn make_candle(time: u64, high: f32, low: f32, close: f32) -> Candle {
        let open = (high + low) / 2.0;
        Candle::new(
            Timestamp(time),
            Price::from_f32(open),
            Price::from_f32(high),
            Price::from_f32(low),
            Price::from_f32(close),
            Volume(100.0),
            Volume(100.0),
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
    fn test_stochastic_empty() {
        let mut study = StochasticStudy::new();
        let input = make_input(&[]);
        study.compute(&input).unwrap();
        assert!(matches!(study.output(), StudyOutput::Empty));
    }

    #[test]
    fn test_stochastic_insufficient() {
        let mut study = StochasticStudy::new();
        // Default k_period=14, needs at least 14 candles
        let candles: Vec<Candle> = (0..5)
            .map(|i| make_candle(i as u64 * 60000, 105.0, 95.0, 100.0))
            .collect();
        let input = make_input(&candles);
        study.compute(&input).unwrap();
        assert!(matches!(study.output(), StudyOutput::Empty));
    }

    #[test]
    fn test_stochastic_calculation() {
        let mut study = StochasticStudy::new();
        study
            .set_parameter("k_period", ParameterValue::Integer(5))
            .unwrap();
        study
            .set_parameter("d_period", ParameterValue::Integer(3))
            .unwrap();
        study
            .set_parameter("smooth", ParameterValue::Integer(1))
            .unwrap();

        // Ascending prices
        let candles: Vec<Candle> = (0..10)
            .map(|i| {
                let base = 100.0 + i as f32 * 5.0;
                make_candle((i + 1) as u64 * 60000, base + 3.0, base - 3.0, base)
            })
            .collect();
        let input = make_input(&candles);
        study.compute(&input).unwrap();

        let output = study.output();
        assert!(matches!(output, StudyOutput::Lines(_)), "expected Lines output");
        let StudyOutput::Lines(lines) = output else { unreachable!() };
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].label, "%K");
        assert_eq!(lines[1].label, "%D");
        // In an uptrend, %K should be high (near 100)
        for pt in &lines[0].points {
            assert!(pt.1 > 50.0, "Expected %K > 50 in uptrend, got {}", pt.1);
        }
    }

    #[test]
    fn test_stochastic_range_bound() {
        let mut study = StochasticStudy::new();
        study
            .set_parameter("k_period", ParameterValue::Integer(5))
            .unwrap();
        study
            .set_parameter("d_period", ParameterValue::Integer(1))
            .unwrap();
        study
            .set_parameter("smooth", ParameterValue::Integer(1))
            .unwrap();

        // All candles identical - flat market
        let candles: Vec<Candle> = (0..10)
            .map(|i| make_candle(i as u64 * 60000, 105.0, 95.0, 100.0))
            .collect();
        let input = make_input(&candles);
        study.compute(&input).unwrap();

        let output = study.output();
        assert!(matches!(output, StudyOutput::Lines(_)), "expected Lines output");
        let StudyOutput::Lines(lines) = output else { unreachable!() };
        assert_eq!(lines.len(), 2);
        // With identical highs and lows, raw %K should be 50
        // (close - low) / (high - low) = (100-95)/(105-95) = 0.5 * 100 = 50
        for pt in &lines[0].points {
            assert!(
                (pt.1 - 50.0).abs() < 0.1,
                "Expected %K ~ 50.0, got {}",
                pt.1
            );
        }
    }

    #[test]
    fn test_stochastic_k_d_same_length() {
        let mut study = StochasticStudy::new();
        study
            .set_parameter("k_period", ParameterValue::Integer(5))
            .unwrap();
        study
            .set_parameter("d_period", ParameterValue::Integer(3))
            .unwrap();
        study
            .set_parameter("smooth", ParameterValue::Integer(3))
            .unwrap();

        let candles: Vec<Candle> = (0..20)
            .map(|i| {
                let base = 100.0 + (i as f32 * 2.0).sin() * 10.0;
                make_candle(i as u64 * 60000, base + 3.0, base - 3.0, base)
            })
            .collect();
        let input = make_input(&candles);
        study.compute(&input).unwrap();

        let output = study.output();
        assert!(matches!(output, StudyOutput::Lines(_)), "expected Lines output");
        let StudyOutput::Lines(lines) = output else { unreachable!() };
        assert_eq!(lines.len(), 2);
        // %K and %D should have the same number of points
        assert_eq!(lines[0].points.len(), lines[1].points.len());
        // And the x-values should match
        for (k, d) in lines[0].points.iter().zip(lines[1].points.iter()) {
            assert_eq!(k.0, d.0);
        }
    }
}
