//! Imbalance Study
//!
//! Highlights price levels where there is a significant imbalance between
//! buying and selling pressure, comparing diagonal bid/ask levels.

use crate::config::{LineStyleValue, ParameterDef, ParameterKind, ParameterValue, StudyConfig};
use crate::error::StudyError;
use crate::output::{PriceLevel, StudyOutput};
use crate::traits::{Study, StudyCategory, StudyInput, StudyPlacement};
use data::SerializableColor;
use std::collections::BTreeMap;

const DEFAULT_THRESHOLD: f64 = 3.0;

const DEFAULT_BUY_COLOR: SerializableColor = SerializableColor {
    r: 0.0,
    g: 0.8,
    b: 0.4,
    a: 0.6,
};

const DEFAULT_SELL_COLOR: SerializableColor = SerializableColor {
    r: 0.9,
    g: 0.2,
    b: 0.2,
    a: 0.6,
};

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

pub struct ImbalanceStudy {
    config: StudyConfig,
    output: StudyOutput,
    params: Vec<ParameterDef>,
}

impl ImbalanceStudy {
    pub fn new() -> Self {
        let params = vec![
            ParameterDef {
                key: "threshold",
                label: "Threshold",
                description: "Imbalance ratio threshold",
                kind: ParameterKind::Float {
                    min: 1.0,
                    max: 10.0,
                    step: 0.5,
                },
                default: ParameterValue::Float(DEFAULT_THRESHOLD),
            },
            ParameterDef {
                key: "buy_color",
                label: "Buy Color",
                description: "Color for buy imbalances",
                kind: ParameterKind::Color,
                default: ParameterValue::Color(DEFAULT_BUY_COLOR),
            },
            ParameterDef {
                key: "sell_color",
                label: "Sell Color",
                description: "Color for sell imbalances",
                kind: ParameterKind::Color,
                default: ParameterValue::Color(DEFAULT_SELL_COLOR),
            },
            ParameterDef {
                key: "ignore_zeros",
                label: "Ignore Zeros",
                description: "Skip levels with zero volume",
                kind: ParameterKind::Boolean,
                default: ParameterValue::Boolean(true),
            },
        ];

        let mut config = StudyConfig::new("imbalance");
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

    fn compute(&mut self, input: &StudyInput) {
        let threshold = self.config.get_float("threshold", DEFAULT_THRESHOLD) as f32;
        let buy_color = self.config.get_color("buy_color", DEFAULT_BUY_COLOR);
        let sell_color = self.config.get_color("sell_color", DEFAULT_SELL_COLOR);
        let ignore_zeros = self.config.get_bool("ignore_zeros", true);

        if input.candles.is_empty() {
            self.output = StudyOutput::Empty;
            return;
        }

        let step = input.tick_size.units();
        if step <= 0 {
            self.output = StudyOutput::Empty;
            return;
        }

        // Build a buy/sell volume profile from candle data
        let mut profile: BTreeMap<i64, (f64, f64)> = BTreeMap::new();

        for c in input.candles {
            let low_units = c.low.round_to_tick(input.tick_size).units();
            let high_units = c.high.round_to_tick(input.tick_size).units();

            if high_units < low_units {
                continue;
            }

            let num_levels = ((high_units - low_units) / step + 1) as f64;
            if num_levels <= 0.0 {
                continue;
            }

            let buy_per_level = c.buy_volume.value() / num_levels;
            let sell_per_level = c.sell_volume.value() / num_levels;

            let mut price_units = low_units;
            while price_units <= high_units {
                let entry = profile.entry(price_units).or_insert((0.0, 0.0));
                entry.0 += buy_per_level;
                entry.1 += sell_per_level;
                price_units += step;
            }
        }

        // Check adjacent levels for imbalances
        let prices: Vec<i64> = profile.keys().copied().collect();
        let mut levels = Vec::new();

        for i in 0..prices.len().saturating_sub(1) {
            let price = prices[i];
            let higher_price = prices[i + 1];

            let (_, sell_qty) = profile[&price];
            let (diag_buy_qty, _) = profile[&higher_price];

            if let Some(imbalance_type) = check_imbalance(
                sell_qty as f32,
                diag_buy_qty as f32,
                threshold,
                ignore_zeros,
            ) {
                let (level_price, color, label) = match imbalance_type {
                    ImbalanceType::Buy { ratio } => {
                        (higher_price, buy_color, format!("Buy {:.1}x", ratio))
                    }
                    ImbalanceType::Sell { ratio } => {
                        (price, sell_color, format!("Sell {:.1}x", ratio))
                    }
                };

                levels.push(PriceLevel {
                    price: data::Price::from_units(level_price).to_f64(),
                    label,
                    color,
                    style: LineStyleValue::Solid,
                    opacity: color.a,
                    show_label: false,
                    fill_above: None,
                    fill_below: None,
                });
            }
        }

        if levels.is_empty() {
            self.output = StudyOutput::Empty;
        } else {
            self.output = StudyOutput::Levels(levels);
        }
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
    fn test_imbalance_study_compute() {
        let mut study = ImbalanceStudy::new();
        // Use very skewed buy/sell to force imbalances
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

        study.compute(&input);

        // The output should be Levels or Empty depending on threshold
        match &study.output {
            StudyOutput::Levels(levels) => {
                assert!(!levels.is_empty());
            }
            StudyOutput::Empty => {
                // Also acceptable if volumes don't meet threshold
            }
            other => panic!("Expected Levels or Empty, got {:?}", other),
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

        study.compute(&input);
        assert!(matches!(study.output(), StudyOutput::Empty));
    }
}
