//! Stochastic Oscillator.
//!
//! Measures where the current close sits within the recent high-low range.
//! The raw %K value is `100 * (Close - Lowest Low) / (Highest High - Lowest Low)`
//! over the lookback period.
//!
//! The "slow" variant (default) applies an SMA smoothing to %K before
//! computing %D (an SMA of the smoothed %K). Traders watch for %K/%D
//! crossovers and readings above 80 (overbought) or below 20 (oversold).
//!
//! Output: `StudyOutput::Lines` with two series (%K solid, %D dashed).

use std::collections::VecDeque;

use crate::config::{
    DisplayFormat, LineStyleValue, ParameterDef, ParameterKind, ParameterTab,
    ParameterValue, StudyConfig, Visibility,
};
use crate::core::{Study, StudyCategory, StudyInput, StudyPlacement};
use crate::error::StudyError;
use crate::output::{LineSeries, StudyOutput};
use crate::studies::trend::sma::compute_sma;
use crate::util::candle_key;
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

/// Stochastic oscillator with %K and %D lines.
///
/// Raw %K locates the current close within the recent high-low range
/// (0 = at the lowest low, 100 = at the highest high). The slow
/// variant applies an SMA smoothing to %K, then derives %D as an SMA
/// of the smoothed %K. Renders in a separate panel with a solid %K
/// line and a dashed %D signal line.
pub struct StochasticStudy {
    config: StudyConfig,
    output: StudyOutput,
    params: Vec<ParameterDef>,
}

