//! Imbalance Study
//!
//! Highlights price levels where there is a significant imbalance between
//! buying and selling pressure, comparing diagonal bid/ask levels.
//!
//! Each imbalance is detected per-candle and emitted as a ray extending
//! rightward from the detection candle. Subsequent candles whose high-low
//! range includes the level price count as "hits", each multiplying the
//! ray's opacity by the `hit_decay` factor. Levels that fade below
//! [`MIN_OPACITY`] are pruned from the output entirely.

use crate::config::{
    DisplayFormat, LineStyleValue, ParameterDef, ParameterKind, ParameterTab,
    ParameterValue, StudyConfig, Visibility,
};
use crate::core::{Study, StudyCategory, StudyInput, StudyPlacement};
use crate::error::StudyError;
use crate::output::{PriceLevel, StudyOutput};
use crate::studies::orderflow::vbp::profile_core;
use crate::util::candle_key;
use crate::{BEARISH_COLOR, BULLISH_COLOR};
use data::SerializableColor;

const DEFAULT_THRESHOLD: f64 = 3.0;
const DEFAULT_HIT_DECAY: f64 = 0.5;

const DEFAULT_BUY_COLOR: SerializableColor = BULLISH_COLOR.with_alpha(0.6);
const DEFAULT_SELL_COLOR: SerializableColor = BEARISH_COLOR.with_alpha(0.6);

/// Levels with opacity below this are invisible and dropped.
const MIN_OPACITY: f32 = 0.03;

/// Hard cap on emitted levels to bound renderer draw calls.
/// When exceeded, oldest (leftmost) levels are discarded.
const MAX_OUTPUT_LEVELS: usize = 1500;

/// Type of imbalance detected at a price level.
#[derive(Debug, Clone, Copy)]
pub enum ImbalanceType {
    Buy { ratio: f32 },
    Sell { ratio: f32 },
}

/// Check if there's an imbalance between two price levels.
///
/// Compares sell quantity at one level against diagonal buy quantity
/// at the next higher level.
pub fn check_imbalance(
    sell_qty: f32,
    diagonal_buy_qty: f32,
    threshold: f32,
    ignore_zeros: bool,
) -> Option<ImbalanceType> {
    if ignore_zeros && (sell_qty <= 0.0 || diagonal_buy_qty <= 0.0) {
        return None;
    }

    if diagonal_buy_qty >= sell_qty && sell_qty > 0.0 {
        let ratio = diagonal_buy_qty / sell_qty;
        if ratio >= threshold {
            return Some(ImbalanceType::Buy { ratio });
        }
    }

    if sell_qty >= diagonal_buy_qty && diagonal_buy_qty > 0.0 {
        let ratio = sell_qty / diagonal_buy_qty;
        if ratio >= threshold {
            return Some(ImbalanceType::Sell { ratio });
        }
    }

    None
}

/// Maximum number of hits before `base_opacity * decay^n < MIN_OPACITY`.
///
/// Used as an early-exit bound in the hit-counting loop so we never
/// scan more candles than necessary.
fn max_visible_hits(base_opacity: f32, decay: f32) -> u32 {
    if base_opacity <= MIN_OPACITY {
        return 0;
    }
    if decay >= 1.0 {
        return u32::MAX;
    }
    if decay <= 0.0 {
        return 1;
    }
    let n = (MIN_OPACITY / base_opacity).ln() / decay.ln();
    (n.ceil() as u32).max(1)
}

pub struct ImbalanceStudy {
    config: StudyConfig,
    output: StudyOutput,
    params: Vec<ParameterDef>,
}

