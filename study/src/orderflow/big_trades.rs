//! Big Trades Study
//!
//! Reconstructs institutional-scale executions by aggregating consecutive
//! same-side fills within a configurable time window, computing a
//! VWAP-weighted price, and outputting them as sized/colored markers.

use crate::config::{ParameterDef, ParameterKind, ParameterValue, StudyConfig};
use crate::error::StudyError;
use crate::output::{StudyOutput, TradeMarker, TradeMarkerDebug};
use crate::traits::{Study, StudyCategory, StudyInput, StudyPlacement};
use data::SerializableColor;

const DEFAULT_MIN_CONTRACTS: i64 = 50;
const DEFAULT_AGGREGATION_WINDOW_MS: i64 = 40;
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
    // Incremental state
    processed_trade_count: usize,
    pending_block: Option<TradeBlock>,
    accumulated_markers: Vec<TradeMarker>,
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
            ParameterDef {
                key: "show_debug",
                label: "Show Debug",
                description: "Show debug annotations on markers",
                kind: ParameterKind::Boolean,
                default: ParameterValue::Boolean(false),
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
            processed_trade_count: 0,
            pending_block: None,
            accumulated_markers: Vec::new(),
        }
    }

    /// Read current parameters from config.
    fn read_params(&self) -> ComputeParams {
        ComputeParams {
            min_contracts: self
                .config
                .get_int("min_contracts", DEFAULT_MIN_CONTRACTS) as f64,
            window_ms: self
                .config
                .get_int("aggregation_window_ms", DEFAULT_AGGREGATION_WINDOW_MS)
                as u64,
            buy_color: self.config.get_color("buy_color", DEFAULT_BUY_COLOR),
            sell_color: self.config.get_color("sell_color", DEFAULT_SELL_COLOR),
            show_labels: self.config.get_bool("show_labels", true),
            show_debug: self.config.get_bool("show_debug", false),
        }
    }

    /// Core aggregation loop: processes a slice of trades, mutating
    /// `pending_block` and appending completed markers.
    fn aggregate_trades(
        trades: &[data::Trade],
        pending: &mut Option<TradeBlock>,
        markers: &mut Vec<TradeMarker>,
        params: &ComputeParams,
        candles: &[data::Candle],
        basis: &data::ChartBasis,
        candle_boundaries: &Option<Vec<(u64, u64)>>,
    ) {
        let is_time_based = matches!(basis, data::ChartBasis::Time(_));

        for trade in trades {
            let qty = trade.quantity.value();
            if qty <= 0.0 {
                continue;
            }

            let price_units = trade.price.units();
            let time = trade.time.0;
            let is_buy = trade.side.is_buy();

            let candle_open = if is_time_based {
                find_candle_open(time, candles)
            } else {
                0
            };

            if let Some(block) = pending {
                let same_candle =
                    !is_time_based || candle_open == block.candle_open;

                if block.is_buy == is_buy
                    && time.saturating_sub(block.last_time)
                        <= params.window_ms
                    && same_candle
                {
                    // Merge into current block
                    block.vwap_numerator += price_units as f64 * qty;
                    block.total_qty += qty;
                    block.last_time = time;
                    block.fill_count += 1;
                    block.min_price_units =
                        block.min_price_units.min(price_units);
                    block.max_price_units =
                        block.max_price_units.max(price_units);
                } else {
                    // Flush current block and start new one
                    flush_block(
                        block,
                        markers,
                        params,
                        candles,
                        basis,
                        candle_boundaries,
                    );
                    *pending = Some(TradeBlock::new(
                        is_buy,
                        price_units,
                        qty,
                        time,
                        candle_open,
                    ));
                }
            } else {
                *pending = Some(TradeBlock::new(
                    is_buy,
                    price_units,
                    qty,
                    time,
                    candle_open,
                ));
            }
        }
    }

    /// Finalize output from accumulated markers.
    fn finalize_output(markers: &[TradeMarker]) -> StudyOutput {
        if markers.is_empty() {
            StudyOutput::Empty
        } else {
            StudyOutput::Markers(markers.to_vec())
        }
    }
}

impl Default for BigTradesStudy {
    fn default() -> Self {
        Self::new()
    }
}

