use super::*;
use crate::output::{
    BackgroundColorMode, FootprintGroupingMode, FootprintRenderMode, OutsideBarStyle, TextFormat,
};
use data::{Candle, ChartBasis, Price, Quantity, Side, Timeframe, Timestamp, Trade, Volume};

fn make_trade(time: u64, price: f32, qty: f64, side: Side) -> Trade {
    Trade {
        time: Timestamp::from_millis(time),
        price: Price::from_f32(price),
        quantity: Quantity(qty),
        side,
    }
}

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
fn test_footprint_compute() {
    let mut study = FootprintStudy::new();
    let candles = vec![make_candle(1000, 100.0, 102.0, 99.0, 101.0, 50.0, 30.0)];
    let trades = vec![
        make_trade(1000, 100.0, 20.0, Side::Buy),
        make_trade(1050, 101.0, 15.0, Side::Buy),
        make_trade(1100, 100.0, 10.0, Side::Sell),
        make_trade(1150, 99.0, 20.0, Side::Sell),
        make_trade(1200, 102.0, 15.0, Side::Buy),
    ];
    let input = StudyInput {
        candles: &candles,
        trades: Some(&trades),
        basis: ChartBasis::Time(Timeframe::M1),
        tick_size: Price::from_f32(1.0),
        visible_range: None,
    };

    study.compute(&input).unwrap();

    match study.output() {
        StudyOutput::Footprint(data) => {
            assert_eq!(data.candles.len(), 1);
            let fp = &data.candles[0];
            assert!(!fp.levels.is_empty());
            assert!(fp.poc_index.is_some());
        }
        _ => panic!("Expected Footprint output"),
    }
}

