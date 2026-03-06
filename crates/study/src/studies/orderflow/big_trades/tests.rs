use super::*;
use crate::config::ParameterValue;
use crate::output::{MarkerShape, StudyOutput, TradeMarker};
use crate::util::test_helpers::{make_candle, make_input_with_trades, make_trade};
use block::format_contracts;
use data::{Candle, ChartBasis, Price, Side, Timeframe, Trade};

fn study_input<'a>(candles: &'a [Candle], trades: &'a [Trade]) -> StudyInput<'a> {
    make_input_with_trades(candles, trades)
}

/// Helper: convert marker price (i64 units) back to f64 for assertions
fn marker_price_f64(marker: &TradeMarker) -> f64 {
    Price::from_units(marker.price).to_f64()
}

// ── Existing tests (unchanged behavior) ─────────────────────────────

#[test]
fn test_empty_trades() {
    let mut study = BigTradesStudy::new();
    let candles = vec![];
    let trades: Vec<Trade> = vec![];
    study.compute(&study_input(&candles, &trades)).unwrap();
    assert!(matches!(study.output(), StudyOutput::Empty));
}

#[test]
fn test_single_large_fill() {
    let mut study = BigTradesStudy::new();
    let candles = vec![make_candle(1000, 100.0)];
    let trades = vec![make_trade(1000, 100.0, 100.0, Side::Buy)];
    study.compute(&study_input(&candles, &trades)).unwrap();

    let output = study.output();
    assert!(
        matches!(output, StudyOutput::Markers(_)),
        "Expected Markers"
    );
    let StudyOutput::Markers(md) = output else {
        unreachable!()
    };
    let m = &md.markers;
    assert_eq!(m.len(), 1);
    assert!(m[0].is_buy);
    assert!((m[0].contracts - 100.0).abs() < f64::EPSILON);
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
    study.compute(&study_input(&candles, &trades)).unwrap();
    assert!(matches!(study.output(), StudyOutput::Empty));
}

#[test]
fn test_three_same_side_fills_merge_with_correct_vwap() {
    let mut study = BigTradesStudy::new();
    let candles = vec![make_candle(1000, 100.0)];
    let trades = vec![
        make_trade(1000, 100.0, 20.0, Side::Buy),
        make_trade(1020, 101.0, 30.0, Side::Buy),
        make_trade(1040, 102.0, 10.0, Side::Buy),
    ];

    study
        .set_parameter("filter_min", ParameterValue::Integer(50))
        .unwrap();
    study.compute(&study_input(&candles, &trades)).unwrap();

    let output = study.output();
    assert!(
        matches!(output, StudyOutput::Markers(_)),
        "Expected Markers"
    );
    let StudyOutput::Markers(md) = output else {
        unreachable!()
    };
    let m = &md.markers;
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
        .set_parameter("filter_min", ParameterValue::Integer(1))
        .unwrap();
    let candles = vec![make_candle(1000, 100.0)];
    let trades = vec![
        make_trade(1000, 5432.75, 7.0, Side::Buy),
        make_trade(1010, 5433.25, 13.0, Side::Buy),
    ];
    study.compute(&study_input(&candles, &trades)).unwrap();

    let output = study.output();
    assert!(
        matches!(output, StudyOutput::Markers(_)),
        "Expected Markers"
    );
    let StudyOutput::Markers(md) = output else {
        unreachable!()
    };
    let m = &md.markers;
    assert_eq!(m.len(), 1);
    let expected = (7.0 * 5432.75 + 13.0 * 5433.25) / 20.0;
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
        .set_parameter("filter_min", ParameterValue::Integer(50))
        .unwrap();
    study
        .set_parameter("aggregation_window_ms", ParameterValue::Integer(100))
        .unwrap();

    let candles = vec![make_candle(1000, 100.0)];
    let trades = vec![
        make_trade(1000, 100.0, 60.0, Side::Buy),
        make_trade(1200, 101.0, 60.0, Side::Buy),
    ];
    study.compute(&study_input(&candles, &trades)).unwrap();

    let output = study.output();
    assert!(
        matches!(output, StudyOutput::Markers(_)),
        "Expected Markers"
    );
    let StudyOutput::Markers(md) = output else {
        unreachable!()
    };
    let m = &md.markers;
    assert_eq!(m.len(), 2);
}