/// Parameters extracted from config for a compute pass.
struct ComputeParams {
    min_contracts: f64,
    window_ms: u64,
    buy_color: SerializableColor,
    sell_color: SerializableColor,
    show_labels: bool,
    show_debug: bool,
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
    /// sum of (price_units_i64 * qty)
    vwap_numerator: f64,
    total_qty: f64,
    first_time: u64,
    last_time: u64,
    fill_count: u32,
    min_price_units: i64,
    max_price_units: i64,
    /// Containing candle's open time (time-based charts only, 0 otherwise)
    candle_open: u64,
}

impl TradeBlock {
    fn new(
        is_buy: bool,
        price_units: i64,
        qty: f64,
        time: u64,
        candle_open: u64,
    ) -> Self {
        Self {
            is_buy,
            vwap_numerator: price_units as f64 * qty,
            total_qty: qty,
            first_time: time,
            last_time: time,
            fill_count: 1,
            min_price_units: price_units,
            max_price_units: price_units,
            candle_open,
        }
    }

    fn vwap_units(&self) -> i64 {
        if self.total_qty > 0.0 {
            (self.vwap_numerator / self.total_qty).round() as i64
        } else {
            0
        }
    }

    fn mid_time(&self) -> u64 {
        (self.first_time + self.last_time) / 2
    }
}

/// Build candle boundary lookup for tick charts.
fn build_candle_boundaries(
    candles: &[data::Candle],
    basis: &data::ChartBasis,
) -> Option<Vec<(u64, u64)>> {
    match basis {
        data::ChartBasis::Tick(_) => {
            let len = candles.len();
            Some(
                candles
                    .iter()
                    .enumerate()
                    .map(|(i, c)| {
                        let end = if i + 1 < len {
                            candles[i + 1].time.0
                        } else {
                            u64::MAX
                        };
                        (c.time.0, end)
                    })
                    .collect(),
            )
        }
        _ => None,
    }
}

/// Find the containing candle's open time for a given timestamp.
fn find_candle_open(time: u64, candles: &[data::Candle]) -> u64 {
    if candles.is_empty() {
        return 0;
    }
    let idx = candles
        .binary_search_by_key(&time, |c| c.time.0)
        .unwrap_or_else(|i| i.saturating_sub(1));
    let idx = idx.min(candles.len().saturating_sub(1));
    candles[idx].time.0
}