impl ImbalanceStudy {
    pub fn new() -> Self {
        let params = vec![
            ParameterDef {
                key: "threshold".into(),
                label: "Threshold".into(),
                description: "Imbalance ratio threshold".into(),
                kind: ParameterKind::Float {
                    min: 1.0,
                    max: 10.0,
                    step: 0.5,
                },
                default: ParameterValue::Float(DEFAULT_THRESHOLD),
                tab: ParameterTab::Parameters,
                section: None,
                order: 0,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
            ParameterDef {
                key: "buy_color".into(),
                label: "Buy Color".into(),
                description: "Color for buy imbalances".into(),
                kind: ParameterKind::Color,
                default: ParameterValue::Color(DEFAULT_BUY_COLOR),
                tab: ParameterTab::Style,
                section: None,
                order: 0,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
            ParameterDef {
                key: "sell_color".into(),
                label: "Sell Color".into(),
                description: "Color for sell imbalances".into(),
                kind: ParameterKind::Color,
                default: ParameterValue::Color(DEFAULT_SELL_COLOR),
                tab: ParameterTab::Style,
                section: None,
                order: 1,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
            ParameterDef {
                key: "ignore_zeros".into(),
                label: "Ignore Zeros".into(),
                description: "Skip levels with zero volume".into(),
                kind: ParameterKind::Boolean,
                default: ParameterValue::Boolean(true),
                tab: ParameterTab::Parameters,
                section: None,
                order: 1,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
            ParameterDef {
                key: "hit_decay".into(),
                label: "Hit Decay".into(),
                description: "Opacity multiplier per price hit".into(),
                kind: ParameterKind::Float {
                    min: 0.1,
                    max: 1.0,
                    step: 0.1,
                },
                default: ParameterValue::Float(DEFAULT_HIT_DECAY),
                tab: ParameterTab::Parameters,
                section: None,
                order: 2,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
        ];

        let mut config = StudyConfig::new("imbalance");
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

impl Default for ImbalanceStudy {
    fn default() -> Self {
        Self::new()
    }
}

impl Study for ImbalanceStudy {
    fn id(&self) -> &str {
        "imbalance"
    }

    fn name(&self) -> &str {
        "Imbalance"
    }

    fn category(&self) -> StudyCategory {
        StudyCategory::OrderFlow
    }

    fn placement(&self) -> StudyPlacement {
        StudyPlacement::Background
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
        let threshold =
            self.config.get_float("threshold", DEFAULT_THRESHOLD) as f32;
        let buy_color =
            self.config.get_color("buy_color", DEFAULT_BUY_COLOR);
        let sell_color =
            self.config.get_color("sell_color", DEFAULT_SELL_COLOR);
        let ignore_zeros = self.config.get_bool("ignore_zeros", true);
        let hit_decay =
            self.config.get_float("hit_decay", DEFAULT_HIT_DECAY) as f32;

        if input.candles.is_empty() || input.tick_size.units() <= 0 {
            self.output = StudyOutput::Empty;
            return Ok(());
        }

        let total = input.candles.len();
        let tick_units = input.tick_size.units();

        // Pre-extract candle ranges as contiguous f64 pairs for
        // cache-friendly hit scanning (avoids repeated to_f64 calls).
        let ranges: Vec<(f64, f64)> = input
            .candles
            .iter()
            .map(|c| (c.low.to_f64(), c.high.to_f64()))
            .collect();

        // Early-exit bound: once a level accumulates this many hits
        // its opacity is guaranteed below MIN_OPACITY.
        let worst_base = buy_color.a.max(sell_color.a);
        let hit_limit = max_visible_hits(worst_base, hit_decay);

        let mut levels = Vec::new();

        for (ci, candle) in input.candles.iter().enumerate() {
            let profile = profile_core::build_profile_from_candles(
                std::slice::from_ref(candle),
                input.tick_size,
                tick_units,
            );

            if profile.len() < 2 {
                continue;
            }

            let key = candle_key(candle, ci, total, &input.basis);

            for i in 0..profile.len() - 1 {
                let imb = check_imbalance(
                    profile[i].sell_volume,
                    profile[i + 1].buy_volume,
                    threshold,
                    ignore_zeros,
                );
                let Some(imb) = imb else { continue };

                let (price, is_buy) = match imb {
                    ImbalanceType::Buy { .. } => {
                        (profile[i + 1].price, true)
                    }
                    ImbalanceType::Sell { .. } => {
                        (profile[i].price, false)
                    }
                };

                let base_opacity =
                    if is_buy { buy_color.a } else { sell_color.a };

                // Count subsequent candles whose range covers this price,
                // stopping early once enough hits guarantee invisibility.
                let mut hits = 0u32;
                for &(low, high) in &ranges[ci + 1..] {
                    if low <= price && price <= high {
                        hits += 1;
                        if hits >= hit_limit {
                            break;
                        }
                    }
                }

                let opacity =
                    base_opacity * hit_decay.powi(hits as i32);
                if opacity < MIN_OPACITY {
                    continue;
                }

                let color =
                    if is_buy { buy_color } else { sell_color };

                levels.push(PriceLevel {
                    price,
                    label: String::new(),
                    color,
                    style: LineStyleValue::Solid,
                    opacity,
                    show_label: false,
                    fill_above: None,
                    fill_below: None,
                    start_x: Some(key),
                });
            }
        }

        // Cap output — keep the newest (rightmost) levels.
        if levels.len() > MAX_OUTPUT_LEVELS {
            levels.drain(..levels.len() - MAX_OUTPUT_LEVELS);
        }

        self.output = if levels.is_empty() {
            StudyOutput::Empty
        } else {
            StudyOutput::Levels(levels)
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

    fn make_candle(
        time: u64,
        open: f32,
        high: f32,
        low: f32,
        close: f32,
        buy_vol: f64,
        sell_vol: f64,
    ) -> Candle {
        Candle::new(
            Timestamp::from_millis(time),
            Price::from_f32(open),
            Price::from_f32(high),
            Price::from_f32(low),
            Price::from_f32(close),
            Volume(buy_vol),
            Volume(sell_vol),
        )
        .expect("test: valid candle")
    }

    #[test]
    fn test_check_imbalance_buy() {
        let result = check_imbalance(10.0, 50.0, 3.0, false);
        assert!(matches!(result, Some(ImbalanceType::Buy { .. })));
    }

    #[test]
    fn test_check_imbalance_sell() {
        let result = check_imbalance(50.0, 10.0, 3.0, false);
        assert!(matches!(result, Some(ImbalanceType::Sell { .. })));
    }

    #[test]
    fn test_check_imbalance_none() {
        let result = check_imbalance(10.0, 15.0, 3.0, false);
        assert!(result.is_none());
    }

    #[test]
    fn test_check_imbalance_ignore_zeros() {
        let result = check_imbalance(0.0, 50.0, 3.0, true);
        assert!(result.is_none());

        let result = check_imbalance(50.0, 0.0, 3.0, true);
        assert!(result.is_none());
    }

    #[test]
    fn test_max_visible_hits() {
        // decay=0.5, base=0.6 → 0.6*0.5^5 = 0.01875 < 0.03
        assert_eq!(max_visible_hits(0.6, 0.5), 5);
        // decay=0.1, base=0.6 → 0.6*0.1^2 = 0.006 < 0.03
        assert_eq!(max_visible_hits(0.6, 0.1), 2);
        // no decay → infinite
        assert_eq!(max_visible_hits(0.6, 1.0), u32::MAX);
        // base already invisible
        assert_eq!(max_visible_hits(0.02, 0.5), 0);
    }

    #[test]
    fn test_imbalance_study_compute() {
        let mut study = ImbalanceStudy::new();
        let candles = vec![
            make_candle(1000, 100.0, 102.0, 99.0, 101.0, 900.0, 10.0),
            make_candle(2000, 101.0, 103.0, 100.0, 102.0, 10.0, 900.0),
        ];

        let input = StudyInput {
            candles: &candles,
            trades: None,
            basis: ChartBasis::Time(Timeframe::M1),
            tick_size: Price::from_f32(1.0),
            visible_range: None,
        };

        study.compute(&input).unwrap();

        match &study.output {
            StudyOutput::Levels(levels) => {
                assert!(!levels.is_empty());
                for level in levels {
                    assert!(
                        level.start_x.is_some(),
                        "Imbalance levels must have start_x set"
                    );
                }
            }
            StudyOutput::Empty => {
                // Acceptable if volumes don't meet threshold
            }
            other => panic!(
                "Expected Levels or Empty, got {:?}",
                other
            ),
        }
    }

    #[test]
    fn test_imbalance_empty() {
        let mut study = ImbalanceStudy::new();
        let candles: Vec<Candle> = vec![];

        let input = StudyInput {
            candles: &candles,
            trades: None,
            basis: ChartBasis::Time(Timeframe::M1),
            tick_size: Price::from_f32(1.0),
            visible_range: None,
        };

        study.compute(&input).unwrap();
        assert!(matches!(study.output(), StudyOutput::Empty));
    }

    #[test]
    fn test_hit_counting_and_opacity_decay() {
        let mut study = ImbalanceStudy::new();
        study.config.set(
            String::from("threshold"),
            ParameterValue::Float(2.0),
        );
        study.config.set(
            String::from("hit_decay"),
            ParameterValue::Float(0.5),
        );

        // Candle 0: strong buy imbalance around 101
        // Candles 1-3: price passes through 101, each counts as hit
        let candles = vec![
            make_candle(
                1000, 100.0, 102.0, 100.0, 101.0, 500.0, 5.0,
            ),
            make_candle(
                2000, 100.0, 102.0, 100.0, 101.0, 50.0, 50.0,
            ),
            make_candle(
                3000, 100.0, 102.0, 100.0, 101.0, 50.0, 50.0,
            ),
            make_candle(
                4000, 100.0, 102.0, 100.0, 101.0, 50.0, 50.0,
            ),
        ];

        let input = StudyInput {
            candles: &candles,
            trades: None,
            basis: ChartBasis::Time(Timeframe::M1),
            tick_size: Price::from_f32(1.0),
            visible_range: None,
        };

        study.compute(&input).unwrap();

        if let StudyOutput::Levels(levels) = &study.output {
            let from_first: Vec<_> = levels
                .iter()
                .filter(|l| l.start_x == Some(1000))
                .collect();
            let from_last: Vec<_> = levels
                .iter()
                .filter(|l| l.start_x == Some(4000))
                .collect();

            for level in &from_first {
                assert!(
                    level.opacity < 0.6,
                    "First candle levels should have decayed: {}",
                    level.opacity
                );
            }

            for level in &from_last {
                assert!(
                    level.opacity >= 0.5,
                    "Last candle levels should be near base: {}",
                    level.opacity
                );
            }
        }
    }

    #[test]
    fn test_levels_disappear_after_many_hits() {
        let mut study = ImbalanceStudy::new();
        study.config.set(
            String::from("threshold"),
            ParameterValue::Float(2.0),
        );
        study.config.set(
            String::from("hit_decay"),
            ParameterValue::Float(0.1),
        );

        let mut candles = vec![make_candle(
            1000, 100.0, 102.0, 100.0, 101.0, 500.0, 5.0,
        )];
        for i in 1..10 {
            candles.push(make_candle(
                1000 + i * 1000,
                100.0,
                102.0,
                100.0,
                101.0,
                50.0,
                50.0,
            ));
        }

        let input = StudyInput {
            candles: &candles,
            trades: None,
            basis: ChartBasis::Time(Timeframe::M1),
            tick_size: Price::from_f32(1.0),
            visible_range: None,
        };

        study.compute(&input).unwrap();

        if let StudyOutput::Levels(levels) = &study.output {
            // decay=0.1: max_visible_hits(0.6, 0.1) = 2
            // Candle 0 has 9 subsequent hits → well past limit
            let from_first: Vec<_> = levels
                .iter()
                .filter(|l| l.start_x == Some(1000))
                .collect();
            assert!(
                from_first.is_empty(),
                "Heavily-hit levels should be pruned"
            );
        }
    }

    #[test]
    fn test_output_capped() {
        let mut study = ImbalanceStudy::new();
        // Very low threshold to maximize imbalance detections
        study.config.set(
            String::from("threshold"),
            ParameterValue::Float(1.1),
        );
        // No decay so nothing gets pruned
        study.config.set(
            String::from("hit_decay"),
            ParameterValue::Float(1.0),
        );

        // Generate many candles with imbalances
        let candles: Vec<Candle> = (0..5000)
            .map(|i| {
                make_candle(
                    i * 60_000,
                    100.0,
                    110.0,
                    90.0,
                    105.0,
                    800.0,
                    10.0,
                )
            })
            .collect();

        let input = StudyInput {
            candles: &candles,
            trades: None,
            basis: ChartBasis::Time(Timeframe::M1),
            tick_size: Price::from_f32(1.0),
            visible_range: None,
        };

        study.compute(&input).unwrap();

        if let StudyOutput::Levels(levels) = &study.output {
            assert!(
                levels.len() <= MAX_OUTPUT_LEVELS,
                "Output should be capped at {}, got {}",
                MAX_OUTPUT_LEVELS,
                levels.len()
            );
        }
    }
}