impl StochasticStudy {
    /// Create a new Stochastic study with standard slow-stochastic
    /// defaults: %K lookback 14, %K smoothing 3, %D period 3,
    /// overbought 80, oversold 20.
    pub fn new() -> Self {
        let params = vec![
            ParameterDef {
                key: "k_period".into(),
                label: "%K Period".into(),
                description: "Lookback period for %K calculation".into(),
                kind: ParameterKind::Integer { min: 5, max: 50 },
                default: ParameterValue::Integer(14),
                tab: ParameterTab::Parameters,
                section: None,
                order: 0,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
            ParameterDef {
                key: "d_period".into(),
                label: "%D Period".into(),
                description: "Smoothing period for %D (signal line)".into(),
                kind: ParameterKind::Integer { min: 1, max: 20 },
                default: ParameterValue::Integer(3),
                tab: ParameterTab::Parameters,
                section: None,
                order: 1,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
            ParameterDef {
                key: "smooth".into(),
                label: "Smooth".into(),
                description: "Smoothing period for %K".into(),
                kind: ParameterKind::Integer { min: 1, max: 10 },
                default: ParameterValue::Integer(3),
                tab: ParameterTab::Parameters,
                section: None,
                order: 2,
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
                default: ParameterValue::Float(80.0),
                tab: ParameterTab::Parameters,
                section: None,
                order: 3,
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
                default: ParameterValue::Float(20.0),
                tab: ParameterTab::Parameters,
                section: None,
                order: 4,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
            ParameterDef {
                key: "k_color".into(),
                label: "%K Color".into(),
                description: "%K line color".into(),
                kind: ParameterKind::Color,
                default: ParameterValue::Color(DEFAULT_K_COLOR),
                tab: ParameterTab::Style,
                section: None,
                order: 0,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
            ParameterDef {
                key: "d_color".into(),
                label: "%D Color".into(),
                description: "%D line color".into(),
                kind: ParameterKind::Color,
                default: ParameterValue::Color(DEFAULT_D_COLOR),
                tab: ParameterTab::Style,
                section: None,
                order: 1,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
        ];

        let mut config = StudyConfig::new("stochastic");
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

impl Default for StochasticStudy {
    fn default() -> Self {
        Self::new()
    }
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

    fn config_mut(&mut self) -> &mut StudyConfig {
        &mut self.config
    }

    fn compute(&mut self, input: &StudyInput) -> Result<(), StudyError> {
        let k_period = self.config.get_int("k_period", 14) as usize;
        let d_period = self.config.get_int("d_period", 3) as usize;
        let smooth = self.config.get_int("smooth", 3) as usize;
        let k_color = self.config.get_color("k_color", DEFAULT_K_COLOR);
        let d_color = self.config.get_color("d_color", DEFAULT_D_COLOR);

        let candles = input.candles;
        if candles.len() < k_period {
            log::debug!(
                "{}: insufficient data ({} candles, need {})",
                self.id(),
                candles.len(),
                k_period
            );
            self.output = StudyOutput::Empty;
            return Ok(());
        }

        // Step 1: Compute raw %K (fast stochastic) using O(N)
        // monotonic deques for sliding-window min/max.
        let highs: Vec<f32> =
            candles.iter().map(|c| c.high.to_f32()).collect();
        let lows: Vec<f32> =
            candles.iter().map(|c| c.low.to_f32()).collect();

        let mut raw_k =
            Vec::with_capacity(candles.len() - k_period + 1);

        // max_deque: indices of decreasing highs (front = max)
        let mut max_deque: VecDeque<usize> = VecDeque::new();
        // min_deque: indices of increasing lows (front = min)
        let mut min_deque: VecDeque<usize> = VecDeque::new();

        for i in 0..candles.len() {
            // Maintain max deque (decreasing highs)
            while max_deque
                .back()
                .is_some_and(|&j| highs[j] <= highs[i])
            {
                max_deque.pop_back();
            }
            max_deque.push_back(i);

            // Maintain min deque (increasing lows)
            while min_deque
                .back()
                .is_some_and(|&j| lows[j] >= lows[i])
            {
                min_deque.pop_back();
            }
            min_deque.push_back(i);

            // Remove elements outside the window
            let window_start = i + 1 - k_period.min(i + 1);
            while max_deque
                .front()
                .is_some_and(|&j| j < window_start)
            {
                max_deque.pop_front();
            }
            while min_deque
                .front()
                .is_some_and(|&j| j < window_start)
            {
                min_deque.pop_front();
            }

            if i >= k_period - 1 {
                let highest = highs[max_deque[0]];
                let lowest = lows[min_deque[0]];
                let close = candles[i].close.to_f32();
                let range = highest - lowest;
                let k_val = if range > 0.0 {
                    100.0 * (close - lowest) as f64 / range as f64
                } else {
                    50.0 // Flat market, default to midpoint
                };
                raw_k.push(k_val);
            }
        }

        // Step 2: Smooth %K with SMA
        let smoothed_k = if smooth > 1 {
            compute_sma(&raw_k, smooth)
        } else {
            raw_k.clone()
        };

        if smoothed_k.is_empty() {
            self.output = StudyOutput::Empty;
            return Ok(());
        }

        // Step 3: %D = SMA of smoothed %K
        let d_values = compute_sma(&smoothed_k, d_period);

        if d_values.is_empty() {
            self.output = StudyOutput::Empty;
            return Ok(());
        }

        // Build points for %K (aligned with %D).
        //
        // Offset derivation (candle indices, 0-based):
        //   raw_k[0]     → candle index (k_period - 1)
        //   smoothed_k   = SMA(raw_k, smooth) → first valid at
        //                   raw_k index (smooth - 1), which is
        //                   candle index (k_period - 1) + (smooth - 1)
        //   k_offset     = k_period - 1 + (smooth - 1)
        //
        //   d_values     = SMA(smoothed_k, d_period) → first valid at
        //                   smoothed_k index (d_period - 1), which is
        //                   candle index k_offset + (d_period - 1)
        //   d_offset     = k_offset + (d_period - 1)
        //
        // Example: k_period=14, smooth=3, d_period=3
        //   k_offset = 13 + 2 = 15
        //   d_offset = 15 + 2 = 17
        //   → first output at candle index 17
        let k_offset = k_period - 1 + (smooth.max(1) - 1);
        let d_offset = k_offset + (d_period - 1);

        // We output %K from d_offset onward so both lines are aligned
        let k_start_in_smoothed = d_period - 1; // index into smoothed_k

        let mut k_points = Vec::with_capacity(d_values.len());
        let mut d_points = Vec::with_capacity(d_values.len());

        for (j, d_val) in d_values.iter().enumerate() {
            let candle_idx = d_offset + j;
            if candle_idx >= candles.len() {
                break;
            }
            let key = candle_key(
                &candles[candle_idx],
                candle_idx,
                candles.len(),
                &input.basis,
            );
            let k_val = smoothed_k[k_start_in_smoothed + j];
            k_points.push((key, k_val as f32));
            d_points.push((key, *d_val as f32));
        }

        self.output = StudyOutput::Lines(vec![
            LineSeries {
                label: "%K".to_string(),
                color: k_color,
                width: 1.5,
                style: LineStyleValue::Solid,
                points: k_points,
            },
            LineSeries {
                label: "%D".to_string(),
                color: d_color,
                width: 1.5,
                style: LineStyleValue::Dashed,
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
        assert!(
            matches!(output, StudyOutput::Lines(_)),
            "expected Lines output"
        );
        let StudyOutput::Lines(lines) = output else {
            unreachable!()
        };
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
        assert!(
            matches!(output, StudyOutput::Lines(_)),
            "expected Lines output"
        );
        let StudyOutput::Lines(lines) = output else {
            unreachable!()
        };
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
        assert!(
            matches!(output, StudyOutput::Lines(_)),
            "expected Lines output"
        );
        let StudyOutput::Lines(lines) = output else {
            unreachable!()
        };
        assert_eq!(lines.len(), 2);
        // %K and %D should have the same number of points
        assert_eq!(lines[0].points.len(), lines[1].points.len());
        // And the x-values should match
        for (k, d) in lines[0].points.iter().zip(lines[1].points.iter()) {
            assert_eq!(k.0, d.0);
        }
    }

    /// H9: Edge case — smooth=1 means no smoothing, %K should equal raw %K
    #[test]
    fn test_stochastic_smooth_1() {
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

        let candles: Vec<Candle> = (0..15)
            .map(|i| {
                let base = 100.0 + i as f32 * 3.0;
                make_candle(i as u64 * 60000, base + 5.0, base - 5.0, base)
            })
            .collect();
        let input = make_input(&candles);
        study.compute(&input).unwrap();

        let output = study.output();
        let StudyOutput::Lines(lines) = output else {
            panic!("expected Lines output");
        };
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].points.len(), lines[1].points.len());
        // All values should be in [0, 100]
        for pt in &lines[0].points {
            assert!(pt.1 >= -0.1 && pt.1 <= 100.1, "%K out of range: {}", pt.1);
        }
        for pt in &lines[1].points {
            assert!(pt.1 >= -0.1 && pt.1 <= 100.1, "%D out of range: {}", pt.1);
        }
    }

    /// H9: Edge case — d_period=1 means %D = %K (no smoothing of %K)
    #[test]
    fn test_stochastic_d_period_1() {
        let mut study = StochasticStudy::new();
        study
            .set_parameter("k_period", ParameterValue::Integer(5))
            .unwrap();
        study
            .set_parameter("d_period", ParameterValue::Integer(1))
            .unwrap();
        study
            .set_parameter("smooth", ParameterValue::Integer(3))
            .unwrap();

        let candles: Vec<Candle> = (0..15)
            .map(|i| {
                let base = 100.0 + i as f32 * 3.0;
                make_candle(i as u64 * 60000, base + 5.0, base - 5.0, base)
            })
            .collect();
        let input = make_input(&candles);
        study.compute(&input).unwrap();

        let output = study.output();
        let StudyOutput::Lines(lines) = output else {
            panic!("expected Lines output");
        };
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].points.len(), lines[1].points.len());
        // With d_period=1, %D = SMA(1) of %K = %K itself
        for (k, d) in lines[0].points.iter().zip(lines[1].points.iter()) {
            assert!(
                (k.1 - d.1).abs() < 0.01,
                "%K ({}) != %D ({}) when d_period=1",
                k.1,
                d.1
            );
        }
    }
}