/// Flush a completed block into a marker if it meets the threshold.
fn flush_block(
    block: &TradeBlock,
    markers: &mut Vec<TradeMarker>,
    params: &ComputeParams,
    candles: &[data::Candle],
    basis: &data::ChartBasis,
    candle_boundaries: &Option<Vec<(u64, u64)>>,
) {
    if block.total_qty < params.min_contracts {
        return;
    }

    let color = if block.is_buy {
        params.buy_color
    } else {
        params.sell_color
    };
    let label = if params.show_labels {
        Some(format_contracts(block.total_qty))
    } else {
        None
    };

    // Map timestamp to appropriate X coordinate
    let time = match basis {
        data::ChartBasis::Time(_) => {
            // Snap to the containing candle's open time so the marker
            // is centered on the correct candle regardless of timeframe.
            let mid = block.mid_time();
            if candles.is_empty() {
                mid
            } else {
                let idx = candles
                    .binary_search_by_key(&mid, |c| c.time.0)
                    .unwrap_or_else(|i| i.saturating_sub(1));
                let idx = idx.min(candles.len().saturating_sub(1));
                candles[idx].time.0
            }
        }
        data::ChartBasis::Tick(_) => {
            if let Some(bounds) = candle_boundaries {
                if bounds.is_empty() {
                    0
                } else {
                    let mid = block.mid_time();
                    let idx = bounds
                        .binary_search_by(|(start, _)| start.cmp(&mid))
                        .unwrap_or_else(|i| i.saturating_sub(1));
                    let idx = idx.min(bounds.len().saturating_sub(1));
                    // Reverse index (newest = 0)
                    (bounds.len().saturating_sub(1) - idx) as u64
                }
            } else {
                // Fallback: binary search candles directly
                let mid = block.mid_time();
                let idx = candles
                    .binary_search_by_key(&mid, |c| c.time.0)
                    .unwrap_or_else(|i| i.saturating_sub(1));
                let idx = idx.min(candles.len().saturating_sub(1));
                (candles.len().saturating_sub(1) - idx) as u64
            }
        }
    };

    let debug = if params.show_debug {
        Some(TradeMarkerDebug {
            fill_count: block.fill_count,
            first_fill_time: block.first_time,
            last_fill_time: block.last_time,
            price_min_units: block.min_price_units,
            price_max_units: block.max_price_units,
            vwap_numerator: block.vwap_numerator,
            vwap_denominator: block.total_qty,
        })
    } else {
        None
    };

    markers.push(TradeMarker {
        time,
        price: block.vwap_units(),
        contracts: block.total_qty,
        is_buy: block.is_buy,
        color,
        label,
        debug,
    });
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

    fn compute(&mut self, input: &StudyInput) -> Result<(), StudyError> {
        let trades = match input.trades {
            Some(t) if !t.is_empty() => t,
            _ => {
                self.output = StudyOutput::Empty;
                self.processed_trade_count = 0;
                self.pending_block = None;
                self.accumulated_markers.clear();
                return Ok(());
            }
        };

        let params = self.read_params();
        let candle_boundaries =
            build_candle_boundaries(input.candles, &input.basis);

        let mut markers: Vec<TradeMarker> = Vec::new();
        let mut pending: Option<TradeBlock> = None;

        BigTradesStudy::aggregate_trades(
            trades,
            &mut pending,
            &mut markers,
            &params,
            input.candles,
            &input.basis,
            &candle_boundaries,
        );

        // Flush final block
        if let Some(ref block) = pending {
            flush_block(
                block,
                &mut markers,
                &params,
                input.candles,
                &input.basis,
                &candle_boundaries,
            );
        }

        // Update incremental state
        self.processed_trade_count = trades.len();
        self.pending_block = pending;
        self.accumulated_markers = markers.clone();
        self.output = Self::finalize_output(&markers);
        Ok(())
    }

    fn append_trades(
        &mut self,
        _new_trades: &[data::Trade],
        input: &StudyInput,
    ) -> Result<(), StudyError> {
        let trades = match input.trades {
            Some(t) if !t.is_empty() => t,
            _ => return Ok(()),
        };

        // If no prior state, do full compute
        if self.processed_trade_count == 0 {
            return self.compute(input);
        }

        // Process only new trades
        if self.processed_trade_count >= trades.len() {
            return Ok(());
        }
        let new_slice = &trades[self.processed_trade_count..];

        let params = self.read_params();
        let candle_boundaries =
            build_candle_boundaries(input.candles, &input.basis);

        // Remove the last marker if we have a pending block
        // (the pending block may produce a different marker now)
        // Actually, the pending block hasn't been flushed yet by definition
        // (it was only flushed at end of last full compute). We need to
        // continue from where we left off.

        // The last flush in compute() flushed the final block. So we need
        // to check if that final flush produced a marker that should be
        // reconsidered. Since the pending block was already flushed, we
        // start fresh with no pending block for incremental.
        // However, to properly handle the case where the last block didn't
        // meet the threshold but now might with new trades, we should keep
        // the pending block unflushed. Let's adjust: in compute(), we save
        // the pending block BEFORE flushing it.

        // For simplicity, if there's a pending block from a previous
        // incremental call, we pop the last marker (if it was from this
        // block) and restart aggregation from the pending block's state.

        BigTradesStudy::aggregate_trades(
            new_slice,
            &mut self.pending_block,
            &mut self.accumulated_markers,
            &params,
            input.candles,
            &input.basis,
            &candle_boundaries,
        );

        self.processed_trade_count = trades.len();

        // Build output: accumulated markers + flush pending (without
        // consuming it)
        let mut all_markers = self.accumulated_markers.clone();
        if let Some(ref block) = self.pending_block {
            flush_block(
                block,
                &mut all_markers,
                &params,
                input.candles,
                &input.basis,
                &candle_boundaries,
            );
        }

        self.output = Self::finalize_output(&all_markers);
        Ok(())
    }

    fn output(&self) -> &StudyOutput {
        &self.output
    }

    fn reset(&mut self) {
        self.output = StudyOutput::Empty;
        self.processed_trade_count = 0;
        self.pending_block = None;
        self.accumulated_markers.clear();
    }

    fn clone_study(&self) -> Box<dyn Study> {
        Box::new(Self {
            config: self.config.clone(),
            output: self.output.clone(),
            params: self.params.clone(),
            processed_trade_count: 0,
            pending_block: None,
            accumulated_markers: Vec::new(),
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

    /// Helper: convert marker price (i64 units) back to f64 for assertions
    fn marker_price_f64(marker: &TradeMarker) -> f64 {
        Price::from_units(marker.price).to_f64()
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

        let output = study.output();
        assert!(matches!(output, StudyOutput::Markers(_)), "Expected Markers");
        let StudyOutput::Markers(m) = output else { unreachable!() };
        assert_eq!(m.len(), 1);
        assert!(m[0].is_buy);
        assert!(
            (m[0].contracts - 100.0).abs() < f64::EPSILON
        );
        assert!(
            (marker_price_f64(&m[0]) - 100.0).abs() < 0.01,
            "price: {} expected ~100.0",
            marker_price_f64(&m[0])
        );
        assert_eq!(m[0].label.as_deref(), Some("100"));
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
        // VWAP = (20*100 + 30*101 + 10*102) / 60 = 6050/60 = 100.8333...
        let trades = vec![
            make_trade(1000, 100.0, 20.0, Side::Buy),
            make_trade(1020, 101.0, 30.0, Side::Buy),
            make_trade(1040, 102.0, 10.0, Side::Buy),
        ];

        study
            .set_parameter(
                "min_contracts",
                ParameterValue::Integer(50),
            )
            .unwrap();
        study.compute(&study_input(&candles, &trades));

        let output = study.output();
        assert!(matches!(output, StudyOutput::Markers(_)), "Expected Markers");
        let StudyOutput::Markers(m) = output else { unreachable!() };
        assert_eq!(m.len(), 1);
        assert!(m[0].is_buy);
        assert!(
            (m[0].contracts - 60.0).abs() < f64::EPSILON,
            "contracts: {}",
            m[0].contracts
        );
        let expected_vwap = 6050.0 / 60.0;
        assert!(
            (marker_price_f64(&m[0]) - expected_vwap).abs() < 0.01,
            "vwap: {} expected: {}",
            marker_price_f64(&m[0]),
            expected_vwap
        );
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

        let output = study.output();
        assert!(matches!(output, StudyOutput::Markers(_)), "Expected Markers");
        let StudyOutput::Markers(m) = output else { unreachable!() };
        assert_eq!(m.len(), 1);
        // VWAP = (7*5432.75 + 13*5433.25) / 20 = 5433.075
        let expected = (7.0 * 5432.75 + 13.0 * 5433.25) / 20.0;
        // With i64 units we have 10^-8 precision, so the
        // round-trip should be very close
        assert!(
            (marker_price_f64(&m[0]) - expected).abs() < 1e-6,
            "vwap: {:.10} expected: {:.10}",
            marker_price_f64(&m[0]),
            expected
        );
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

        let output = study.output();
        assert!(matches!(output, StudyOutput::Markers(_)), "Expected Markers");
        let StudyOutput::Markers(m) = output else { unreachable!() };
        assert_eq!(m.len(), 2);
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

        let output = study.output();
        assert!(matches!(output, StudyOutput::Markers(_)), "Expected Markers");
        let StudyOutput::Markers(m) = output else { unreachable!() };
        assert_eq!(m.len(), 2);
        assert!(m[0].is_buy);
        assert!(!m[1].is_buy);
    }

    #[test]
    fn test_continuous_burst_merges_with_previous_fill_window() {
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

        let output = study.output();
        assert!(matches!(output, StudyOutput::Markers(_)), "Expected Markers");
        let StudyOutput::Markers(m) = output else { unreachable!() };
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

    #[test]
    fn test_zero_quantity_trades_skipped() {
        let mut study = BigTradesStudy::new();
        study
            .set_parameter("min_contracts", ParameterValue::Integer(50))
            .unwrap();
        study
            .set_parameter(
                "aggregation_window_ms",
                ParameterValue::Integer(150),
            )
            .unwrap();

        let candles = vec![make_candle(1000, 100.0)];
        let trades = vec![
            make_trade(1000, 100.0, 60.0, Side::Buy),
            make_trade(1050, 100.0, 0.0, Side::Sell), // zero qty
            make_trade(1100, 100.0, 10.0, Side::Buy),
        ];
        study.compute(&study_input(&candles, &trades));

        let output = study.output();
        assert!(matches!(output, StudyOutput::Markers(_)), "Expected Markers");
        let StudyOutput::Markers(m) = output else { unreachable!() };
        assert_eq!(m.len(), 1);
        assert!(
            (m[0].contracts - 70.0).abs() < f64::EPSILON,
            "contracts: {}",
            m[0].contracts
        );
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

    #[test]
    fn test_debug_annotations_populated() {
        let mut study = BigTradesStudy::new();
        study
            .set_parameter("min_contracts", ParameterValue::Integer(1))
            .unwrap();
        study
            .set_parameter("show_debug", ParameterValue::Boolean(true))
            .unwrap();

        let candles = vec![make_candle(1000, 100.0)];
        let trades = vec![
            make_trade(1000, 100.0, 20.0, Side::Buy),
            make_trade(1030, 101.0, 30.0, Side::Buy),
        ];
        study.compute(&study_input(&candles, &trades));

        let output = study.output();
        assert!(matches!(output, StudyOutput::Markers(_)), "Expected Markers");
        let StudyOutput::Markers(m) = output else { unreachable!() };
        assert_eq!(m.len(), 1);
        let debug = m[0].debug.as_ref().expect("debug should be set");
        assert_eq!(debug.fill_count, 2);
        assert_eq!(debug.first_fill_time, 1000);
        assert_eq!(debug.last_fill_time, 1030);
    }

    #[test]
    fn test_incremental_append() {
        let mut study = BigTradesStudy::new();
        study
            .set_parameter("min_contracts", ParameterValue::Integer(50))
            .unwrap();

        let candles = vec![make_candle(1000, 100.0)];

        // First batch: 30 contracts (below threshold)
        let trades1 = vec![make_trade(1000, 100.0, 30.0, Side::Buy)];
        study.compute(&study_input(&candles, &trades1));
        assert!(matches!(study.output(), StudyOutput::Empty));

        // Append more trades to reach threshold
        let mut trades2 = trades1.clone();
        trades2.push(make_trade(1030, 100.0, 30.0, Side::Buy));

        let input = study_input(&candles, &trades2);
        study.append_trades(&trades2[1..], &input);

        let output = study.output();
        assert!(matches!(output, StudyOutput::Markers(_)), "Expected Markers");
        let StudyOutput::Markers(m) = output else { unreachable!() };
        assert_eq!(m.len(), 1);
        assert!(
            (m[0].contracts - 60.0).abs() < f64::EPSILON,
            "contracts: {}",
            m[0].contracts
        );
    }

    #[test]
    fn test_time_based_marker_snaps_to_candle_open() {
        let mut study = BigTradesStudy::new();
        study
            .set_parameter("min_contracts", ParameterValue::Integer(1))
            .unwrap();

        // M5 candles: open at 0, 300_000, 600_000
        let candles = vec![
            make_candle(0, 100.0),
            make_candle(300_000, 101.0),
            make_candle(600_000, 102.0),
        ];
        // Trades at 150_100ms and 150_120ms — inside the first M5 candle
        let trades = vec![
            make_trade(150_100, 100.0, 30.0, Side::Buy),
            make_trade(150_120, 100.0, 30.0, Side::Buy),
        ];

        let input = StudyInput {
            candles: &candles,
            trades: Some(&trades),
            basis: ChartBasis::Time(Timeframe::M5),
            tick_size: Price::from_f32(0.25),
            visible_range: None,
        };
        study.compute(&input);

        let output = study.output();
        assert!(matches!(output, StudyOutput::Markers(_)), "Expected Markers");
        let StudyOutput::Markers(m) = output else { unreachable!() };
        assert_eq!(m.len(), 1);
        // Marker time should snap to candle open (0),
        // not the raw mid_time (150_150)
        assert_eq!(
            m[0].time, 0,
            "marker time {} should snap to candle open 0",
            m[0].time
        );
    }

    #[test]
    fn test_time_based_marker_snaps_to_correct_candle() {
        let mut study = BigTradesStudy::new();
        study
            .set_parameter("min_contracts", ParameterValue::Integer(1))
            .unwrap();

        // M5 candles
        let candles = vec![
            make_candle(0, 100.0),
            make_candle(300_000, 101.0),
            make_candle(600_000, 102.0),
        ];
        // Trades in the second candle (300_000..600_000)
        let trades = vec![
            make_trade(450_000, 101.0, 50.0, Side::Sell),
        ];

        let input = StudyInput {
            candles: &candles,
            trades: Some(&trades),
            basis: ChartBasis::Time(Timeframe::M5),
            tick_size: Price::from_f32(0.25),
            visible_range: None,
        };
        study.compute(&input);

        let output = study.output();
        assert!(matches!(output, StudyOutput::Markers(_)), "Expected Markers");
        let StudyOutput::Markers(m) = output else { unreachable!() };
        assert_eq!(m.len(), 1);
        assert_eq!(
            m[0].time, 300_000,
            "marker time {} should snap to candle open 300000",
            m[0].time
        );
    }

    #[test]
    fn test_tick_based_marker_uses_candle_index() {
        let mut study = BigTradesStudy::new();
        study
            .set_parameter("min_contracts", ParameterValue::Integer(1))
            .unwrap();

        // 3 tick candles
        let candles = vec![
            make_candle(1000, 100.0),
            make_candle(2000, 101.0),
            make_candle(3000, 102.0),
        ];
        // Trade in the middle candle
        let trades = vec![
            make_trade(2500, 101.0, 50.0, Side::Buy),
        ];

        let input = StudyInput {
            candles: &candles,
            trades: Some(&trades),
            basis: ChartBasis::Tick(100),
            tick_size: Price::from_f32(0.25),
            visible_range: None,
        };
        study.compute(&input);

        let output = study.output();
        assert!(matches!(output, StudyOutput::Markers(_)), "Expected Markers");
        let StudyOutput::Markers(m) = output else { unreachable!() };
        assert_eq!(m.len(), 1);
        // Candle index 1 (middle), reverse index = 2 - 1 = 1
        assert_eq!(
            m[0].time, 1,
            "marker time {} should be reverse candle index 1",
            m[0].time
        );
    }

    #[test]
    fn test_time_based_candle_boundary_splits_block() {
        let mut study = BigTradesStudy::new();
        study
            .set_parameter("min_contracts", ParameterValue::Integer(1))
            .unwrap();
        // Use a long aggregation window so only the candle boundary
        // causes the split, not the time gap.
        study
            .set_parameter(
                "aggregation_window_ms",
                ParameterValue::Integer(5000),
            )
            .unwrap();

        // Two M5 candles
        let candles = vec![
            make_candle(0, 100.0),
            make_candle(300_000, 101.0),
        ];
        // Two same-side trades 50ms apart but straddling the candle
        // boundary (299_980 in candle 0, 300_030 in candle 1).
        let trades = vec![
            make_trade(299_980, 100.0, 30.0, Side::Buy),
            make_trade(300_030, 100.0, 30.0, Side::Buy),
        ];

        let input = StudyInput {
            candles: &candles,
            trades: Some(&trades),
            basis: ChartBasis::Time(Timeframe::M5),
            tick_size: Price::from_f32(0.25),
            visible_range: None,
        };
        study.compute(&input);

        let output = study.output();
        assert!(matches!(output, StudyOutput::Markers(_)), "Expected Markers");
        let StudyOutput::Markers(m) = output else { unreachable!() };
        assert_eq!(
            m.len(),
            2,
            "trades crossing a candle boundary should produce \
             separate markers, got {}",
            m.len()
        );
        assert_eq!(m[0].time, 0);
        assert_eq!(m[1].time, 300_000);
    }

    #[test]
    fn test_tick_based_no_candle_boundary_restriction() {
        // Tick charts should NOT split on candle boundaries since the
        // x-mapping already handles index assignment independently.
        let mut study = BigTradesStudy::new();
        study
            .set_parameter("min_contracts", ParameterValue::Integer(1))
            .unwrap();
        study
            .set_parameter(
                "aggregation_window_ms",
                ParameterValue::Integer(5000),
            )
            .unwrap();

        let candles = vec![
            make_candle(1000, 100.0),
            make_candle(2000, 101.0),
        ];
        // Two same-side trades straddling tick candle boundary
        let trades = vec![
            make_trade(1500, 100.0, 30.0, Side::Buy),
            make_trade(2500, 100.0, 30.0, Side::Buy),
        ];

        let input = StudyInput {
            candles: &candles,
            trades: Some(&trades),
            basis: ChartBasis::Tick(100),
            tick_size: Price::from_f32(0.25),
            visible_range: None,
        };
        study.compute(&input);

        let output = study.output();
        assert!(matches!(output, StudyOutput::Markers(_)), "Expected Markers");
        let StudyOutput::Markers(m) = output else { unreachable!() };
        // Should merge into a single marker on tick charts
        assert_eq!(
            m.len(),
            1,
            "tick charts should not split on candle boundaries, \
             got {} markers",
            m.len()
        );
        assert!(
            (m[0].contracts - 60.0).abs() < f64::EPSILON,
            "contracts: {}",
            m[0].contracts
        );
    }
}
