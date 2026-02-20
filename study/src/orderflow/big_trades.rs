//! Big Trades Study
//!
//! Reconstructs institutional-scale executions by aggregating consecutive
//! same-side fills within a configurable time window, computing a
//! VWAP-weighted price, and outputting them as sized/colored markers.

use crate::config::{ParameterDef, ParameterKind, ParameterValue, StudyConfig};
use crate::error::StudyError;
use crate::output::{StudyOutput, TradeMarker};
use crate::traits::{Study, StudyCategory, StudyInput, StudyPlacement};
use data::SerializableColor;

const DEFAULT_MIN_CONTRACTS: i64 = 50;
const DEFAULT_AGGREGATION_WINDOW_MS: i64 = 150;
const DEFAULT_BUBBLE_SCALE: f64 = 1.0;

const DEFAULT_BUY_COLOR: SerializableColor = SerializableColor {
    r: 0.0,
    g: 0.8,
    b: 0.4,
    a: 0.7,
};

const DEFAULT_SELL_COLOR: SerializableColor = SerializableColor {
    r: 0.9,
    g: 0.2,
    b: 0.2,
    a: 0.7,
};

pub struct BigTradesStudy {
    config: StudyConfig,
    output: StudyOutput,
    params: Vec<ParameterDef>,
}

