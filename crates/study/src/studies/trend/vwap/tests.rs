use super::*;
use crate::config::ParameterValue;
use data::{Candle, ChartBasis, Price, Timeframe, Timestamp, Volume};

fn make_candle(time: u64, high: f32, low: f32, close: f32, buy_vol: f64, sell_vol: f64) -> Candle {
    let open = (high + low) / 2.0;
    Candle::new(
        Timestamp(time),
        Price::from_f32(open),
        Price::from_f32(high),
        Price::from_f32(low),
        Price::from_f32(close),
        Volume(buy_vol),
        Volume(sell_vol),
    )
    .expect("test: valid candle")
}

fn make_input(candles: &[Candle]) -> crate::core::StudyInput<'_> {
    crate::core::StudyInput {
        candles,
        trades: None,
        basis: ChartBasis::Time(Timeframe::M1),
        tick_size: Price::from_f32(0.25),
        visible_range: None,
    }
}

#[test]
fn test_vwap_empty() {
    let mut study = VwapStudy::new();
    let input = make_input(&[]);
    study.compute(&input).unwrap();
    assert!(matches!(study.output(), StudyOutput::Empty));
}

#[test]
fn test_vwap_single_candle() {
    let mut study = VwapStudy::new();
    // TP = (30 + 10 + 20) / 3 = 20.0
    let candles = vec![make_candle(1000, 30.0, 10.0, 20.0, 50.0, 50.0)];
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
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].points.len(), 1);
    assert!((lines[0].points[0].1 - 20.0).abs() < 0.01);
}

#[test]
fn test_vwap_calculation() {
    let mut study = VwapStudy::new();
    // Candle 1: TP = (12+8+10)/3 = 10, vol = 100
    // Candle 2: TP = (24+16+20)/3 = 20, vol = 200
    // VWAP after candle 1: 10*100/100 = 10.0
    // VWAP after candle 2: (10*100 + 20*200) / (100+200) = 5000/300 = 16.667
    let candles = vec![
        make_candle(1000, 12.0, 8.0, 10.0, 60.0, 40.0),
        make_candle(2000, 24.0, 16.0, 20.0, 120.0, 80.0),
    ];
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
    assert_eq!(lines.len(), 1);
    let pts = &lines[0].points;
    assert_eq!(pts.len(), 2);
    assert!((pts[0].1 - 10.0).abs() < 0.01);
    assert!((pts[1].1 - 16.667).abs() < 0.01);
}

#[test]
fn test_vwap_with_bands() {
    let mut study = VwapStudy::new();
    study
        .set_parameter("show_bands", ParameterValue::Boolean(true))
        .unwrap();

    let candles = vec![
        make_candle(1000, 12.0, 8.0, 10.0, 60.0, 40.0),
        make_candle(2000, 24.0, 16.0, 20.0, 120.0, 80.0),
        make_candle(3000, 18.0, 12.0, 15.0, 80.0, 70.0),
    ];
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
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0].label, "VWAP");
    assert_eq!(lines[1].label, "VWAP Upper");
    assert_eq!(lines[2].label, "VWAP Lower");
    // Upper should be above VWAP, lower below
    for i in 0..lines[0].points.len() {
        assert!(lines[1].points[i].1 >= lines[0].points[i].1);
        assert!(lines[2].points[i].1 <= lines[0].points[i].1);
    }
}

#[test]
fn test_vwap_zero_volume() {
    let mut study = VwapStudy::new();
    // Zero volume candle should use typical price as VWAP
    let candles = vec![make_candle(1000, 30.0, 10.0, 20.0, 0.0, 0.0)];
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
    assert_eq!(lines[0].points.len(), 1);
    // TP = (30+10+20)/3 = 20.0
    assert!((lines[0].points[0].1 - 20.0).abs() < 0.01);
}