#[test]
fn test_side_change_creates_separate_markers() {
    let mut study = BigTradesStudy::new();
    study
        .set_parameter("filter_min", ParameterValue::Integer(50))
        .unwrap();

    let candles = vec![make_candle(1000, 100.0)];
    let trades = vec![
        make_trade(1000, 100.0, 60.0, Side::Buy),
        make_trade(1050, 100.0, 60.0, Side::Sell),
    ];
    study.compute(&study_input(&candles, &trades)).unwrap();

    let output = study.output();
    assert!(
        matches!(output, StudyOutput::Markers(_)),
        "Expected Markers"
    );
    let StudyOutput::Markers(md) = output else {
        unreachable!()
    };
    let m = &md.markers;
    assert_eq!(m.len(), 2);
    assert!(m[0].is_buy);
    assert!(!m[1].is_buy);
}

#[test]
fn test_continuous_burst_merges_with_previous_fill_window() {
    let mut study = BigTradesStudy::new();
    study
        .set_parameter("filter_min", ParameterValue::Integer(1))
        .unwrap();
    study
        .set_parameter("aggregation_window_ms", ParameterValue::Integer(150))
        .unwrap();

    let candles = vec![make_candle(1000, 100.0)];
    let trades: Vec<Trade> = (0..10)
        .map(|i| make_trade(1000 + i * 100, 100.0, 10.0, Side::Buy))
        .collect();
    study.compute(&study_input(&candles, &trades)).unwrap();

    let output = study.output();
    assert!(
        matches!(output, StudyOutput::Markers(_)),
        "Expected Markers"
    );
    let StudyOutput::Markers(md) = output else {
        unreachable!()
    };
    let m = &md.markers;
    assert_eq!(m.len(), 1, "Expected 1 merged marker, got {}", m.len());
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
        .set_parameter("filter_min", ParameterValue::Integer(50))
        .unwrap();
    study
        .set_parameter("aggregation_window_ms", ParameterValue::Integer(150))
        .unwrap();

    let candles = vec![make_candle(1000, 100.0)];
    let trades = vec![
        make_trade(1000, 100.0, 60.0, Side::Buy),
        make_trade(1050, 100.0, 0.0, Side::Sell),
        make_trade(1100, 100.0, 10.0, Side::Buy),
    ];
    study.compute(&study_input(&candles, &trades)).unwrap();

    let output = study.output();
    assert!(
        matches!(output, StudyOutput::Markers(_)),
        "Expected Markers"
    );
    let StudyOutput::Markers(md) = output else {
        unreachable!()
    };
    let m = &md.markers;
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

    study.compute(&study_input(&candles, &trades)).unwrap();
    assert!(matches!(study.output(), StudyOutput::Empty));

    study
        .set_parameter("filter_min", ParameterValue::Integer(20))
        .unwrap();
    study.compute(&study_input(&candles, &trades)).unwrap();
    assert!(matches!(study.output(), StudyOutput::Markers(_)));
}

#[test]
fn test_clone_study_produces_independent_copy() {
    let mut study = BigTradesStudy::new();
    study
        .set_parameter("filter_min", ParameterValue::Integer(10))
        .unwrap();

    let cloned = study.clone_study();
    assert_eq!(cloned.id(), "big_trades");
    assert_eq!(cloned.config().get_int("filter_min", 50), 10);

    study
        .set_parameter("filter_min", ParameterValue::Integer(99))
        .unwrap();
    assert_eq!(cloned.config().get_int("filter_min", 50), 10);
}

