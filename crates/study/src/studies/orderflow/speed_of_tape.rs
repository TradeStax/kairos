//! Speed of Tape study — trade activity per time bucket visualization.
//!
//! Aggregates trade volume (or count) into configurable N-second
//! buckets within each candle period, then extracts OHLC directly
//! from the raw bucket values (open = first, high = max, low = min,
//! close = last). Rendered as mini-candlesticks in a panel below
//! the main chart. Giant candles = initiative/bursty activity.
//!
//! Each candle is colored green (buy-dominant) or purple
//! (sell-dominant) based on raw trade counts.

use crate::BULLISH_COLOR;
use crate::config::{
    DisplayFormat, ParameterDef, ParameterKind, ParameterSection, ParameterTab, ParameterValue,
    StudyConfig, Visibility,
};
use crate::core::{Study, StudyCategory, StudyInput, StudyPlacement};
use crate::error::StudyError;
use crate::output::{StudyCandlePoint, StudyCandleSeries, StudyOutput};
use crate::util::candle_key;
use data::{ChartBasis, SerializableColor, Side};

const DEFAULT_BUCKET_SECONDS: i64 = 10;
const DEFAULT_FILTER_MIN: i64 = 1;
const DEFAULT_FILTER_MAX: i64 = 0;
const DEFAULT_STDDEV_FILTER: f64 = 2.0;

const DEFAULT_BUY_COLOR: SerializableColor = BULLISH_COLOR;

/// Default sell color — purple #8C52AF.
const DEFAULT_SELL_COLOR: SerializableColor = SerializableColor::from_rgb8_const(140, 82, 175);

const DEFAULT_BODY_OPACITY: f64 = 0.5;
const DEFAULT_BORDER_OPACITY: f64 = 1.0;

/// Measures trade activity per time bucket as OHLC
/// mini-candlesticks.
///
/// For each candle, the study:
/// 1. Finds trades within the candle's time range via cursor
/// 2. Filters trades by size (min/max)
/// 3. Aggregates volume or count into N-second buckets
/// 4. Optionally caps outlier buckets via stddev filter
/// 5. Extracts OHLC from raw bucket values (first/max/min/last)
pub struct SpeedOfTapeStudy {
    config: StudyConfig,
    output: StudyOutput,
    params: Vec<ParameterDef>,
    /// Reusable scratch buffer for time-bucketed values.
    bucket_buf: Vec<f32>,
}

