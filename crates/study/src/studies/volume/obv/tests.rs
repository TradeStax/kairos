use super::*;
use crate::util::test_helpers::{make_candle_ohlcv, make_input};
use data::Candle;

fn make_candle(time: u64, close: f32, buy_vol: f64, sell_vol: f64) -> Candle {
    make_candle_ohlcv(
        time,
        close,
        close + 1.0,
        close - 1.0,
        close,
        buy_vol,
        sell_vol,
    )
}

#[test]
fn test_obv_empty() {
    let mut study = ObvStudy::new();
    let input = make_input(&[]);
    study.compute(&input).unwrap();
    assert!(matches!(study.output(), StudyOutput::Empty));
}

#[test]
fn test_obv_single_candle() {
    let mut study = ObvStudy::new();
    let candles = vec![make_candle(1000, 100.0, 50.0, 50.0)];
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
    assert!((lines[0].points[0].1).abs() < 0.01);
}

#[test]
fn test_obv_calculation() {
    let mut study = ObvStudy::new();
    let candles = vec![
        make_candle(1000, 100.0, 50.0, 50.0), // OBV = 0
        make_candle(2000, 105.0, 60.0, 40.0), // close up, vol=100 => OBV = +100
        make_candle(3000, 102.0, 30.0, 50.0), // close down, vol=80 => OBV = +20
        make_candle(4000, 102.0, 45.0, 45.0), // close equal, vol=90 => OBV = +20
        make_candle(5000, 110.0, 70.0, 30.0), // close up, vol=100 => OBV = +120
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
    let pts = &lines[0].points;
    assert_eq!(pts.len(), 5);
    assert!((pts[0].1 - 0.0).abs() < 0.01);
    assert!((pts[1].1 - 100.0).abs() < 0.01);
    assert!((pts[2].1 - 20.0).abs() < 0.01);
    assert!((pts[3].1 - 20.0).abs() < 0.01); // unchanged
    assert!((pts[4].1 - 120.0).abs() < 0.01);
}

#[test]
fn test_obv_downtrend() {
    let mut study = ObvStudy::new();
    let candles = vec![
        make_candle(1000, 100.0, 50.0, 50.0),
        make_candle(2000, 95.0, 40.0, 60.0), // down, vol=100 => -100
        make_candle(3000, 90.0, 30.0, 70.0), // down, vol=100 => -200
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
    let pts = &lines[0].points;
    assert!((pts[0].1).abs() < 0.01);
    assert!((pts[1].1 - (-100.0)).abs() < 0.01);
    assert!((pts[2].1 - (-200.0)).abs() < 0.01);
}

#[test]
fn obv_zero_volume_candles_unchanged() {
    let mut study = ObvStudy::new();
    let candles = vec![
        make_candle(1000, 100.0, 50.0, 50.0),
        // Close up but zero volume -> OBV should still add 0
        make_candle_ohlcv(2000, 105.0, 106.0, 104.0, 105.0, 0.0, 0.0),
    ];
    let input = make_input(&candles);
    study.compute(&input).unwrap();

    let StudyOutput::Lines(lines) = study.output() else {
        panic!("expected Lines output")
    };
    let pts = &lines[0].points;
    assert!((pts[0].1).abs() < 0.01); // first candle OBV = 0
    assert!((pts[1].1).abs() < 0.01); // close up, but vol=0 => 0+0=0
}

#[test]
fn obv_all_equal_closes() {
    let mut study = ObvStudy::new();
    let candles = vec![
        make_candle(1000, 100.0, 50.0, 50.0),
        make_candle(2000, 100.0, 60.0, 40.0), // equal close, vol=100 => unchanged
        make_candle(3000, 100.0, 70.0, 30.0), // equal close, vol=100 => unchanged
    ];
    let input = make_input(&candles);
    study.compute(&input).unwrap();

    let StudyOutput::Lines(lines) = study.output() else {
        panic!("expected Lines output")
    };
    let pts = &lines[0].points;
    // All closes equal => OBV stays at 0 throughout
    assert!((pts[0].1).abs() < 0.01);
    assert!((pts[1].1).abs() < 0.01);
    assert!((pts[2].1).abs() < 0.01);
}

#[test]
fn obv_reset_clears_output() {
    let mut study = ObvStudy::new();
    let candles = vec![make_candle(1000, 100.0, 50.0, 50.0)];
    let input = make_input(&candles);
    study.compute(&input).unwrap();
    assert!(!matches!(study.output(), StudyOutput::Empty));

    study.reset();
    assert!(matches!(study.output(), StudyOutput::Empty));
}

#[test]
fn obv_alternating_direction() {
    let mut study = ObvStudy::new();
    let candles = vec![
        make_candle(1000, 100.0, 50.0, 50.0), // OBV = 0
        make_candle(2000, 110.0, 40.0, 60.0), // up, vol=100 => +100
        make_candle(3000, 105.0, 30.0, 70.0), // down, vol=100 => 0
        make_candle(4000, 115.0, 80.0, 20.0), // up, vol=100 => +100
        make_candle(5000, 108.0, 25.0, 75.0), // down, vol=100 => 0
    ];
    let input = make_input(&candles);
    study.compute(&input).unwrap();

    let StudyOutput::Lines(lines) = study.output() else {
        panic!("expected Lines output")
    };
    let pts = &lines[0].points;
    assert_eq!(pts.len(), 5);
    assert!((pts[0].1 - 0.0).abs() < 0.01);
    assert!((pts[1].1 - 100.0).abs() < 0.01);
    assert!((pts[2].1 - 0.0).abs() < 0.01);
    assert!((pts[3].1 - 100.0).abs() < 0.01);
    assert!((pts[4].1 - 0.0).abs() < 0.01);
}