#[test]
fn test_footprint_empty() {
    let mut study = FootprintStudy::new();
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
fn test_footprint_placement_and_config() {
    let study = FootprintStudy::new();
    assert_eq!(study.metadata().placement, StudyPlacement::CandleReplace);
    assert!(study.candle_render_config().is_some());

    let config = study.candle_render_config().unwrap();
    assert_eq!(config.default_cell_width, 80.0);
    assert_eq!(config.initial_candle_window, 12);
}

#[test]
fn test_footprint_append_trades() {
    let mut study = FootprintStudy::new();
    let candles = vec![make_candle(1000, 100.0, 101.0, 99.0, 100.0, 10.0, 10.0)];
    let trades = vec![
        make_trade(1000, 100.0, 10.0, Side::Buy),
        make_trade(1050, 100.0, 10.0, Side::Sell),
    ];
    let input = StudyInput {
        candles: &candles,
        trades: Some(&trades),
        basis: ChartBasis::Time(Timeframe::M1),
        tick_size: Price::from_f32(1.0),
        visible_range: None,
    };

    study.compute(&input).unwrap();

    // Append one more trade
    let new_trade = make_trade(1100, 101.0, 5.0, Side::Buy);
    study.append_trades(&[new_trade], &input).unwrap();

    match study.output() {
        StudyOutput::Footprint(data) => {
            assert_eq!(data.candles.len(), 1);
            // Should have levels at 100 and 101
            let level_prices: Vec<i64> = data.candles[0].levels.iter().map(|l| l.price).collect();
            assert!(level_prices.len() >= 2);
        }
        _ => panic!("Expected Footprint output"),
    }
}

#[test]
fn test_tick_grouping_manual() {
    let mut study = FootprintStudy::new();
    study
        .set_parameter(
            "auto_grouping",
            ParameterValue::Choice("Manual".to_string()),
        )
        .unwrap();
    study
        .set_parameter("manual_ticks", ParameterValue::Integer(2))
        .unwrap();
    study
        .set_parameter("group_mode", ParameterValue::Choice("Fixed".to_string()))
        .unwrap();

    let candles = vec![make_candle(1000, 100.0, 104.0, 99.0, 103.0, 50.0, 50.0)];
    // Trades at 5 different prices: 99, 100, 101, 102, 103
    let trades = vec![
        make_trade(1000, 99.0, 10.0, Side::Sell),
        make_trade(1010, 100.0, 10.0, Side::Buy),
        make_trade(1020, 101.0, 10.0, Side::Buy),
        make_trade(1030, 102.0, 10.0, Side::Sell),
        make_trade(1040, 103.0, 10.0, Side::Buy),
    ];
    let input = StudyInput {
        candles: &candles,
        trades: Some(&trades),
        basis: ChartBasis::Time(Timeframe::M1),
        tick_size: Price::from_f32(1.0),
        visible_range: None,
    };

    study.compute(&input).unwrap();

    match study.output() {
        StudyOutput::Footprint(data) => {
            assert_eq!(data.candles.len(), 1);
            let fp = &data.candles[0];
            // With manual_ticks=2, prices grouped by 2 tick units
            // so 5 distinct prices should reduce to fewer levels
            assert!(
                fp.levels.len() < 5,
                "Expected fewer levels with grouping, got {}",
                fp.levels.len()
            );
        }
        _ => panic!("Expected Footprint output"),
    }
}

#[test]
fn test_tick_grouping_automatic() {
    let mut study = FootprintStudy::new();
    study
        .set_parameter("auto_group_factor", ParameterValue::Integer(10))
        .unwrap();

    let candles = vec![make_candle(1000, 100.0, 120.0, 99.0, 115.0, 100.0, 100.0)];
    let mut trades = Vec::new();
    for i in 0..20 {
        trades.push(make_trade(
            1000 + i * 10,
            99.0 + i as f32,
            5.0,
            if i % 2 == 0 { Side::Buy } else { Side::Sell },
        ));
    }
    let input = StudyInput {
        candles: &candles,
        trades: Some(&trades),
        basis: ChartBasis::Time(Timeframe::M1),
        tick_size: Price::from_f32(1.0),
        visible_range: None,
    };

    study.compute(&input).unwrap();

    match study.output() {
        StudyOutput::Footprint(data) => {
            assert_eq!(data.candles.len(), 1);
            // Automatic: study computes at 1-tick resolution,
            // renderer will merge dynamically based on y-axis zoom
            assert_eq!(
                data.grouping_mode,
                FootprintGroupingMode::Automatic { factor: 10 }
            );
            let fp = &data.candles[0];
            // At 1-tick resolution, levels span the full range
            assert!(
                fp.levels.len() >= 20,
                "Expected >= 20 levels at 1-tick resolution, \
                 got {}",
                fp.levels.len()
            );
        }
        _ => panic!("Expected Footprint output"),
    }
}

#[test]
fn test_new_parameters_accepted() {
    let mut study = FootprintStudy::new();

    let test_params: Vec<(&str, ParameterValue)> = vec![
        ("data_type", ParameterValue::Choice("Delta".to_string())),
        ("mode", ParameterValue::Choice("Box".to_string())),
        (
            "auto_grouping",
            ParameterValue::Choice("Manual".to_string()),
        ),
        ("auto_group_factor", ParameterValue::Integer(5)),
        ("manual_ticks", ParameterValue::Integer(3)),
        ("group_mode", ParameterValue::Choice("Fixed".to_string())),
        ("bar_marker_width", ParameterValue::Float(0.5)),
        (
            "outside_bar_style",
            ParameterValue::Choice("Candle".to_string()),
        ),
        (
            "marker_alignment",
            ParameterValue::Choice("Center".to_string()),
        ),
        ("show_outside_border", ParameterValue::Boolean(true)),
        ("max_bars_to_show", ParameterValue::Integer(500)),
        ("scaling", ParameterValue::Choice("Linear".to_string())),
        (
            "bg_color_mode",
            ParameterValue::Choice("Delta Intensity".to_string()),
        ),
        ("bg_max_alpha", ParameterValue::Float(0.8)),
        ("bg_buy_color", ParameterValue::Color(crate::BULLISH_COLOR)),
        ("bg_sell_color", ParameterValue::Color(crate::BEARISH_COLOR)),
        ("show_grid_lines", ParameterValue::Boolean(false)),
        ("font_size", ParameterValue::Float(14.0)),
        ("text_format", ParameterValue::Choice("K".to_string())),
        ("dynamic_text_size", ParameterValue::Boolean(false)),
        ("show_zero_values", ParameterValue::Boolean(true)),
    ];

    for (key, value) in test_params {
        assert!(
            study.set_parameter(key, value).is_ok(),
            "Parameter '{key}' should be accepted"
        );
    }

    // Unknown parameter should fail
    assert!(
        study
            .set_parameter("nonexistent", ParameterValue::Integer(1))
            .is_err()
    );
}

#[test]
fn test_build_output_fields() {
    let mut study = FootprintStudy::new();
    study
        .set_parameter("mode", ParameterValue::Choice("Box".to_string()))
        .unwrap();
    study
        .set_parameter("bar_marker_width", ParameterValue::Float(0.5))
        .unwrap();
    study
        .set_parameter(
            "outside_bar_style",
            ParameterValue::Choice("Candle".to_string()),
        )
        .unwrap();
    study
        .set_parameter(
            "bg_color_mode",
            ParameterValue::Choice("Delta Intensity".to_string()),
        )
        .unwrap();
    study
        .set_parameter("text_format", ParameterValue::Choice("K".to_string()))
        .unwrap();
    study
        .set_parameter("show_zero_values", ParameterValue::Boolean(true))
        .unwrap();
    study
        .set_parameter("max_bars_to_show", ParameterValue::Integer(100))
        .unwrap();

    let candles = vec![make_candle(1000, 100.0, 102.0, 99.0, 101.0, 50.0, 30.0)];
    let trades = vec![make_trade(1000, 100.0, 20.0, Side::Buy)];
    let input = StudyInput {
        candles: &candles,
        trades: Some(&trades),
        basis: ChartBasis::Time(Timeframe::M1),
        tick_size: Price::from_f32(1.0),
        visible_range: None,
    };

    study.compute(&input).unwrap();

    match study.output() {
        StudyOutput::Footprint(data) => {
            assert_eq!(data.mode, FootprintRenderMode::Box);
            assert!((data.bar_marker_width - 0.5).abs() < 0.01);
            assert_eq!(data.outside_bar_style, OutsideBarStyle::Candle);
            assert_eq!(data.bg_color_mode, BackgroundColorMode::DeltaIntensity);
            assert_eq!(data.text_format, TextFormat::K);
            assert!(data.show_zero_values);
            assert_eq!(data.max_bars_to_show, 100);
        }
        _ => panic!("Expected Footprint output"),
    }
}

#[test]
fn test_max_bars_does_not_affect_compute() {
    let mut study = FootprintStudy::new();
    study
        .set_parameter("max_bars_to_show", ParameterValue::Integer(10))
        .unwrap();

    let candles = vec![
        make_candle(1000, 100.0, 102.0, 99.0, 101.0, 50.0, 30.0),
        make_candle(61000, 101.0, 103.0, 100.0, 102.0, 40.0, 20.0),
        make_candle(121000, 102.0, 104.0, 101.0, 103.0, 60.0, 10.0),
    ];
    let trades = vec![
        make_trade(1000, 100.0, 20.0, Side::Buy),
        make_trade(61000, 101.0, 15.0, Side::Buy),
        make_trade(121000, 102.0, 10.0, Side::Sell),
    ];
    let input = StudyInput {
        candles: &candles,
        trades: Some(&trades),
        basis: ChartBasis::Time(Timeframe::M1),
        tick_size: Price::from_f32(1.0),
        visible_range: None,
    };

    study.compute(&input).unwrap();

    match study.output() {
        StudyOutput::Footprint(data) => {
            // max_bars is render-side, compute still outputs all
            assert_eq!(data.candles.len(), 3);
            assert_eq!(data.max_bars_to_show, 10);
        }
        _ => panic!("Expected Footprint output"),
    }
}
