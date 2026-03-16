use super::*;
use crate::config::ParameterValue;
use crate::util::test_helpers::{make_candle_ohlcv, make_input};
use data::Candle;

fn make_candle(time: u64, buy_vol: f64, sell_vol: f64) -> Candle {
    make_candle_ohlcv(time, 100.0, 102.0, 99.0, 101.0, buy_vol, sell_vol)
}

#[test]
fn test_cvd_empty() {
    let mut study = CvdStudy::new();
    let input = make_input(&[]);
    study.compute(&input).unwrap();
    assert!(matches!(study.output(), StudyOutput::Empty));
}

#[test]
fn test_cvd_cumulative() {
    let mut study = CvdStudy::new();
    let candles = vec![
        make_candle(1000, 300.0, 200.0), // delta = +100
        make_candle(2000, 100.0, 250.0), // delta = -150
        make_candle(3000, 200.0, 150.0), // delta = +50
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
    assert_eq!(pts.len(), 3);
    assert!((pts[0].1 - 100.0).abs() < 1.0);
    assert!((pts[1].1 - (-50.0)).abs() < 1.0);
    assert!((pts[2].1 - 0.0).abs() < 1.0);
}

#[test]
fn test_cvd_daily_reset() {
    let mut study = CvdStudy::new();
    study
        .set_parameter("reset_period", ParameterValue::Choice("Daily".to_string()))
        .unwrap();

    // Day 1: 2 candles
    let day1_start = 86_400_000u64; // start of day 1
    // Day 2: 1 candle
    let day2_start = 86_400_000u64 * 2;

    let candles = vec![
        make_candle(day1_start, 300.0, 200.0),          // +100
        make_candle(day1_start + 60_000, 200.0, 100.0), // +100 => cum 200
        make_candle(day2_start, 100.0, 300.0),          // -200, reset => -200
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
    assert_eq!(pts.len(), 3);
    assert!((pts[0].1 - 100.0).abs() < 1.0);
    assert!((pts[1].1 - 200.0).abs() < 1.0);
    // Day boundary reset, so cumulative starts fresh
    assert!((pts[2].1 - (-200.0)).abs() < 1.0);
}

#[test]
fn test_cvd_no_reset() {
    let mut study = CvdStudy::new();

    let day1_start = 86_400_000u64;
    let day2_start = 86_400_000u64 * 2;

    let candles = vec![
        make_candle(day1_start, 300.0, 200.0),
        make_candle(day2_start, 100.0, 300.0),
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
    // No reset, so cumulative continues
    assert!((pts[0].1 - 100.0).abs() < 1.0);
    assert!((pts[1].1 - (-100.0)).abs() < 1.0);
}

#[test]
fn single_candle_cvd_equals_delta() {
    let mut study = CvdStudy::new();
    let candles = vec![make_candle(1000, 500.0, 200.0)]; // delta = +300
    let input = make_input(&candles);
    study.compute(&input).unwrap();

    let StudyOutput::Lines(lines) = study.output() else {
        panic!("expected Lines output")
    };
    assert_eq!(lines[0].points.len(), 1);
    assert!((lines[0].points[0].1 - 300.0).abs() < 1.0);
}

#[test]
fn zero_volume_candles_cvd_stays_flat() {
    let mut study = CvdStudy::new();
    let candles = vec![
        make_candle(1000, 100.0, 50.0), // delta = +50
        make_candle(2000, 0.0, 0.0),    // delta = 0
        make_candle(3000, 0.0, 0.0),    // delta = 0
    ];
    let input = make_input(&candles);
    study.compute(&input).unwrap();

    let StudyOutput::Lines(lines) = study.output() else {
        panic!("expected Lines output")
    };
    let pts = &lines[0].points;
    assert!((pts[0].1 - 50.0).abs() < 1.0);
    assert!((pts[1].1 - 50.0).abs() < 1.0); // unchanged
    assert!((pts[2].1 - 50.0).abs() < 1.0); // unchanged
}

#[test]
fn cvd_all_negative_deltas() {
    let mut study = CvdStudy::new();
    let candles = vec![
        make_candle(1000, 100.0, 200.0), // delta = -100
        make_candle(2000, 50.0, 300.0),  // delta = -250
        make_candle(3000, 0.0, 100.0),   // delta = -100
    ];
    let input = make_input(&candles);
    study.compute(&input).unwrap();

    let StudyOutput::Lines(lines) = study.output() else {
        panic!("expected Lines output")
    };
    let pts = &lines[0].points;
    assert!((pts[0].1 - (-100.0)).abs() < 1.0);
    assert!((pts[1].1 - (-350.0)).abs() < 1.0);
    assert!((pts[2].1 - (-450.0)).abs() < 1.0);
}

#[test]
fn cvd_reset_clears_output() {
    let mut study = CvdStudy::new();
    let candles = vec![make_candle(1000, 200.0, 100.0)];
    let input = make_input(&candles);
    study.compute(&input).unwrap();
    assert!(!matches!(study.output(), StudyOutput::Empty));

    study.reset();
    assert!(matches!(study.output(), StudyOutput::Empty));
}