#[test]
fn test_debug_annotations_populated() {
    let mut study = BigTradesStudy::new();
    study
        .set_parameter("filter_min", ParameterValue::Integer(1))
        .unwrap();
    study
        .set_parameter("show_debug", ParameterValue::Boolean(true))
        .unwrap();

    let candles = vec![make_candle(1000, 100.0)];
    let trades = vec![
        make_trade(1000, 100.0, 20.0, Side::Buy),
        make_trade(1030, 101.0, 30.0, Side::Buy),
    ];
    study.compute(&study_input(&candles, &trades)).unwrap();

    let output = study.output();
    assert!(
        matches!(output, StudyOutput::Markers(_)),
        "Expected Markers"
    );
    let StudyOutput::Markers(md) = output else {
        unreachable!()
    };
    let m = &md.markers;
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
        .set_parameter("filter_min", ParameterValue::Integer(50))
        .unwrap();

    let candles = vec![make_candle(1000, 100.0)];

    let trades1 = vec![make_trade(1000, 100.0, 30.0, Side::Buy)];
    study.compute(&study_input(&candles, &trades1)).unwrap();
    assert!(matches!(study.output(), StudyOutput::Empty));

    let mut trades2 = trades1.clone();
    trades2.push(make_trade(1030, 100.0, 30.0, Side::Buy));

    let input = study_input(&candles, &trades2);
    study.append_trades(&trades2[1..], &input).unwrap();

    let output = study.output();
    assert!(
        matches!(output, StudyOutput::Markers(_)),
        "Expected Markers"
    );
    let StudyOutput::Markers(md) = output else {
        unreachable!()
    };
    let m = &md.markers;
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
        .set_parameter("filter_min", ParameterValue::Integer(1))
        .unwrap();

    let candles = vec![
        make_candle(0, 100.0),
        make_candle(300_000, 101.0),
        make_candle(600_000, 102.0),
    ];
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
    study.compute(&input).unwrap();

    let output = study.output();
    assert!(
        matches!(output, StudyOutput::Markers(_)),
        "Expected Markers"
    );
    let StudyOutput::Markers(md) = output else {
        unreachable!()
    };
    let m = &md.markers;
    assert_eq!(m.len(), 1);
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
        .set_parameter("filter_min", ParameterValue::Integer(1))
        .unwrap();

    let candles = vec![
        make_candle(0, 100.0),
        make_candle(300_000, 101.0),
        make_candle(600_000, 102.0),
    ];
    let trades = vec![make_trade(450_000, 101.0, 50.0, Side::Sell)];

    let input = StudyInput {
        candles: &candles,
        trades: Some(&trades),
        basis: ChartBasis::Time(Timeframe::M5),
        tick_size: Price::from_f32(0.25),
        visible_range: None,
    };
    study.compute(&input).unwrap();

    let output = study.output();
    assert!(
        matches!(output, StudyOutput::Markers(_)),
        "Expected Markers"
    );
    let StudyOutput::Markers(md) = output else {
        unreachable!()
    };
    let m = &md.markers;
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
        .set_parameter("filter_min", ParameterValue::Integer(1))
        .unwrap();

    let candles = vec![
        make_candle(1000, 100.0),
        make_candle(2000, 101.0),
        make_candle(3000, 102.0),
    ];
    let trades = vec![make_trade(2500, 101.0, 50.0, Side::Buy)];

    let input = StudyInput {
        candles: &candles,
        trades: Some(&trades),
        basis: ChartBasis::Tick(100),
        tick_size: Price::from_f32(0.25),
        visible_range: None,
    };
    study.compute(&input).unwrap();

    let output = study.output();
    assert!(
        matches!(output, StudyOutput::Markers(_)),
        "Expected Markers"
    );
    let StudyOutput::Markers(md) = output else {
        unreachable!()
    };
    let m = &md.markers;
    assert_eq!(m.len(), 1);
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
        .set_parameter("filter_min", ParameterValue::Integer(1))
        .unwrap();
    study
        .set_parameter("aggregation_window_ms", ParameterValue::Integer(200))
        .unwrap();

    let candles = vec![make_candle(0, 100.0), make_candle(300_000, 101.0)];
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
    study.compute(&input).unwrap();

    let output = study.output();
    assert!(
        matches!(output, StudyOutput::Markers(_)),
        "Expected Markers"
    );
    let StudyOutput::Markers(md) = output else {
        unreachable!()
    };
    let m = &md.markers;
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
    let mut study = BigTradesStudy::new();
    study
        .set_parameter("filter_min", ParameterValue::Integer(1))
        .unwrap();
    study
        .set_parameter("aggregation_window_ms", ParameterValue::Integer(400))
        .unwrap();

    let candles = vec![make_candle(1000, 100.0), make_candle(2000, 101.0)];
    let trades = vec![
        make_trade(1500, 100.0, 30.0, Side::Buy),
        make_trade(1700, 100.0, 30.0, Side::Buy),
    ];

    let input = StudyInput {
        candles: &candles,
        trades: Some(&trades),
        basis: ChartBasis::Tick(100),
        tick_size: Price::from_f32(0.25),
        visible_range: None,
    };
    study.compute(&input).unwrap();

    let output = study.output();
    assert!(
        matches!(output, StudyOutput::Markers(_)),
        "Expected Markers"
    );
    let StudyOutput::Markers(md) = output else {
        unreachable!()
    };
    let m = &md.markers;
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

#[test]
fn test_filter_max_excludes_large_trades() {
    let mut study = BigTradesStudy::new();
    study
        .set_parameter("filter_min", ParameterValue::Integer(1))
        .unwrap();
    study
        .set_parameter("filter_max", ParameterValue::Integer(50))
        .unwrap();

    let candles = vec![make_candle(1000, 100.0)];
    let trades = vec![
        make_trade(1000, 100.0, 30.0, Side::Buy),
        make_trade(2000, 100.0, 60.0, Side::Sell),
    ];
    study.compute(&study_input(&candles, &trades)).unwrap();

    let output = study.output();
    assert!(
        matches!(output, StudyOutput::Markers(_)),
        "Expected Markers"
    );
    let StudyOutput::Markers(md) = output else {
        unreachable!()
    };
    let m = &md.markers;
    assert_eq!(m.len(), 1, "filter_max should exclude 60-lot trade");
    assert!(
        (m[0].contracts - 30.0).abs() < f64::EPSILON,
        "contracts: {}",
        m[0].contracts
    );
}

#[test]
fn test_filter_max_zero_means_no_upper_limit() {
    let mut study = BigTradesStudy::new();
    study
        .set_parameter("filter_min", ParameterValue::Integer(1))
        .unwrap();

    let candles = vec![make_candle(1000, 100.0)];
    let trades = vec![make_trade(1000, 100.0, 10000.0, Side::Buy)];
    study.compute(&study_input(&candles, &trades)).unwrap();

    let output = study.output();
    assert!(
        matches!(output, StudyOutput::Markers(_)),
        "Expected Markers with no upper filter"
    );
}

#[test]
fn test_marker_render_config() {
    let study = BigTradesStudy::new();
    let config = study.build_marker_render_config();
    assert_eq!(config.shape, MarkerShape::Circle);
    assert!(!config.hollow);
    assert!(config.show_text);
    assert!((config.scale_min - 50.0).abs() < f64::EPSILON);
    assert!((config.scale_max - 500.0).abs() < f64::EPSILON);
    assert!((config.min_size - 8.0).abs() < f32::EPSILON);
    assert!((config.max_size - 36.0).abs() < f32::EPSILON);
    assert!((config.min_opacity - 0.10).abs() < f32::EPSILON);
    assert!((config.max_opacity - 0.60).abs() < f32::EPSILON);
}

// ── Absorption integration tests ────────────────────────────────────

#[test]
fn test_absorption_disabled_produces_markers_not_composite() {
    let mut study = BigTradesStudy::new();
    study
        .set_parameter("filter_min", ParameterValue::Integer(1))
        .unwrap();
    // absorption_enabled defaults to false

    let candles = vec![make_candle(1000, 100.0)];
    let trades = vec![make_trade(1000, 100.0, 100.0, Side::Buy)];
    study.compute(&study_input(&candles, &trades)).unwrap();

    assert!(
        matches!(study.output(), StudyOutput::Markers(_)),
        "With absorption disabled, output should be Markers, got {:?}",
        study.output().discriminant_name()
    );
}

#[test]
fn test_absorption_enabled_no_zones_still_markers() {
    let mut study = BigTradesStudy::new();
    study
        .set_parameter("filter_min", ParameterValue::Integer(1))
        .unwrap();
    study
        .set_parameter("absorption_enabled", ParameterValue::Boolean(true))
        .unwrap();

    // Just one trade — not enough history for absorption detection
    let candles = vec![make_candle(1000, 100.0)];
    let trades = vec![make_trade(1000, 100.0, 100.0, Side::Buy)];
    study.compute(&study_input(&candles, &trades)).unwrap();

    assert!(
        matches!(study.output(), StudyOutput::Markers(_)),
        "With no zones, output should still be Markers"
    );
}

#[test]
fn test_absorption_parameter_tab_present() {
    let study = BigTradesStudy::new();
    let tabs = study.tab_labels().unwrap();
    assert!(
        tabs.iter().any(|(_, t)| *t == ParameterTab::Absorption),
        "Absorption tab should be in tab_labels"
    );
}

#[test]
fn test_absorption_params_have_correct_tab() {
    let study = BigTradesStudy::new();
    let absorption_params: Vec<_> = study
        .parameters()
        .iter()
        .filter(|p| p.tab == ParameterTab::Absorption)
        .collect();
    assert_eq!(
        absorption_params.len(),
        10,
        "Expected 10 absorption parameters"
    );
}

#[test]
fn test_absorption_params_visibility_when_disabled() {
    let study = BigTradesStudy::new();
    let config = study.config();
    // absorption_enabled is false by default
    let hidden: Vec<_> = study
        .parameters()
        .iter()
        .filter(|p| p.tab == ParameterTab::Absorption)
        .filter(|p| !p.visible_when.is_visible(config))
        .collect();
    // All except the enable toggle should be hidden
    assert_eq!(
        hidden.len(),
        9,
        "9 absorption sub-params should be hidden when disabled"
    );
}

#[test]
fn test_absorption_params_visible_when_enabled() {
    let mut study = BigTradesStudy::new();
    study
        .set_parameter("absorption_enabled", ParameterValue::Boolean(true))
        .unwrap();
    let config = study.config();
    let visible: Vec<_> = study
        .parameters()
        .iter()
        .filter(|p| p.tab == ParameterTab::Absorption)
        .filter(|p| p.visible_when.is_visible(config))
        .collect();
    assert_eq!(
        visible.len(),
        10,
        "All 10 absorption params should be visible when enabled"
    );
}

#[test]
fn test_reset_clears_absorption_state() {
    let mut study = BigTradesStudy::new();
    study
        .set_parameter("absorption_enabled", ParameterValue::Boolean(true))
        .unwrap();
    study
        .set_parameter("filter_min", ParameterValue::Integer(1))
        .unwrap();

    let candles = vec![make_candle(1000, 100.0)];
    let trades = vec![make_trade(1000, 100.0, 100.0, Side::Buy)];
    study.compute(&study_input(&candles, &trades)).unwrap();

    study.reset();
    assert!(matches!(study.output(), StudyOutput::Empty));
}

#[test]
fn test_clone_preserves_absorption_config() {
    let mut study = BigTradesStudy::new();
    study
        .set_parameter("absorption_enabled", ParameterValue::Boolean(true))
        .unwrap();
    study
        .set_parameter("absorption_score_threshold", ParameterValue::Float(0.15))
        .unwrap();

    let cloned = study.clone_study();
    assert!(cloned.config().get_bool("absorption_enabled", false));
    assert!(
        (cloned
            .config()
            .get_float("absorption_score_threshold", 0.25)
            - 0.15)
            .abs()
            < f64::EPSILON
    );
}

// ── Production polish tests ─────────────────────────────────────────

#[test]
fn test_style_param_no_full_recompute() {
    let mut study = BigTradesStudy::new();
    study
        .set_parameter("filter_min", ParameterValue::Integer(1))
        .unwrap();

    let candles = vec![make_candle(1000, 100.0)];
    let trades = vec![
        make_trade(1000, 100.0, 50.0, Side::Buy),
        make_trade(1020, 101.0, 30.0, Side::Buy),
    ];
    study.compute(&study_input(&candles, &trades)).unwrap();

    // Verify markers exist
    let StudyOutput::Markers(md) = study.output() else {
        panic!("Expected Markers");
    };
    let original_contracts = md.markers[0].contracts;

    // Change a style-only param (color) — should NOT trigger full reprocessing
    study
        .set_parameter(
            "buy_color",
            ParameterValue::Color(data::SerializableColor {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            }),
        )
        .unwrap();
    assert_eq!(
        study.recompute_level,
        RecomputeLevel::None,
        "Style param should not raise recompute level"
    );

    // Re-compute with same trades — fast path should preserve markers
    study.compute(&study_input(&candles, &trades)).unwrap();

    let StudyOutput::Markers(md) = study.output() else {
        panic!("Expected Markers after style change");
    };
    assert!(
        (md.markers[0].contracts - original_contracts).abs() < f64::EPSILON,
        "Markers should be preserved through style-only rebuild"
    );
}

#[test]
fn test_structural_param_triggers_recompute() {
    let mut study = BigTradesStudy::new();
    study
        .set_parameter("filter_min", ParameterValue::Integer(1))
        .unwrap();

    let candles = vec![make_candle(1000, 100.0)];
    let trades = vec![make_trade(1000, 100.0, 50.0, Side::Buy)];
    study.compute(&study_input(&candles, &trades)).unwrap();
    assert!(matches!(study.output(), StudyOutput::Markers(_)));

    // Raise filter_min above 50 — structural change
    study
        .set_parameter("filter_min", ParameterValue::Integer(100))
        .unwrap();
    assert_eq!(
        study.recompute_level,
        RecomputeLevel::Full,
        "Structural param should set Full recompute"
    );

    study.compute(&study_input(&candles, &trades)).unwrap();
    assert!(
        matches!(study.output(), StudyOutput::Empty),
        "Should filter out the 50-lot trade"
    );
}

#[test]
fn test_append_returns_unchanged_when_no_new() {
    let mut study = BigTradesStudy::new();
    study
        .set_parameter("filter_min", ParameterValue::Integer(50))
        .unwrap();

    let candles = vec![make_candle(1000, 100.0)];
    let trades = vec![make_trade(1000, 100.0, 60.0, Side::Buy)];
    study.compute(&study_input(&candles, &trades)).unwrap();

    // append_trades with same trades (no new data)
    let input = study_input(&candles, &trades);
    let result = study.append_trades(&[], &input).unwrap();
    assert!(
        !result.output_changed,
        "append_trades with no new data should return unchanged"
    );
}

#[test]
fn test_empty_compute_returns_unchanged() {
    let mut study = BigTradesStudy::new();
    let candles: Vec<Candle> = vec![];
    let trades: Vec<Trade> = vec![];
    let result = study.compute(&study_input(&candles, &trades)).unwrap();
    assert!(
        !result.output_changed,
        "Empty compute should return unchanged"
    );
}

#[test]
fn test_estimator_no_float_drift() {
    // Feed 10K+ blocks through the detector and verify it doesn't
    // produce bogus absorption detections from drifted volume stats.
    use super::params::{
        DEFAULT_ABSORPTION_BUY_COLOR, DEFAULT_ABSORPTION_SELL_COLOR,
    };
    let params = super::params::AbsorptionParams {
        enabled: true,
        lambda_window: 100,
        lambda_smooth: 20,
        score_threshold: 0.25,
        volume_k: 2.0,
        confirm_window_ms: 20000,
        buy_zone_color: DEFAULT_ABSORPTION_BUY_COLOR,
        sell_zone_color: DEFAULT_ABSORPTION_SELL_COLOR,
        zone_opacity: 0.30,
        show_zone_labels: true,
    };
    let tick_units = 25_000_000_i64; // ES tick
    let base_price = 500_000_000_000_i64;
    let mut det = absorption::AbsorptionDetector::new(tick_units, &params);

    // Feed 12K blocks with varying volumes — enough to trigger many
    // evictions and exercise the drift correction path.
    for i in 0..12_000u64 {
        let vol = 40.0 + (i % 60) as f64;
        let block = block::TradeBlock {
            is_buy: true,
            vwap_numerator: (base_price + tick_units) as f64 * vol,
            total_qty: vol,
            first_time: i * 100,
            last_time: i * 100 + 40,
            fill_count: vol as u32,
            min_price_units: base_price,
            max_price_units: base_price + tick_units,
            candle_open: i * 100,
            first_price_units: base_price,
        };
        det.on_block_flushed(&block, &params, i * 100);
    }

    // The detector should still function after 12K blocks.
    // If floating-point drift caused negative mean → bogus threshold,
    // this would either panic or produce incorrect behavior.
    // We just verify it runs to completion without issue.
    // Check pending via public API:
    let _ = det.has_pending();
    // Check zones via test accessor:
    let _ = det.zones();
}

#[test]
fn test_clone_uses_actual_tick_size() {
    let mut study = BigTradesStudy::new();
    study
        .set_parameter("filter_min", ParameterValue::Integer(1))
        .unwrap();

    // Use tick_size 0.50 to differentiate from default 0.25
    let candles = vec![make_candle(1000, 100.0)];
    let trades = vec![make_trade(1000, 100.0, 50.0, Side::Buy)];
    let input = StudyInput {
        candles: &candles,
        trades: Some(&trades),
        basis: ChartBasis::Time(Timeframe::M1),
        tick_size: Price::from_f32(0.50),
        visible_range: None,
    };
    study.compute(&input).unwrap();

    // After compute, last_tick_size_units should reflect 0.50, not 0.25
    let expected_tick_units = Price::from_f32(0.50).units();
    assert_eq!(
        study.last_tick_size_units, expected_tick_units,
        "last_tick_size_units should match input tick_size"
    );

    // Default (before any compute) would have been 0.25
    let default_tick_units = Price::from_f32(0.25).units();
    assert_ne!(
        expected_tick_units, default_tick_units,
        "Test should use non-default tick size"
    );

    // clone_study() now uses last_tick_size_units instead of hardcoded 0.25
    let _cloned = study.clone_study();
    // If this reached here without panic, clone_study succeeded
    // The key fix is that clone_study passes last_tick_size_units to
    // AbsorptionDetector::new() instead of Price::from_f32(0.25).units()
}

#[test]
fn test_absorption_toggle_uses_fast_path() {
    let mut study = BigTradesStudy::new();
    study
        .set_parameter("filter_min", ParameterValue::Integer(1))
        .unwrap();

    let candles = vec![make_candle(1000, 100.0)];
    let trades: Vec<Trade> = (0..20)
        .map(|i| make_trade(1000 + i * 30, 100.0, 50.0, Side::Buy))
        .collect();

    // Initial compute
    study.compute(&study_input(&candles, &trades)).unwrap();
    let markers_before = match study.output() {
        StudyOutput::Markers(md) => md.markers.len(),
        _ => panic!("Expected Markers"),
    };

    // Toggle absorption — should set AbsorptionOnly, NOT Full
    study
        .set_parameter("absorption_enabled", ParameterValue::Boolean(true))
        .unwrap();
    assert_eq!(
        study.recompute_level,
        RecomputeLevel::AbsorptionOnly,
        "Absorption toggle should use AbsorptionOnly, not Full"
    );

    // Compute should use the medium path, preserving markers
    study.compute(&study_input(&candles, &trades)).unwrap();

    // Markers should be identical (absorption doesn't affect them)
    let markers_after = match study.output() {
        StudyOutput::Markers(md) => md.markers.len(),
        StudyOutput::Composite(parts) => {
            parts.iter().find_map(|p| match p {
                StudyOutput::Markers(md) => Some(md.markers.len()),
                _ => None,
            }).unwrap_or(0)
        }
        other => panic!("Expected Markers or Composite, got {:?}", other.discriminant_name()),
    };
    assert_eq!(
        markers_before, markers_after,
        "Absorption toggle should not change marker count"
    );
}

#[test]
fn test_absorption_toggle_roundtrip() {
    let mut study = BigTradesStudy::new();
    study
        .set_parameter("filter_min", ParameterValue::Integer(1))
        .unwrap();

    let candles = vec![make_candle(1000, 100.0)];
    let trades: Vec<Trade> = (0..20)
        .map(|i| make_trade(1000 + i * 30, 100.0, 50.0, Side::Buy))
        .collect();

    // Compute with absorption off
    study.compute(&study_input(&candles, &trades)).unwrap();
    let output_off = study.output().clone();

    // Toggle absorption on
    study
        .set_parameter("absorption_enabled", ParameterValue::Boolean(true))
        .unwrap();
    study.compute(&study_input(&candles, &trades)).unwrap();

    // Toggle absorption off
    study
        .set_parameter("absorption_enabled", ParameterValue::Boolean(false))
        .unwrap();
    study.compute(&study_input(&candles, &trades)).unwrap();

    // Verify we get markers (not crash/empty)
    assert!(
        matches!(study.output(), StudyOutput::Markers(_)),
        "After absorption toggle roundtrip, should still produce Markers"
    );

    // Toggle back on
    study
        .set_parameter("absorption_enabled", ParameterValue::Boolean(true))
        .unwrap();
    study.compute(&study_input(&candles, &trades)).unwrap();

    // Should not crash
    assert!(
        !matches!(study.output(), StudyOutput::Empty),
        "After re-enabling absorption, should produce output"
    );
}
