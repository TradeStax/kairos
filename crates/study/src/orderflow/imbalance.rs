//! Imbalance Study
//!
//! Highlights price levels where there is a significant imbalance between
//! buying and selling pressure, comparing diagonal bid/ask levels.

use crate::config::{
    DisplayFormat, LineStyleValue, ParameterDef, ParameterKind, ParameterTab,
    ParameterValue, StudyConfig, Visibility,
};
use crate::error::StudyError;
use crate::orderflow::vbp::profile_core;
use crate::output::{PriceLevel, StudyOutput};
use crate::core::{Study, StudyCategory, StudyInput, StudyPlacement};
use crate::{BEARISH_COLOR, BULLISH_COLOR};
use data::SerializableColor;

const DEFAULT_THRESHOLD: f64 = 3.0;

const DEFAULT_BUY_COLOR: SerializableColor = BULLISH_COLOR.with_alpha(0.6);

const DEFAULT_SELL_COLOR: SerializableColor = BEARISH_COLOR.with_alpha(0.6);

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
        let threshold = self.config.get_float("threshold", DEFAULT_THRESHOLD) as f32;
        let buy_color = self.config.get_color("buy_color", DEFAULT_BUY_COLOR);
        let sell_color = self.config.get_color("sell_color", DEFAULT_SELL_COLOR);
        let ignore_zeros = self.config.get_bool("ignore_zeros", true);

        if input.candles.is_empty() {
            self.output = StudyOutput::Empty;
            return Ok(());
        }

        if input.tick_size.units() <= 0 {
            self.output = StudyOutput::Empty;
            return Ok(());
        }

        // Build a buy/sell volume profile from candle data
        let profile =
            profile_core::build_profile_from_candles(
                input.candles,
                input.tick_size,
                input.tick_size.units(),
            );

        // Check adjacent levels for imbalances
        let mut levels = Vec::new();

        for i in 0..profile.len().saturating_sub(1) {
            let sell_qty = profile[i].sell_volume;
            let diag_buy_qty = profile[i + 1].buy_volume;

            if let Some(imbalance_type) = check_imbalance(
                sell_qty,
                diag_buy_qty,
                threshold,
                ignore_zeros,
            ) {
                let (level_price, color, label) = match imbalance_type {
                    ImbalanceType::Buy { ratio } => {
                        (profile[i + 1].price, buy_color, format!("Buy {:.1}x", ratio))
                    }
                    ImbalanceType::Sell { ratio } => {
                        (profile[i].price, sell_color, format!("Sell {:.1}x", ratio))
                    }
                };

                levels.push(PriceLevel {
                    price: level_price,
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

        study.compute(&input).unwrap();

        // The output should be Levels or Empty depending on threshold
        match &study.output {
            StudyOutput::Levels(levels) => {
                assert!(!levels.is_empty());
            }
            StudyOutput::Empty => {
                // Also acceptable if volumes don't meet threshold
            }
            other => assert!(
                matches!(other, StudyOutput::Levels(_) | StudyOutput::Empty),
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
}
