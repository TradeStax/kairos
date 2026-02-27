//! Relative Strength Index (RSI).
//!
//! RSI measures the speed and magnitude of recent price changes to evaluate
//! overbought or oversold conditions. It oscillates between 0 and 100.
//!
//! Uses Wilder's smoothed moving average: after seeding from the first
//! `period` price changes, each subsequent value blends the previous average
//! with the current gain or loss using the factor `(period - 1) / period`.
//!
//! Traders typically watch for readings above 70 (overbought) or below 30
//! (oversold), divergences between RSI and price, and centerline (50)
//! crossovers for trend confirmation.
//!
//! Output: `StudyOutput::Composite` containing a line series and two
//! horizontal levels (overbought / oversold).

use crate::config::{
    DisplayFormat, LineStyleValue, ParameterDef, ParameterKind, ParameterTab,
    ParameterValue, StudyConfig, Visibility,
};
use crate::core::{Study, StudyCategory, StudyInput, StudyPlacement};
use crate::error::StudyError;
use crate::output::{LineSeries, PriceLevel, StudyOutput};
use crate::util::candle_key;
use data::SerializableColor;

/// Build the default parameter definitions for the RSI study.
fn make_params() -> Vec<ParameterDef> {
    vec![
        ParameterDef {
            key: "period".into(),
            label: "Period".into(),
            description: "RSI lookback period".into(),
            kind: ParameterKind::Integer { min: 2, max: 100 },
            default: ParameterValue::Integer(14),
            tab: ParameterTab::Parameters,
            section: None,
            order: 0,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "overbought".into(),
            label: "Overbought".into(),
            description: "Overbought level".into(),
            kind: ParameterKind::Float {
                min: 50.0,
                max: 100.0,
                step: 5.0,
            },
            default: ParameterValue::Float(70.0),
            tab: ParameterTab::Parameters,
            section: None,
            order: 1,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "oversold".into(),
            label: "Oversold".into(),
            description: "Oversold level".into(),
            kind: ParameterKind::Float {
                min: 0.0,
                max: 50.0,
                step: 5.0,
            },
            default: ParameterValue::Float(30.0),
            tab: ParameterTab::Parameters,
            section: None,
            order: 2,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "color".into(),
            label: "Color".into(),
            description: "RSI line color".into(),
            kind: ParameterKind::Color,
            default: ParameterValue::Color(SerializableColor {
                r: 1.0,
                g: 0.85,
                b: 0.2,
                a: 1.0,
            }),
            tab: ParameterTab::Style,
            section: None,
            order: 0,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
    ]
}

/// Relative Strength Index oscillator.
///
/// Computes the ratio of average gains to average losses over the
/// configured lookback period using Wilder's smoothing, then maps the
/// result to a 0--100 scale. Renders as a single line in a separate
/// panel with configurable overbought and oversold reference levels.
pub struct RsiStudy {
    config: StudyConfig,
    output: StudyOutput,
    params: Vec<ParameterDef>,
}

impl RsiStudy {
    /// Create a new RSI study with the standard default parameters:
    /// 14-period lookback, overbought level at 70, oversold level at 30.
    pub fn new() -> Self {
        let params = make_params();
        let mut config = StudyConfig::new("rsi");
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
        &self.params
    }

    fn config(&self) -> &StudyConfig {
        &self.config
    }

    fn config_mut(&mut self) -> &mut StudyConfig {
        &mut self.config
    }

    fn compute(&mut self, input: &StudyInput) -> Result<(), StudyError> {
        let period = self.config.get_int("period", 14) as usize;
        let ob = self.config.get_float("overbought", 70.0);
        let os = self.config.get_float("oversold", 30.0);
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
            log::debug!(
                "{}: insufficient data ({} candles, need {})",
                self.id(),
                candles.len(),
                period + 1
            );
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
            points.push((
                candle_key(&candles[i], i, candles.len(), &input.basis),
                rsi as f32,
            ));
        }

        let level_color = SerializableColor {
            r: 0.7,
            g: 0.7,
            b: 0.7,
            a: 0.6,
        };
        let levels = vec![
            PriceLevel {
                price: ob,
                label: format!("OB {ob}"),
                color: level_color,
                style: LineStyleValue::Dashed,
                opacity: 0.6,
                show_label: true,
                fill_above: None,
                fill_below: None,
                width: 1.0,
                start_x: None,
                zone_half_width: None,
            },
            PriceLevel {
                price: os,
                label: format!("OS {os}"),
                color: level_color,
                style: LineStyleValue::Dashed,
                opacity: 0.6,
                show_label: true,
                fill_above: None,
                fill_below: None,
                width: 1.0,
                start_x: None,
                zone_half_width: None,
            },
        ];

        self.output = StudyOutput::Composite(vec![
            StudyOutput::Lines(vec![LineSeries {
                label: format!("RSI({})", period),
                color,
                width: 1.5,
                style: LineStyleValue::Solid,
                points,
            }]),
            StudyOutput::Levels(levels),
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
        Box::new(RsiStudy {
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

    /// Extract the RSI line series from the Composite output.
    fn extract_lines(output: &StudyOutput) -> &[LineSeries] {
        let StudyOutput::Composite(parts) = output else {
            panic!("expected Composite output, got {:?}", output);
        };
        let StudyOutput::Lines(lines) = &parts[0] else {
            panic!("expected Lines as first Composite element");
        };
        lines
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

        let lines = extract_lines(study.output());
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

        let lines = extract_lines(study.output());
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

        let lines = extract_lines(study.output());
        for point in &lines[0].points {
            assert!(point.1 >= 0.0 && point.1 <= 100.0);
        }
    }

    #[test]
    fn test_rsi_levels_in_output() {
        let mut study = RsiStudy::new();
        study
            .set_parameter("period", ParameterValue::Integer(3))
            .unwrap();

        let candles = vec![
            make_candle(1000, 10.0),
            make_candle(2000, 20.0),
            make_candle(3000, 30.0),
            make_candle(4000, 40.0),
        ];
        let input = make_input(&candles);
        study.compute(&input).unwrap();

        let StudyOutput::Composite(parts) = study.output() else {
            panic!("expected Composite output");
        };
        assert_eq!(parts.len(), 2);
        assert!(matches!(&parts[0], StudyOutput::Lines(_)));
        let StudyOutput::Levels(levels) = &parts[1] else {
            panic!("expected Levels as second element");
        };
        assert_eq!(levels.len(), 2);
        assert!((levels[0].price - 70.0).abs() < 0.001);
        assert!((levels[1].price - 30.0).abs() < 0.001);
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
