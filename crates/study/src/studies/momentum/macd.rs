//! Moving Average Convergence Divergence (MACD).
//!
//! MACD tracks the relationship between two exponential moving averages
//! of closing prices. It consists of three components:
//!
//! - **MACD line** — difference between the fast and slow EMAs.
//! - **Signal line** — EMA of the MACD line itself.
//! - **Histogram** — MACD minus Signal, visualising momentum shifts.
//!
//! Traders use MACD crossovers (MACD crossing the signal line), histogram
//! direction changes, and divergences from price to gauge trend momentum
//! and potential reversals.
//!
//! Output: `StudyOutput::Composite` containing two line series (MACD and
//! Signal) and a histogram.

use crate::config::{
    DisplayFormat, LineStyleValue, ParameterDef, ParameterKind, ParameterTab,
    ParameterValue, StudyConfig, Visibility,
};
use crate::core::{Study, StudyCategory, StudyInput, StudyPlacement};
use crate::error::StudyError;
use crate::output::{HistogramBar, LineSeries, StudyOutput};
use crate::studies::trend::ema::compute_ema;
use crate::util::candle_key;
use crate::{BEARISH_COLOR, BULLISH_COLOR};
use data::SerializableColor;

/// Build the default parameter definitions for the MACD study.
///
/// Defines three period parameters (fast, slow, signal), two line
/// colors (MACD line, signal line), and two histogram colors
/// (positive/negative divergence).
fn make_params() -> Vec<ParameterDef> {
    vec![
        ParameterDef {
            key: "fast_period".into(),
            label: "Fast Period".into(),
            description: "Fast EMA period".into(),
            kind: ParameterKind::Integer { min: 2, max: 100 },
            default: ParameterValue::Integer(12),
            tab: ParameterTab::Parameters,
            section: None,
            order: 0,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "slow_period".into(),
            label: "Slow Period".into(),
            description: "Slow EMA period".into(),
            kind: ParameterKind::Integer { min: 2, max: 200 },
            default: ParameterValue::Integer(26),
            tab: ParameterTab::Parameters,
            section: None,
            order: 1,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "signal_period".into(),
            label: "Signal Period".into(),
            description: "Signal line EMA period".into(),
            kind: ParameterKind::Integer { min: 2, max: 100 },
            default: ParameterValue::Integer(9),
            tab: ParameterTab::Parameters,
            section: None,
            order: 2,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "macd_color".into(),
            label: "MACD Color".into(),
            description: "MACD line color".into(),
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
            key: "signal_color".into(),
            label: "Signal Color".into(),
            description: "Signal line color".into(),
            kind: ParameterKind::Color,
            default: ParameterValue::Color(SerializableColor {
                r: 1.0,
                g: 0.6,
                b: 0.2,
                a: 1.0,
            }),
            tab: ParameterTab::Style,
            section: None,
            order: 1,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "hist_positive_color".into(),
            label: "Histogram +".into(),
            description: "Histogram positive color".into(),
            kind: ParameterKind::Color,
            default: ParameterValue::Color(BULLISH_COLOR.with_alpha(0.7)),
            tab: ParameterTab::Style,
            section: None,
            order: 2,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
        ParameterDef {
            key: "hist_negative_color".into(),
            label: "Histogram -".into(),
            description: "Histogram negative color".into(),
            kind: ParameterKind::Color,
            default: ParameterValue::Color(BEARISH_COLOR.with_alpha(0.7)),
            tab: ParameterTab::Style,
            section: None,
            order: 3,
            format: DisplayFormat::Auto,
            visible_when: Visibility::Always,
        },
    ]
}

/// MACD oscillator with signal line and histogram.
///
/// The MACD line is the difference between a fast and slow EMA of
/// closing prices. The signal line is an EMA of the MACD line, and the
/// histogram visualises MACD minus signal. Renders in a separate panel
/// with two overlaid lines and a colored divergence histogram.
pub struct MacdStudy {
    config: StudyConfig,
    output: StudyOutput,
    params: Vec<ParameterDef>,
}

impl MacdStudy {
    /// Create a new MACD study with the classic default periods:
    /// fast EMA 12, slow EMA 26, signal EMA 9.
    pub fn new() -> Self {
        let params = make_params();
        let mut config = StudyConfig::new("macd");
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

impl Default for MacdStudy {
    fn default() -> Self {
        Self::new()
    }
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
        &self.params
    }

    fn config(&self) -> &StudyConfig {
        &self.config
    }

    fn config_mut(&mut self) -> &mut StudyConfig {
        &mut self.config
    }

    fn compute(&mut self, input: &StudyInput) -> Result<(), StudyError> {
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
        let hist_pos_color = self
            .config
            .get_color("hist_positive_color", BULLISH_COLOR.with_alpha(0.7));
        let hist_neg_color = self
            .config
            .get_color("hist_negative_color", BEARISH_COLOR.with_alpha(0.7));
        let candles = input.candles;
        let max_period = fast_period.max(slow_period);
        if candles.len() < max_period {
            log::debug!(
                "{}: insufficient data ({} candles, need {})",
                self.id(),
                candles.len(),
                max_period
            );
            self.output = StudyOutput::Empty;
            return Ok(());
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
            return Ok(());
        }

        let macd_values: Vec<f64> = (0..macd_len)
            .map(|i| fast_ema[i + fast_offset] - slow_ema[i])
            .collect();

        // Compute signal line (EMA of MACD values)
        let signal_ema = compute_ema(&macd_values, signal_period);
        if signal_ema.is_empty() {
            self.output = StudyOutput::Empty;
            return Ok(());
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
            let key = candle_key(
                &candles[candle_idx],
                candle_idx,
                candles.len(),
                &input.basis,
            );
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

        self.output = StudyOutput::Composite(vec![
            StudyOutput::Lines(vec![
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
            ]),
            StudyOutput::Histogram(histogram),
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
        Box::new(MacdStudy {
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
        let mut study = MacdStudy::new();
        let input = make_input(&[]);
        study.compute(&input).unwrap();
        assert!(matches!(study.output(), StudyOutput::Empty));
    }

    #[test]
    fn test_insufficient_candles() {
        let mut study = MacdStudy::new();
        let candles: Vec<Candle> = (0..20).map(|i| make_candle(i * 60000, 100.0)).collect();
        let input = make_input(&candles);
        study.compute(&input).unwrap();
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
        study.compute(&input).unwrap();

        let output = study.output();
        assert!(
            matches!(output, StudyOutput::Composite(_)),
            "expected Composite output"
        );
        let StudyOutput::Composite(outputs) = output else {
            unreachable!()
        };
        assert!(outputs.len() >= 2);
        let StudyOutput::Lines(lines) = &outputs[0] else {
            panic!("expected Lines first")
        };
        assert_eq!(lines.len(), 2);
        // MACD line should be near 0 for constant prices
        for point in &lines[0].points {
            assert!(point.1.abs() < 0.01, "MACD should be ~0, got {}", point.1);
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
        study.compute(&input).unwrap();

        let output = study.output();
        assert!(
            matches!(output, StudyOutput::Composite(_)),
            "expected Composite output"
        );
        let StudyOutput::Composite(outputs) = output else {
            unreachable!()
        };
        assert!(outputs.len() >= 2);
        let StudyOutput::Lines(lines) = &outputs[0] else {
            panic!("expected Lines first")
        };
        assert_eq!(lines.len(), 2);
        // In a strong uptrend, MACD should be positive
        for point in &lines[0].points {
            assert!(point.1 > 0.0, "MACD should be positive in uptrend");
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

    #[test]
    fn test_macd_alignment_all_three_series() {
        // H10: Verify MACD, Signal, and Histogram all align correctly
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

        let candles: Vec<Candle> = (0..20)
            .map(|i| make_candle(i * 60000, 100.0 + i as f32 * 5.0))
            .collect();
        let input = make_input(&candles);
        study.compute(&input).unwrap();

        let output = study.output();
        let StudyOutput::Composite(outputs) = output else {
            panic!("expected Composite output");
        };
        let StudyOutput::Lines(lines) = &outputs[0] else {
            panic!("expected Lines first");
        };
        let StudyOutput::Histogram(hist) = &outputs[1] else {
            panic!("expected Histogram second");
        };

        let macd_pts = &lines[0].points;
        let signal_pts = &lines[1].points;

        // All three series must have the same length
        assert_eq!(
            macd_pts.len(),
            signal_pts.len(),
            "MACD and Signal must have same length"
        );
        assert_eq!(
            macd_pts.len(),
            hist.len(),
            "MACD and Histogram must have same length"
        );

        // X-values must all match
        for i in 0..macd_pts.len() {
            assert_eq!(
                macd_pts[i].0, signal_pts[i].0,
                "MACD and Signal x-values must match at index {}",
                i
            );
            assert_eq!(
                macd_pts[i].0, hist[i].x,
                "MACD and Histogram x-values must match at index {}",
                i
            );
        }

        // Histogram = MACD - Signal at each point
        for i in 0..macd_pts.len() {
            let expected_hist = macd_pts[i].1 - signal_pts[i].1;
            assert!(
                (hist[i].value - expected_hist).abs() < 0.01,
                "Histogram[{}] = {} but MACD-Signal = {}",
                i,
                hist[i].value,
                expected_hist
            );
        }
    }
}