impl SpeedOfTapeStudy {
    pub fn new() -> Self {
        let params = vec![
            // ── Data Settings (order: 0) ──────────────────
            ParameterDef {
                key: "input_data".into(),
                label: "Input Data".into(),
                description: "Measure volume or trade count per bucket".into(),
                kind: ParameterKind::Choice {
                    options: &["Volume", "Trades"],
                },
                default: ParameterValue::Choice("Volume".into()),
                tab: ParameterTab::Parameters,
                section: Some(ParameterSection {
                    label: "Data Settings",
                    order: 0,
                }),
                order: 0,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
            ParameterDef {
                key: "filter_min".into(),
                label: "Filter Min".into(),
                description: "Min trade size to include (0 = none)".into(),
                kind: ParameterKind::Integer { min: 0, max: 10000 },
                default: ParameterValue::Integer(DEFAULT_FILTER_MIN),
                tab: ParameterTab::Parameters,
                section: Some(ParameterSection {
                    label: "Data Settings",
                    order: 0,
                }),
                order: 1,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
            ParameterDef {
                key: "filter_max".into(),
                label: "Filter Max".into(),
                description: "Max trade size to include (0 = none)".into(),
                kind: ParameterKind::Integer { min: 0, max: 10000 },
                default: ParameterValue::Integer(DEFAULT_FILTER_MAX),
                tab: ParameterTab::Parameters,
                section: Some(ParameterSection {
                    label: "Data Settings",
                    order: 0,
                }),
                order: 2,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
            // ── Mode (order: 1) ───────────────────────────
            ParameterDef {
                key: "display_value".into(),
                label: "Display Value".into(),
                description: "Which side of activity to display".into(),
                kind: ParameterKind::Choice {
                    options: &["Total", "Buy", "Sell", "Delta"],
                },
                default: ParameterValue::Choice("Total".into()),
                tab: ParameterTab::Parameters,
                section: Some(ParameterSection {
                    label: "Mode",
                    order: 1,
                }),
                order: 0,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
            ParameterDef {
                key: "bucket_seconds".into(),
                label: "Bucket Seconds".into(),
                description: "Bucket time window in seconds".into(),
                kind: ParameterKind::Integer { min: 1, max: 120 },
                default: ParameterValue::Integer(DEFAULT_BUCKET_SECONDS),
                tab: ParameterTab::Parameters,
                section: Some(ParameterSection {
                    label: "Mode",
                    order: 1,
                }),
                order: 1,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
            // ── Filter (order: 2) ─────────────────────────
            ParameterDef {
                key: "filter_mode".into(),
                label: "Filter Mode".into(),
                description: "Outlier filtering mode".into(),
                kind: ParameterKind::Choice {
                    options: &["None", "Automatic"],
                },
                default: ParameterValue::Choice("Automatic".into()),
                tab: ParameterTab::Parameters,
                section: Some(ParameterSection {
                    label: "Filter",
                    order: 2,
                }),
                order: 0,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
            ParameterDef {
                key: "stddev_filter".into(),
                label: "StdDev Multiplier".into(),
                description: "Cap at mean + mult × stddev".into(),
                kind: ParameterKind::Float {
                    min: 0.5,
                    max: 5.0,
                    step: 0.1,
                },
                default: ParameterValue::Float(DEFAULT_STDDEV_FILTER),
                tab: ParameterTab::Parameters,
                section: Some(ParameterSection {
                    label: "Filter",
                    order: 2,
                }),
                order: 1,
                format: DisplayFormat::Auto,
                visible_when: Visibility::WhenChoice {
                    key: "filter_mode",
                    equals: "Automatic",
                },
            },
            // ── Style ─────────────────────────────────────
            ParameterDef {
                key: "buy_color".into(),
                label: "Buy Color".into(),
                description: "Color for buy-dominant candles".into(),
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
                description: "Color for sell-dominant candles".into(),
                kind: ParameterKind::Color,
                default: ParameterValue::Color(DEFAULT_SELL_COLOR),
                tab: ParameterTab::Style,
                section: None,
                order: 1,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
            ParameterDef {
                key: "body_opacity".into(),
                label: "Body Opacity".into(),
                description: "Opacity of the candle body fill".into(),
                kind: ParameterKind::Float {
                    min: 0.0,
                    max: 1.0,
                    step: 0.05,
                },
                default: ParameterValue::Float(DEFAULT_BODY_OPACITY),
                tab: ParameterTab::Style,
                section: None,
                order: 2,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
            ParameterDef {
                key: "border_opacity".into(),
                label: "Border Opacity".into(),
                description: "Opacity of the candle wick and outline".into(),
                kind: ParameterKind::Float {
                    min: 0.0,
                    max: 1.0,
                    step: 0.05,
                },
                default: ParameterValue::Float(DEFAULT_BORDER_OPACITY),
                tab: ParameterTab::Style,
                section: None,
                order: 3,
                format: DisplayFormat::Auto,
                visible_when: Visibility::Always,
            },
        ];

        let mut config = StudyConfig::new("speed_of_tape");
        for p in &params {
            config.set(p.key.clone(), p.default.clone());
        }

        Self {
            config,
            output: StudyOutput::Empty,
            params,
            bucket_buf: Vec::new(),
        }
    }
}

impl Default for SpeedOfTapeStudy {
    fn default() -> Self {
        Self::new()
    }
}

/// Binary search for the first trade at or after `target_ms`.
fn lower_bound(trades: &[data::Trade], target_ms: u64) -> usize {
    trades.partition_point(|t| t.time.0 < target_ms)
}

/// Extract OHLC from raw bucket values.
///
/// When `skip_zeros` is true (use for Total/Buy/Sell modes),
/// zero-valued buckets are ignored so the low doesn't always
/// collapse to 0 due to empty time windows.
/// When false (use for Delta mode), all values including zero
/// are considered since zero delta is a meaningful data point.
///
/// Returns all zeros if no qualifying buckets exist.
fn extract_ohlc(buckets: &[f32], skip_zeros: bool) -> (f32, f32, f32, f32) {
    let mut open = 0.0f32;
    let mut close = 0.0f32;
    let mut high = f32::NEG_INFINITY;
    let mut low = f32::INFINITY;
    let mut found_any = false;

    for &v in buckets {
        if skip_zeros && v == 0.0 {
            continue;
        }
        if !found_any {
            open = v;
            found_any = true;
        }
        close = v;
        if v > high {
            high = v;
        }
        if v < low {
            low = v;
        }
    }

    if !found_any {
        return (0.0, 0.0, 0.0, 0.0);
    }
    (open, high, low, close)
}

/// Cap each bucket value at `mean + multiplier * stddev`.
///
/// Requires at least 2 buckets to compute a meaningful stddev.
fn apply_stddev_filter(buckets: &mut [f32], multiplier: f32) {
    let n = buckets.len();
    if n < 2 {
        return;
    }

    let sum: f32 = buckets.iter().sum();
    let mean = sum / n as f32;
    let variance = buckets
        .iter()
        .map(|&v| {
            let d = v - mean;
            d * d
        })
        .sum::<f32>()
        / n as f32;
    let stddev = variance.sqrt();

    let cap = mean + multiplier * stddev;
    for v in buckets.iter_mut() {
        if *v > cap {
            *v = cap;
        }
    }
}

impl Study for SpeedOfTapeStudy {
    fn id(&self) -> &str {
        "speed_of_tape"
    }

    fn name(&self) -> &str {
        "Speed of Tape"
    }

    fn category(&self) -> StudyCategory {
        StudyCategory::OrderFlow
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
        let trades = match input.trades {
            Some(t) if !t.is_empty() => t,
            _ => {
                self.output = StudyOutput::Empty;
                return Ok(());
            }
        };

        if input.candles.is_empty() {
            self.output = StudyOutput::Empty;
            return Ok(());
        }

        // Read parameters
        let input_data = self.config.get_choice("input_data", "Volume");
        let display_value = self.config.get_choice("display_value", "Total");
        let bucket_seconds = self
            .config
            .get_int("bucket_seconds", DEFAULT_BUCKET_SECONDS)
            .max(1);
        let filter_min = self.config.get_int("filter_min", DEFAULT_FILTER_MIN).max(0);
        let filter_max = self.config.get_int("filter_max", DEFAULT_FILTER_MAX).max(0);
        let filter_mode = self.config.get_choice("filter_mode", "Automatic");
        let stddev_mult = self
            .config
            .get_float("stddev_filter", DEFAULT_STDDEV_FILTER) as f32;

        let use_volume = input_data == "Volume";
        let bucket_ms = (bucket_seconds * 1000) as u64;

        let buy_color = self.config.get_color("buy_color", DEFAULT_BUY_COLOR);
        let sell_color = self.config.get_color("sell_color", DEFAULT_SELL_COLOR);
        let body_opacity = self.config.get_float("body_opacity", DEFAULT_BODY_OPACITY) as f32;
        let border_opacity =
            self.config
                .get_float("border_opacity", DEFAULT_BORDER_OPACITY) as f32;

        // Precompute all 4 color variants
        let buy_body = buy_color.with_alpha(body_opacity);
        let buy_border = buy_color.with_alpha(border_opacity);
        let sell_body = sell_color.with_alpha(body_opacity);
        let sell_border = sell_color.with_alpha(border_opacity);

        let total = input.candles.len();
        let mut points = Vec::with_capacity(total);

        // Running trade cursor (CRITICAL-2)
        let first_start = input.candles.first().map_or(0, |c| c.time.0);
        let mut trade_cursor = lower_bound(trades, first_start);

        for (ci, candle) in input.candles.iter().enumerate() {
            let key = candle_key(candle, ci, total, &input.basis);

            let start_ms = candle.time.0;
            let end_ms = match &input.basis {
                ChartBasis::Time(tf) => start_ms + tf.to_milliseconds(),
                ChartBasis::Tick(_) => {
                    if ci + 1 < total {
                        input.candles[ci + 1].time.0
                    } else {
                        trades.last().map_or(start_ms + 1000, |t| t.time.0 + 1)
                    }
                }
            };

            if end_ms <= start_ms {
                points.push(StudyCandlePoint {
                    x: key,
                    open: 0.0,
                    high: 0.0,
                    low: 0.0,
                    close: 0.0,
                    body_color: buy_body,
                    border_color: buy_border,
                });
                continue;
            }

            // Search from cursor (CRITICAL-2)
            let lo = trade_cursor;
            let hi = lower_bound(&trades[lo..], end_ms) + lo;
            trade_cursor = hi;

            let slice = &trades[lo..hi];

            if slice.is_empty() {
                points.push(StudyCandlePoint {
                    x: key,
                    open: 0.0,
                    high: 0.0,
                    low: 0.0,
                    close: 0.0,
                    body_color: buy_body,
                    border_color: buy_border,
                });
                continue;
            }

            // Bucket setup
            let duration_ms = end_ms - start_ms;
            let num_buckets = duration_ms.div_ceil(bucket_ms).max(1) as usize;

            // Reuse bucket buffer (CRITICAL-1)
            self.bucket_buf.clear();
            self.bucket_buf.resize(num_buckets, 0.0);

            // Fused buy/sell counting + bucketing (CRITICAL-3)
            let mut buy_count = 0u32;
            let mut sell_count = 0u32;

            for t in slice {
                let qty = t.quantity.0;

                // Trade size filter
                if filter_min > 0 && (qty as i64) < filter_min {
                    continue;
                }
                if filter_max > 0 && (qty as i64) > filter_max {
                    continue;
                }

                let is_buy = matches!(t.side, Side::Buy | Side::Ask);

                // Always count for color determination
                if is_buy {
                    buy_count += 1;
                } else {
                    sell_count += 1;
                }

                let contribution = if use_volume { qty as f32 } else { 1.0 };

                let idx = ((t.time.0.saturating_sub(start_ms)) / bucket_ms) as usize;
                let idx = idx.min(num_buckets - 1);

                match display_value {
                    "Total" => {
                        self.bucket_buf[idx] += contribution;
                    }
                    "Buy" => {
                        if is_buy {
                            self.bucket_buf[idx] += contribution;
                        }
                    }
                    "Sell" => {
                        if !is_buy {
                            self.bucket_buf[idx] += contribution;
                        }
                    }
                    _ => {
                        if is_buy {
                            self.bucket_buf[idx] += contribution;
                        } else {
                            self.bucket_buf[idx] -= contribution;
                        }
                    }
                }
            }

            // Stddev filter
            if filter_mode == "Automatic" {
                apply_stddev_filter(&mut self.bucket_buf, stddev_mult);
            }

            // OHLC from bucket values — skip zeros for
            // Total/Buy/Sell (empty buckets are no-data);
            // include zeros for Delta (zero = balanced)
            let skip_zeros = display_value != "Delta";
            let (open, high, low, close) = extract_ohlc(&self.bucket_buf, skip_zeros);

            let is_buy = buy_count >= sell_count;
            let (body_color, border_color) = if is_buy {
                (buy_body, buy_border)
            } else {
                (sell_body, sell_border)
            };

            points.push(StudyCandlePoint {
                x: key,
                open,
                high,
                low,
                close,
                body_color,
                border_color,
            });
        }

        self.output = if points.is_empty() {
            StudyOutput::Empty
        } else {
            StudyOutput::StudyCandles(vec![StudyCandleSeries {
                label: "Speed of Tape".to_string(),
                points,
            }])
        };

        Ok(())
    }

    fn output(&self) -> &StudyOutput {
        &self.output
    }

    fn reset(&mut self) {
        self.output = StudyOutput::Empty;
        self.bucket_buf.clear();
    }

    fn clone_study(&self) -> Box<dyn Study> {
        Box::new(Self {
            config: self.config.clone(),
            output: self.output.clone(),
            params: self.params.clone(),
            bucket_buf: Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use data::{Candle, ChartBasis, Price, Quantity, Timeframe, Timestamp, Trade, Volume};

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

    fn make_trade(time: u64, side: Side, qty: f64) -> Trade {
        Trade::new(
            Timestamp::from_millis(time),
            Price::from_f32(100.0),
            Quantity(qty),
            side,
        )
    }

    fn make_input<'a>(candles: &'a [Candle], trades: &'a [Trade]) -> StudyInput<'a> {
        StudyInput {
            candles,
            trades: Some(trades),
            basis: ChartBasis::Time(Timeframe::M1),
            tick_size: Price::from_f32(0.25),
            visible_range: None,
        }
    }

    fn set_params(study: &mut SpeedOfTapeStudy, pairs: &[(&str, ParameterValue)]) {
        for (key, val) in pairs {
            study.config.set((*key).to_string(), val.clone());
        }
    }

    // ── Unit tests for helpers ────────────────────────────

    #[test]
    fn test_extract_ohlc() {
        // All non-zero: straightforward
        let buckets = [10.0, 30.0, 5.0, 20.0];
        let (o, h, l, c) = extract_ohlc(&buckets, true);
        assert_eq!(o, 10.0);
        assert_eq!(h, 30.0);
        assert_eq!(l, 5.0);
        assert_eq!(c, 20.0);
    }

    #[test]
    fn test_extract_ohlc_skips_zeros() {
        // Zero buckets should be ignored when skip_zeros=true
        let buckets = [0.0, 10.0, 30.0, 0.0, 5.0, 0.0];
        let (o, h, l, c) = extract_ohlc(&buckets, true);
        assert_eq!(o, 10.0); // first non-zero
        assert_eq!(h, 30.0); // max non-zero
        assert_eq!(l, 5.0); // min non-zero
        assert_eq!(c, 5.0); // last non-zero
    }

    #[test]
    fn test_extract_ohlc_includes_zeros_for_delta() {
        // skip_zeros=false: zeros are included (for Delta mode)
        let buckets = [0.0, 10.0, -5.0, 0.0];
        let (o, h, l, c) = extract_ohlc(&buckets, false);
        assert_eq!(o, 0.0); // first value
        assert_eq!(h, 10.0); // max
        assert_eq!(l, -5.0); // min
        assert_eq!(c, 0.0); // last value
    }

    #[test]
    fn test_extract_ohlc_all_zeros() {
        // skip_zeros=true with all zeros → no data
        let buckets = [0.0, 0.0, 0.0];
        let (o, h, l, c) = extract_ohlc(&buckets, true);
        assert_eq!((o, h, l, c), (0.0, 0.0, 0.0, 0.0));
    }

    #[test]
    fn test_extract_ohlc_empty() {
        let (o, h, l, c) = extract_ohlc(&[], true);
        assert_eq!((o, h, l, c), (0.0, 0.0, 0.0, 0.0));
    }

    #[test]
    fn test_apply_stddev_filter() {
        // 9 normal values of 10 + 1 outlier of 100
        let mut buckets = vec![10.0; 9];
        buckets.push(100.0);
        let mean_before: f32 = buckets.iter().sum::<f32>() / 10.0; // 19.0
        apply_stddev_filter(&mut buckets, 1.0);
        // The outlier should be capped
        assert!(
            buckets[9] < 100.0,
            "outlier should be capped, got {}",
            buckets[9]
        );
        // Normal values should be unchanged
        for &v in &buckets[..9] {
            assert!(v <= mean_before + 50.0, "normal values shouldn't grow");
        }
    }

    // ── Integration tests ─────────────────────────────────

    #[test]
    fn test_volume_total() {
        let mut study = SpeedOfTapeStudy::new();
        set_params(
            &mut study,
            &[
                ("input_data", ParameterValue::Choice("Volume".into())),
                ("display_value", ParameterValue::Choice("Total".into())),
                ("bucket_seconds", ParameterValue::Integer(10)),
                ("filter_mode", ParameterValue::Choice("None".into())),
            ],
        );

        let candles = vec![make_candle(0, 100.0, 101.0, 99.0, 100.5, 50.0, 50.0)];
        // 6 buckets in 60s at 10s each
        // Bucket 0 (0–10s): qty 5 + 3 = 8
        // Bucket 1 (10–20s): qty 10 = 10
        // Bucket 2 (20–30s): qty 2 = 2
        // Buckets 3–5: empty (skipped by extract_ohlc)
        let trades = vec![
            make_trade(1000, Side::Buy, 5.0),
            make_trade(5000, Side::Sell, 3.0),
            make_trade(15000, Side::Buy, 10.0),
            make_trade(25000, Side::Sell, 2.0),
        ];

        let input = make_input(&candles, &trades);
        study.compute(&input).unwrap();

        match study.output() {
            StudyOutput::StudyCandles(series) => {
                let pt = &series[0].points[0];
                // open = first non-zero bucket = 8
                assert_eq!(pt.open, 8.0);
                // high = max non-zero bucket = 10
                assert_eq!(pt.high, 10.0);
                // low = min non-zero bucket = 2
                assert_eq!(pt.low, 2.0);
                // close = last non-zero bucket = 2
                assert_eq!(pt.close, 2.0);
            }
            other => panic!(
                "Expected StudyCandles, got {:?}",
                std::mem::discriminant(other)
            ),
        }
    }

    #[test]
    fn test_volume_buy_only() {
        let mut study = SpeedOfTapeStudy::new();
        set_params(
            &mut study,
            &[
                ("input_data", ParameterValue::Choice("Volume".into())),
                ("display_value", ParameterValue::Choice("Buy".into())),
                ("bucket_seconds", ParameterValue::Integer(30)),
                ("filter_mode", ParameterValue::Choice("None".into())),
            ],
        );

        let candles = vec![make_candle(0, 100.0, 101.0, 99.0, 100.5, 50.0, 50.0)];
        // 2 buckets at 30s: bucket 0, bucket 1
        let trades = vec![
            make_trade(5000, Side::Buy, 10.0),
            make_trade(10000, Side::Sell, 20.0),
            make_trade(35000, Side::Buy, 7.0),
        ];

        let input = make_input(&candles, &trades);
        study.compute(&input).unwrap();

        match study.output() {
            StudyOutput::StudyCandles(series) => {
                let pt = &series[0].points[0];
                // Bucket 0: only buy = 10
                // Bucket 1: only buy = 7
                assert_eq!(pt.open, 10.0);
                assert_eq!(pt.high, 10.0);
                assert_eq!(pt.low, 7.0);
                assert_eq!(pt.close, 7.0);
            }
            other => panic!(
                "Expected StudyCandles, got {:?}",
                std::mem::discriminant(other)
            ),
        }
    }

    #[test]
    fn test_volume_sell_only() {
        let mut study = SpeedOfTapeStudy::new();
        set_params(
            &mut study,
            &[
                ("input_data", ParameterValue::Choice("Volume".into())),
                ("display_value", ParameterValue::Choice("Sell".into())),
                ("bucket_seconds", ParameterValue::Integer(30)),
                ("filter_mode", ParameterValue::Choice("None".into())),
            ],
        );

        let candles = vec![make_candle(0, 100.0, 101.0, 99.0, 100.5, 50.0, 50.0)];
        let trades = vec![
            make_trade(5000, Side::Buy, 10.0),
            make_trade(10000, Side::Sell, 20.0),
            make_trade(35000, Side::Sell, 5.0),
        ];

        let input = make_input(&candles, &trades);
        study.compute(&input).unwrap();

        match study.output() {
            StudyOutput::StudyCandles(series) => {
                let pt = &series[0].points[0];
                // Bucket 0: sell = 20
                // Bucket 1: sell = 5
                assert_eq!(pt.open, 20.0);
                assert_eq!(pt.high, 20.0);
                assert_eq!(pt.low, 5.0);
                assert_eq!(pt.close, 5.0);
            }
            other => panic!(
                "Expected StudyCandles, got {:?}",
                std::mem::discriminant(other)
            ),
        }
    }

    #[test]
    fn test_volume_delta() {
        let mut study = SpeedOfTapeStudy::new();
        set_params(
            &mut study,
            &[
                ("input_data", ParameterValue::Choice("Volume".into())),
                ("display_value", ParameterValue::Choice("Delta".into())),
                ("bucket_seconds", ParameterValue::Integer(30)),
                ("filter_mode", ParameterValue::Choice("None".into())),
            ],
        );

        let candles = vec![make_candle(0, 100.0, 101.0, 99.0, 100.5, 50.0, 50.0)];
        // Bucket 0: buy 10 - sell 20 = -10
        // Bucket 1: buy 7 - sell 0 = +7
        let trades = vec![
            make_trade(5000, Side::Buy, 10.0),
            make_trade(10000, Side::Sell, 20.0),
            make_trade(35000, Side::Buy, 7.0),
        ];

        let input = make_input(&candles, &trades);
        study.compute(&input).unwrap();

        match study.output() {
            StudyOutput::StudyCandles(series) => {
                let pt = &series[0].points[0];
                assert_eq!(pt.open, -10.0);
                assert_eq!(pt.high, 7.0);
                assert_eq!(pt.low, -10.0);
                assert_eq!(pt.close, 7.0);
            }
            other => panic!(
                "Expected StudyCandles, got {:?}",
                std::mem::discriminant(other)
            ),
        }
    }

    #[test]
    fn test_trade_count_mode() {
        let mut study = SpeedOfTapeStudy::new();
        set_params(
            &mut study,
            &[
                ("input_data", ParameterValue::Choice("Trades".into())),
                ("display_value", ParameterValue::Choice("Total".into())),
                ("bucket_seconds", ParameterValue::Integer(30)),
                ("filter_mode", ParameterValue::Choice("None".into())),
            ],
        );

        let candles = vec![make_candle(0, 100.0, 101.0, 99.0, 100.5, 50.0, 50.0)];
        // Bucket 0: 2 trades (regardless of qty)
        // Bucket 1: 1 trade
        let trades = vec![
            make_trade(5000, Side::Buy, 100.0),
            make_trade(10000, Side::Sell, 200.0),
            make_trade(35000, Side::Buy, 50.0),
        ];

        let input = make_input(&candles, &trades);
        study.compute(&input).unwrap();

        match study.output() {
            StudyOutput::StudyCandles(series) => {
                let pt = &series[0].points[0];
                assert_eq!(pt.open, 2.0);
                assert_eq!(pt.high, 2.0);
                assert_eq!(pt.low, 1.0);
                assert_eq!(pt.close, 1.0);
            }
            other => panic!(
                "Expected StudyCandles, got {:?}",
                std::mem::discriminant(other)
            ),
        }
    }

    #[test]
    fn test_filter_min() {
        let mut study = SpeedOfTapeStudy::new();
        set_params(
            &mut study,
            &[
                ("input_data", ParameterValue::Choice("Volume".into())),
                ("display_value", ParameterValue::Choice("Total".into())),
                ("bucket_seconds", ParameterValue::Integer(60)),
                ("filter_min", ParameterValue::Integer(5)),
                ("filter_mode", ParameterValue::Choice("None".into())),
            ],
        );

        let candles = vec![make_candle(0, 100.0, 101.0, 99.0, 100.5, 50.0, 50.0)];
        // Only qty >= 5 should pass
        let trades = vec![
            make_trade(1000, Side::Buy, 2.0),   // filtered
            make_trade(2000, Side::Buy, 5.0),   // passes
            make_trade(3000, Side::Sell, 10.0), // passes
        ];

        let input = make_input(&candles, &trades);
        study.compute(&input).unwrap();

        match study.output() {
            StudyOutput::StudyCandles(series) => {
                let pt = &series[0].points[0];
                // Only 5 + 10 = 15 in single bucket
                assert_eq!(pt.open, 15.0);
                assert_eq!(pt.close, 15.0);
            }
            other => panic!(
                "Expected StudyCandles, got {:?}",
                std::mem::discriminant(other)
            ),
        }
    }

    #[test]
    fn test_filter_max() {
        let mut study = SpeedOfTapeStudy::new();
        set_params(
            &mut study,
            &[
                ("input_data", ParameterValue::Choice("Volume".into())),
                ("display_value", ParameterValue::Choice("Total".into())),
                ("bucket_seconds", ParameterValue::Integer(60)),
                ("filter_max", ParameterValue::Integer(10)),
                ("filter_mode", ParameterValue::Choice("None".into())),
            ],
        );

        let candles = vec![make_candle(0, 100.0, 101.0, 99.0, 100.5, 50.0, 50.0)];
        // Only qty <= 10 should pass (filter_max > 0)
        let trades = vec![
            make_trade(1000, Side::Buy, 5.0),   // passes
            make_trade(2000, Side::Buy, 10.0),  // passes
            make_trade(3000, Side::Sell, 50.0), // filtered
        ];

        let input = make_input(&candles, &trades);
        study.compute(&input).unwrap();

        match study.output() {
            StudyOutput::StudyCandles(series) => {
                let pt = &series[0].points[0];
                assert_eq!(pt.open, 15.0);
                assert_eq!(pt.close, 15.0);
            }
            other => panic!(
                "Expected StudyCandles, got {:?}",
                std::mem::discriminant(other)
            ),
        }
    }

    #[test]
    fn test_stddev_auto_filter() {
        let mut study = SpeedOfTapeStudy::new();
        set_params(
            &mut study,
            &[
                ("input_data", ParameterValue::Choice("Volume".into())),
                ("display_value", ParameterValue::Choice("Total".into())),
                ("bucket_seconds", ParameterValue::Integer(10)),
                ("filter_mode", ParameterValue::Choice("Automatic".into())),
                ("stddev_filter", ParameterValue::Float(1.0)),
            ],
        );

        let candles = vec![make_candle(0, 100.0, 101.0, 99.0, 100.5, 50.0, 50.0)];
        // 6 buckets at 10s each in 60s candle
        // Place a massive outlier in bucket 0, modest in others
        let trades = vec![
            make_trade(1000, Side::Buy, 500.0), // bucket 0
            make_trade(15000, Side::Buy, 10.0), // bucket 1
            make_trade(25000, Side::Buy, 10.0), // bucket 2
            make_trade(35000, Side::Buy, 10.0), // bucket 3
            make_trade(45000, Side::Buy, 10.0), // bucket 4
            make_trade(55000, Side::Buy, 10.0), // bucket 5
        ];

        let input = make_input(&candles, &trades);
        study.compute(&input).unwrap();

        match study.output() {
            StudyOutput::StudyCandles(series) => {
                let pt = &series[0].points[0];
                // The high should be less than 500 due to capping
                assert!(pt.high < 500.0, "outlier should be capped, got {}", pt.high);
            }
            other => panic!(
                "Expected StudyCandles, got {:?}",
                std::mem::discriminant(other)
            ),
        }
    }

    #[test]
    fn test_stddev_filter_none() {
        let mut study = SpeedOfTapeStudy::new();
        set_params(
            &mut study,
            &[
                ("input_data", ParameterValue::Choice("Volume".into())),
                ("display_value", ParameterValue::Choice("Total".into())),
                ("bucket_seconds", ParameterValue::Integer(10)),
                ("filter_mode", ParameterValue::Choice("None".into())),
            ],
        );

        let candles = vec![make_candle(0, 100.0, 101.0, 99.0, 100.5, 50.0, 50.0)];
        let trades = vec![
            make_trade(1000, Side::Buy, 500.0), // bucket 0
            make_trade(15000, Side::Buy, 10.0), // bucket 1
        ];

        let input = make_input(&candles, &trades);
        study.compute(&input).unwrap();

        match study.output() {
            StudyOutput::StudyCandles(series) => {
                let pt = &series[0].points[0];
                // No capping — high should be the raw 500
                assert_eq!(pt.open, 500.0);
                assert_eq!(pt.high, 500.0);
            }
            other => panic!(
                "Expected StudyCandles, got {:?}",
                std::mem::discriminant(other)
            ),
        }
    }

    #[test]
    fn test_bucket_seconds() {
        let mut study = SpeedOfTapeStudy::new();
        set_params(
            &mut study,
            &[
                ("input_data", ParameterValue::Choice("Trades".into())),
                ("display_value", ParameterValue::Choice("Total".into())),
                ("bucket_seconds", ParameterValue::Integer(20)),
                ("filter_mode", ParameterValue::Choice("None".into())),
            ],
        );

        let candles = vec![make_candle(0, 100.0, 101.0, 99.0, 100.5, 50.0, 50.0)];
        // 60s / 20s = 3 buckets
        // Bucket 0 (0–20s): 1 trade
        // Bucket 1 (20–40s): 2 trades
        // Bucket 2 (40–60s): 0 trades
        let trades = vec![
            make_trade(5000, Side::Buy, 1.0),
            make_trade(25000, Side::Buy, 1.0),
            make_trade(35000, Side::Sell, 1.0),
        ];

        let input = make_input(&candles, &trades);
        study.compute(&input).unwrap();

        match study.output() {
            StudyOutput::StudyCandles(series) => {
                let pt = &series[0].points[0];
                // open = first non-zero = bucket 0 = 1
                // high = max non-zero = bucket 1 = 2
                // low = min non-zero = bucket 0 = 1
                // close = last non-zero = bucket 1 = 2
                // (bucket 2 is empty, skipped)
                assert_eq!(pt.open, 1.0);
                assert_eq!(pt.high, 2.0);
                assert_eq!(pt.low, 1.0);
                assert_eq!(pt.close, 2.0);
            }
            other => panic!(
                "Expected StudyCandles, got {:?}",
                std::mem::discriminant(other)
            ),
        }
    }

    #[test]
    fn test_empty_trades() {
        let mut study = SpeedOfTapeStudy::new();
        let candles = vec![make_candle(0, 100.0, 101.0, 99.0, 100.5, 50.0, 50.0)];
        let trades: Vec<Trade> = vec![];

        let input = make_input(&candles, &trades);
        study.compute(&input).unwrap();
        assert!(matches!(study.output(), StudyOutput::Empty));
    }

    #[test]
    fn test_no_trades_input() {
        let mut study = SpeedOfTapeStudy::new();
        let candles = vec![make_candle(0, 100.0, 101.0, 99.0, 100.5, 50.0, 50.0)];

        let input = StudyInput {
            candles: &candles,
            trades: None,
            basis: ChartBasis::Time(Timeframe::M1),
            tick_size: Price::from_f32(0.25),
            visible_range: None,
        };

        study.compute(&input).unwrap();
        assert!(matches!(study.output(), StudyOutput::Empty));
    }

    #[test]
    fn test_buy_sell_color() {
        let mut study = SpeedOfTapeStudy::new();
        set_params(
            &mut study,
            &[("filter_mode", ParameterValue::Choice("None".into()))],
        );

        let buy_color = study.config.get_color("buy_color", DEFAULT_BUY_COLOR);
        let sell_color = study.config.get_color("sell_color", DEFAULT_SELL_COLOR);

        let candles = vec![
            make_candle(0, 100.0, 101.0, 99.0, 100.5, 50.0, 50.0),
            make_candle(60_000, 100.0, 101.0, 99.0, 100.5, 50.0, 50.0),
        ];

        let trades = vec![
            // Candle 0: 3 buys, 1 sell → buy color
            make_trade(1000, Side::Buy, 1.0),
            make_trade(2000, Side::Buy, 1.0),
            make_trade(3000, Side::Buy, 1.0),
            make_trade(4000, Side::Sell, 1.0),
            // Candle 1: 1 buy, 3 sells → sell color
            make_trade(61_000, Side::Buy, 1.0),
            make_trade(62_000, Side::Sell, 1.0),
            make_trade(63_000, Side::Sell, 1.0),
            make_trade(64_000, Side::Sell, 1.0),
        ];

        let input = make_input(&candles, &trades);
        study.compute(&input).unwrap();

        match study.output() {
            StudyOutput::StudyCandles(series) => {
                assert_eq!(series[0].points.len(), 2);
                let p0 = &series[0].points[0];
                let p1 = &series[0].points[1];
                assert_eq!(p0.border_color.r, buy_color.r);
                assert_eq!(p0.border_color.g, buy_color.g);
                assert_eq!(p0.border_color.b, buy_color.b);
                assert_eq!(p1.border_color.r, sell_color.r);
                assert_eq!(p1.border_color.g, sell_color.g);
                assert_eq!(p1.border_color.b, sell_color.b);
            }
            other => panic!(
                "Expected StudyCandles, got {:?}",
                std::mem::discriminant(other)
            ),
        }
    }

    #[test]
    fn test_tick_basis() {
        let mut study = SpeedOfTapeStudy::new();
        set_params(
            &mut study,
            &[("filter_mode", ParameterValue::Choice("None".into()))],
        );

        let candles = vec![
            make_candle(0, 100.0, 101.0, 99.0, 100.5, 50.0, 50.0),
            make_candle(5000, 100.0, 101.0, 99.0, 100.5, 50.0, 50.0),
        ];

        let trades = vec![
            make_trade(1000, Side::Buy, 1.0),
            make_trade(6000, Side::Buy, 1.0),
        ];

        let input = StudyInput {
            candles: &candles,
            trades: Some(&trades),
            basis: ChartBasis::Tick(100),
            tick_size: Price::from_f32(0.25),
            visible_range: None,
        };

        study.compute(&input).unwrap();

        match study.output() {
            StudyOutput::StudyCandles(series) => {
                assert_eq!(series[0].points.len(), 2);
                // Tick basis: reverse indices
                assert_eq!(series[0].points[0].x, 1);
                assert_eq!(series[0].points[1].x, 0);
            }
            other => panic!(
                "Expected StudyCandles, got {:?}",
                std::mem::discriminant(other)
            ),
        }
    }

    #[test]
    fn test_no_trades_in_range() {
        let mut study = SpeedOfTapeStudy::new();
        set_params(
            &mut study,
            &[("filter_mode", ParameterValue::Choice("None".into()))],
        );

        let candles = vec![make_candle(0, 100.0, 101.0, 99.0, 100.5, 50.0, 50.0)];
        let trades = vec![make_trade(120_000, Side::Buy, 1.0)];

        let input = make_input(&candles, &trades);
        study.compute(&input).unwrap();

        match study.output() {
            StudyOutput::StudyCandles(series) => {
                let pt = &series[0].points[0];
                assert_eq!(pt.open, 0.0);
                assert_eq!(pt.high, 0.0);
                assert_eq!(pt.low, 0.0);
                assert_eq!(pt.close, 0.0);
            }
            other => panic!(
                "Expected StudyCandles, got {:?}",
                std::mem::discriminant(other)
            ),
        }
    }

    #[test]
    fn test_reset() {
        let mut study = SpeedOfTapeStudy::new();
        set_params(
            &mut study,
            &[("filter_mode", ParameterValue::Choice("None".into()))],
        );

        let candles = vec![make_candle(0, 100.0, 101.0, 99.0, 100.5, 50.0, 50.0)];
        let trades = vec![make_trade(1000, Side::Buy, 1.0)];

        let input = make_input(&candles, &trades);
        study.compute(&input).unwrap();
        assert!(!matches!(study.output(), StudyOutput::Empty));

        study.reset();
        assert!(matches!(study.output(), StudyOutput::Empty));
    }

    #[test]
    fn test_clone() {
        let study = SpeedOfTapeStudy::new();
        let cloned = study.clone_study();
        assert_eq!(cloned.id(), "speed_of_tape");
        assert_eq!(cloned.name(), "Speed of Tape");
    }
}
