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
    assert_eq!(cloned.metadata().name, "Speed of Tape");
}