impl BigTradesStudy {
    pub fn new() -> Self {
        let params = vec![
            ParameterDef {
                key: "min_contracts",
                label: "Min Contracts",
                description: "Minimum contracts to display",
                kind: ParameterKind::Integer {
                    min: 1,
                    max: 10000,
                },
                default: ParameterValue::Integer(DEFAULT_MIN_CONTRACTS),
            },
            ParameterDef {
                key: "aggregation_window_ms",
                label: "Aggregation Window (ms)",
                description: "Max ms gap between fills to merge",
                kind: ParameterKind::Integer {
                    min: 10,
                    max: 5000,
                },
                default: ParameterValue::Integer(DEFAULT_AGGREGATION_WINDOW_MS),
            },
            ParameterDef {
                key: "buy_color",
                label: "Buy Color",
                description: "Buy bubble color",
                kind: ParameterKind::Color,
                default: ParameterValue::Color(DEFAULT_BUY_COLOR),
            },
            ParameterDef {
                key: "sell_color",
                label: "Sell Color",
                description: "Sell bubble color",
                kind: ParameterKind::Color,
                default: ParameterValue::Color(DEFAULT_SELL_COLOR),
            },
            ParameterDef {
                key: "bubble_scale",
                label: "Bubble Scale",
                description: "Bubble size multiplier",
                kind: ParameterKind::Float {
                    min: 0.5,
                    max: 3.0,
                    step: 0.1,
                },
                default: ParameterValue::Float(DEFAULT_BUBBLE_SCALE),
            },
            ParameterDef {
                key: "show_labels",
                label: "Show Labels",
                description: "Show contract count text",
                kind: ParameterKind::Boolean,
                default: ParameterValue::Boolean(true),
            },
        ];

        let mut config = StudyConfig::new("big_trades");
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

impl Default for BigTradesStudy {
    fn default() -> Self {
        Self::new()
    }
}

/// Format contract count for display.
fn format_contracts(contracts: f64) -> String {
    if contracts >= 1000.0 {
        format!("{:.1}K", contracts / 1000.0)
    } else {
        format!("{}", contracts as u64)
    }
}

/// Accumulator for aggregating consecutive same-side fills.
struct TradeBlock {
    is_buy: bool,
    vwap_numerator: f64,
    total_qty: f64,
    first_time: u64,
    last_time: u64,
}

impl TradeBlock {
    fn vwap(&self) -> f64 {
        if self.total_qty > 0.0 {
            self.vwap_numerator / self.total_qty
        } else {
            0.0
        }
    }

    fn mid_time(&self) -> u64 {
        (self.first_time + self.last_time) / 2
    }
}

impl Study for BigTradesStudy {
    fn id(&self) -> &str {
        "big_trades"
    }

    fn name(&self) -> &str {
        "Big Trades"
    }

    fn category(&self) -> StudyCategory {
        StudyCategory::OrderFlow
    }

    fn placement(&self) -> StudyPlacement {
        StudyPlacement::Overlay
    }

    fn parameters(&self) -> &[ParameterDef] {
        &self.params
    }

    fn config(&self) -> &StudyConfig {
        &self.config
    }

    fn set_parameter(
        &mut self,
        key: &str,
        value: ParameterValue,
    ) -> Result<(), StudyError> {
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
        let trades = match input.trades {
            Some(t) if !t.is_empty() => t,
            _ => {
                self.output = StudyOutput::Empty;
                return;
            }
        };

        let min_contracts =
            self.config.get_int("min_contracts", DEFAULT_MIN_CONTRACTS) as f64;
        let window_ms = self
            .config
            .get_int("aggregation_window_ms", DEFAULT_AGGREGATION_WINDOW_MS)
            as u64;
        let buy_color = self.config.get_color("buy_color", DEFAULT_BUY_COLOR);
        let sell_color =
            self.config.get_color("sell_color", DEFAULT_SELL_COLOR);
        let show_labels = self.config.get_bool("show_labels", true);

        let mut markers: Vec<TradeMarker> = Vec::new();
        let mut current_block: Option<TradeBlock> = None;

        let flush = |block: &TradeBlock,
                     markers: &mut Vec<TradeMarker>,
                     min_contracts: f64,
                     buy_color: SerializableColor,
                     sell_color: SerializableColor,
                     show_labels: bool,
                     candles: &[data::Candle],
                     basis: &data::ChartBasis| {
            if block.total_qty < min_contracts {
                return;
            }

            let color = if block.is_buy { buy_color } else { sell_color };
            let label = if show_labels {
                Some(format_contracts(block.total_qty))
            } else {
                None
            };

            // Map timestamp to appropriate X coordinate
            let time = match basis {
                data::ChartBasis::Time(_) => block.mid_time(),
                data::ChartBasis::Tick(_) => {
                    // Binary search candles to find reverse index
                    let mid = block.mid_time();
                    let idx = candles
                        .binary_search_by_key(&mid, |c| c.time.0)
                        .unwrap_or_else(|i| i.saturating_sub(1));
                    let idx = idx.min(candles.len().saturating_sub(1));
                    // Reverse index (newest = 0)
                    (candles.len().saturating_sub(1) - idx) as u64
                }
            };

            markers.push(TradeMarker {
                time,
                price: block.vwap(),
                contracts: block.total_qty,
                is_buy: block.is_buy,
                color,
                label,
            });
        };

        for trade in trades {
            let qty = trade.quantity.value();
            if qty <= 0.0 {
                continue;
            }

            let price = trade.price.to_f64();
            let time = trade.time.0;
            let is_buy = trade.side.is_buy();

            if let Some(ref mut block) = current_block {
                if block.is_buy == is_buy
                    && time.saturating_sub(block.last_time) <= window_ms
                {
                    // Merge into current block
                    block.vwap_numerator += price * qty;
                    block.total_qty += qty;
                    block.last_time = time;
                } else {
                    // Flush current block and start new one
                    flush(
                        block,
                        &mut markers,
                        min_contracts,
                        buy_color,
                        sell_color,
                        show_labels,
                        input.candles,
                        &input.basis,
                    );
                    current_block = Some(TradeBlock {
                        is_buy,
                        vwap_numerator: price * qty,
                        total_qty: qty,
                        first_time: time,
                        last_time: time,
                    });
                }
            } else {
                current_block = Some(TradeBlock {
                    is_buy,
                    vwap_numerator: price * qty,
                    total_qty: qty,
                    first_time: time,
                    last_time: time,
                });
            }
        }

        // Flush final block
        if let Some(ref block) = current_block {
            flush(
                block,
                &mut markers,
                min_contracts,
                buy_color,
                sell_color,
                show_labels,
                input.candles,
                &input.basis,
            );
        }

        if markers.is_empty() {
            self.output = StudyOutput::Empty;
        } else {
            self.output = StudyOutput::Markers(markers);
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
    use data::{
        Candle, ChartBasis, Price, Quantity, Side, Timeframe, Timestamp,
        Trade, Volume,
    };

    fn make_trade(time_ms: u64, price: f32, qty: f64, side: Side) -> Trade {
        Trade {
            time: Timestamp::from_millis(time_ms),
            price: Price::from_f32(price),
            quantity: Quantity(qty),
            side,
        }
    }

    fn make_candle(time_ms: u64, price: f32) -> Candle {
        Candle::new(
            Timestamp::from_millis(time_ms),
            Price::from_f32(price),
            Price::from_f32(price),
            Price::from_f32(price),
            Price::from_f32(price),
            Volume(0.0),
            Volume(0.0),
        )
    }

    fn study_input<'a>(
        candles: &'a [Candle],
        trades: &'a [Trade],
    ) -> StudyInput<'a> {
        StudyInput {
            candles,
            trades: Some(trades),
            basis: ChartBasis::Time(Timeframe::M1),
            tick_size: Price::from_f32(0.25),
            visible_range: None,
        }
    }

    #[test]
    fn test_empty_trades() {
        let mut study = BigTradesStudy::new();
        let candles = vec![];
        let trades: Vec<Trade> = vec![];
        study.compute(&study_input(&candles, &trades));
        assert!(matches!(study.output(), StudyOutput::Empty));
    }

    #[test]
    fn test_single_large_fill() {
        let mut study = BigTradesStudy::new();
        let candles = vec![make_candle(1000, 100.0)];
        let trades = vec![make_trade(1000, 100.0, 100.0, Side::Buy)];
        study.compute(&study_input(&candles, &trades));

        match study.output() {
            StudyOutput::Markers(m) => {
                assert_eq!(m.len(), 1);
                assert!(m[0].is_buy);
                assert!((m[0].contracts - 100.0).abs() < f64::EPSILON);
                assert!((m[0].price - 100.0).abs() < 0.01);
                assert_eq!(m[0].label.as_deref(), Some("100"));
            }
            other => panic!("Expected Markers, got {:?}", other),
        }
    }

    #[test]
    fn test_single_small_fill_below_threshold() {
        let mut study = BigTradesStudy::new();
        let candles = vec![make_candle(1000, 100.0)];
        let trades = vec![make_trade(1000, 100.0, 10.0, Side::Buy)];
        study.compute(&study_input(&candles, &trades));
        assert!(matches!(study.output(), StudyOutput::Empty));
    }

    #[test]
    fn test_three_same_side_fills_merge_with_correct_vwap() {
        let mut study = BigTradesStudy::new();
        let candles = vec![make_candle(1000, 100.0)];
        // 20 @ 100.0, 30 @ 101.0, 10 @ 102.0 => total 60
        // VWAP = (20*100 + 30*101 + 10*102) / 60 = (2000+3030+1020)/60
        //      = 6050 / 60 = 100.8333...
        let trades = vec![
            make_trade(1000, 100.0, 20.0, Side::Buy),
            make_trade(1050, 101.0, 30.0, Side::Buy),
            make_trade(1100, 102.0, 10.0, Side::Buy),
        ];

        study
            .set_parameter(
                "min_contracts",
                ParameterValue::Integer(50),
            )
            .unwrap();
        study.compute(&study_input(&candles, &trades));

        match study.output() {
            StudyOutput::Markers(m) => {
                assert_eq!(m.len(), 1);
                assert!(m[0].is_buy);
                assert!(
                    (m[0].contracts - 60.0).abs() < f64::EPSILON,
                    "contracts: {}",
                    m[0].contracts
                );
                let expected_vwap = 6050.0 / 60.0;
                assert!(
                    (m[0].price - expected_vwap).abs() < 0.01,
                    "vwap: {} expected: {}",
                    m[0].price,
                    expected_vwap
                );
            }
            other => panic!("Expected Markers, got {:?}", other),
        }
    }

    #[test]
    fn test_vwap_precision() {
        let mut study = BigTradesStudy::new();
        study
            .set_parameter("min_contracts", ParameterValue::Integer(1))
            .unwrap();
        let candles = vec![make_candle(1000, 100.0)];
        let trades = vec![
            make_trade(1000, 5432.75, 7.0, Side::Buy),
            make_trade(1010, 5433.25, 13.0, Side::Buy),
        ];
        study.compute(&study_input(&candles, &trades));

        match study.output() {
            StudyOutput::Markers(m) => {
                assert_eq!(m.len(), 1);
                // VWAP = (7*5432.75 + 13*5433.25) / 20
                //      = (38029.25 + 70632.25) / 20
                //      = 108661.5 / 20 = 5433.075
                let expected = (7.0 * 5432.75 + 13.0 * 5433.25) / 20.0;
                assert!(
                    (m[0].price - expected).abs() < 1e-8,
                    "vwap: {:.10} expected: {:.10}",
                    m[0].price,
                    expected
                );
            }
            other => panic!("Expected Markers, got {:?}", other),
        }
    }

    #[test]
    fn test_gap_exceeding_window_creates_two_markers() {
        let mut study = BigTradesStudy::new();
        study
            .set_parameter("min_contracts", ParameterValue::Integer(50))
            .unwrap();
        study
            .set_parameter(
                "aggregation_window_ms",
                ParameterValue::Integer(100),
            )
            .unwrap();

        let candles = vec![make_candle(1000, 100.0)];
        // Two groups separated by 200ms gap (> 100ms window)
        let trades = vec![
            make_trade(1000, 100.0, 60.0, Side::Buy),
            make_trade(1200, 101.0, 60.0, Side::Buy),
        ];
        study.compute(&study_input(&candles, &trades));

        match study.output() {
            StudyOutput::Markers(m) => {
                assert_eq!(m.len(), 2);
            }
            other => panic!("Expected Markers, got {:?}", other),
        }
    }

    #[test]
    fn test_side_change_creates_separate_markers() {
        let mut study = BigTradesStudy::new();
        study
            .set_parameter("min_contracts", ParameterValue::Integer(50))
            .unwrap();

        let candles = vec![make_candle(1000, 100.0)];
        let trades = vec![
            make_trade(1000, 100.0, 60.0, Side::Buy),
            make_trade(1050, 100.0, 60.0, Side::Sell),
        ];
        study.compute(&study_input(&candles, &trades));

        match study.output() {
            StudyOutput::Markers(m) => {
                assert_eq!(m.len(), 2);
                assert!(m[0].is_buy);
                assert!(!m[1].is_buy);
            }
            other => panic!("Expected Markers, got {:?}", other),
        }
    }

    #[test]
    fn test_continuous_burst_merges_with_previous_fill_window() {
        // 10 fills spaced 100ms apart with 150ms window
        // Each gap (100ms) <= window (150ms) measured from previous fill,
        // so all should merge into one marker
        let mut study = BigTradesStudy::new();
        study
            .set_parameter("min_contracts", ParameterValue::Integer(1))
            .unwrap();
        study
            .set_parameter(
                "aggregation_window_ms",
                ParameterValue::Integer(150),
            )
            .unwrap();

        let candles = vec![make_candle(1000, 100.0)];
        let trades: Vec<Trade> = (0..10)
            .map(|i| make_trade(1000 + i * 100, 100.0, 10.0, Side::Buy))
            .collect();
        study.compute(&study_input(&candles, &trades));

        match study.output() {
            StudyOutput::Markers(m) => {
                assert_eq!(
                    m.len(),
                    1,
                    "Expected 1 merged marker, got {}",
                    m.len()
                );
                assert!(
                    (m[0].contracts - 100.0).abs() < f64::EPSILON,
                    "contracts: {}",
                    m[0].contracts
                );
            }
            other => panic!("Expected Markers, got {:?}", other),
        }
    }

    #[test]
    fn test_zero_quantity_trades_skipped() {
        let mut study = BigTradesStudy::new();
        study
            .set_parameter("min_contracts", ParameterValue::Integer(50))
            .unwrap();

        let candles = vec![make_candle(1000, 100.0)];
        let trades = vec![
            make_trade(1000, 100.0, 60.0, Side::Buy),
            make_trade(1050, 100.0, 0.0, Side::Sell), // zero qty
            make_trade(1100, 100.0, 10.0, Side::Buy),
        ];
        study.compute(&study_input(&candles, &trades));

        match study.output() {
            StudyOutput::Markers(m) => {
                assert_eq!(m.len(), 1);
                // zero-qty sell didn't break the buy block
                assert!(
                    (m[0].contracts - 70.0).abs() < f64::EPSILON,
                    "contracts: {}",
                    m[0].contracts
                );
            }
            other => panic!("Expected Markers, got {:?}", other),
        }
    }

    #[test]
    fn test_label_formatting() {
        assert_eq!(format_contracts(50.0), "50");
        assert_eq!(format_contracts(999.0), "999");
        assert_eq!(format_contracts(1000.0), "1.0K");
        assert_eq!(format_contracts(1200.0), "1.2K");
        assert_eq!(format_contracts(15000.0), "15.0K");
    }

    #[test]
    fn test_parameter_update_affects_output() {
        let mut study = BigTradesStudy::new();
        let candles = vec![make_candle(1000, 100.0)];
        let trades = vec![make_trade(1000, 100.0, 30.0, Side::Buy)];

        // Default min_contracts=50, so 30 contracts won't show
        study.compute(&study_input(&candles, &trades));
        assert!(matches!(study.output(), StudyOutput::Empty));

        // Lower threshold to 20
        study
            .set_parameter("min_contracts", ParameterValue::Integer(20))
            .unwrap();
        study.compute(&study_input(&candles, &trades));
        assert!(matches!(study.output(), StudyOutput::Markers(_)));
    }

    #[test]
    fn test_clone_study_produces_independent_copy() {
        let mut study = BigTradesStudy::new();
        study
            .set_parameter("min_contracts", ParameterValue::Integer(10))
            .unwrap();

        let cloned = study.clone_study();
        assert_eq!(cloned.id(), "big_trades");
        assert_eq!(cloned.config().get_int("min_contracts", 50), 10);

        // Mutating original doesn't affect clone
        study
            .set_parameter("min_contracts", ParameterValue::Integer(99))
            .unwrap();
        assert_eq!(cloned.config().get_int("min_contracts", 50), 10);
    }
}
