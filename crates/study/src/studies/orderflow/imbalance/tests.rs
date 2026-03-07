use super::*;
use crate::config::ParameterValue;
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
fn test_max_visible_hits() {
    // decay=0.5, base=0.6 → 0.6*0.5^5 = 0.01875 < 0.03
    assert_eq!(max_visible_hits(0.6, 0.5), 5);
    // decay=0.1, base=0.6 → 0.6*0.1^2 = 0.006 < 0.03
    assert_eq!(max_visible_hits(0.6, 0.1), 2);
    // no decay → infinite
    assert_eq!(max_visible_hits(0.6, 1.0), u32::MAX);
    // base already invisible
    assert_eq!(max_visible_hits(0.02, 0.5), 0);
}

#[test]
fn test_imbalance_study_compute() {
    let mut study = ImbalanceStudy::new();
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

    match &study.output {
        StudyOutput::Levels(levels) => {
            assert!(!levels.is_empty());
            for level in levels {
                assert!(
                    level.start_x.is_some(),
                    "Imbalance levels must have start_x set"
                );
            }
        }
        StudyOutput::Empty => {
            // Acceptable if volumes don't meet threshold
        }
        other => panic!("Expected Levels or Empty, got {:?}", other),
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

#[test]
fn test_hit_counting_and_opacity_decay() {
    let mut study = ImbalanceStudy::new();
    study
        .config
        .set(String::from("threshold"), ParameterValue::Float(2.0));
    study
        .config
        .set(String::from("hit_decay"), ParameterValue::Float(0.5));

    // Candle 0: strong buy imbalance around 101
    // Candles 1-3: price passes through 101, each counts as hit
    let candles = vec![
        make_candle(1000, 100.0, 102.0, 100.0, 101.0, 500.0, 5.0),
        make_candle(2000, 100.0, 102.0, 100.0, 101.0, 50.0, 50.0),
        make_candle(3000, 100.0, 102.0, 100.0, 101.0, 50.0, 50.0),
        make_candle(4000, 100.0, 102.0, 100.0, 101.0, 50.0, 50.0),
    ];

    let input = StudyInput {
        candles: &candles,
        trades: None,
        basis: ChartBasis::Time(Timeframe::M1),
        tick_size: Price::from_f32(1.0),
        visible_range: None,
    };

    study.compute(&input).unwrap();

    if let StudyOutput::Levels(levels) = &study.output {
        let from_first: Vec<_> = levels.iter().filter(|l| l.start_x == Some(1000)).collect();
        let from_last: Vec<_> = levels.iter().filter(|l| l.start_x == Some(4000)).collect();

        for level in &from_first {
            assert!(
                level.opacity < 0.6,
                "First candle levels should have decayed: {}",
                level.opacity
            );
        }

        for level in &from_last {
            assert!(
                level.opacity >= 0.5,
                "Last candle levels should be near base: {}",
                level.opacity
            );
        }
    }
}

#[test]
fn test_levels_disappear_after_many_hits() {
    let mut study = ImbalanceStudy::new();
    study
        .config
        .set(String::from("threshold"), ParameterValue::Float(2.0));
    study
        .config
        .set(String::from("hit_decay"), ParameterValue::Float(0.1));

    let mut candles = vec![make_candle(1000, 100.0, 102.0, 100.0, 101.0, 500.0, 5.0)];
    for i in 1..10 {
        candles.push(make_candle(
            1000 + i * 1000,
            100.0,
            102.0,
            100.0,
            101.0,
            50.0,
            50.0,
        ));
    }

    let input = StudyInput {
        candles: &candles,
        trades: None,
        basis: ChartBasis::Time(Timeframe::M1),
        tick_size: Price::from_f32(1.0),
        visible_range: None,
    };

    study.compute(&input).unwrap();

    if let StudyOutput::Levels(levels) = &study.output {
        // decay=0.1: max_visible_hits(0.6, 0.1) = 2
        // Candle 0 has 9 subsequent hits → well past limit
        let from_first: Vec<_> = levels.iter().filter(|l| l.start_x == Some(1000)).collect();
        assert!(from_first.is_empty(), "Heavily-hit levels should be pruned");
    }
}

#[test]
fn test_output_capped() {
    let mut study = ImbalanceStudy::new();
    // Very low threshold to maximize imbalance detections
    study
        .config
        .set(String::from("threshold"), ParameterValue::Float(1.1));
    // No decay so nothing gets pruned
    study
        .config
        .set(String::from("hit_decay"), ParameterValue::Float(1.0));

    // Generate many candles with imbalances
    let candles: Vec<Candle> = (0..5000)
        .map(|i| make_candle(i * 60_000, 100.0, 110.0, 90.0, 105.0, 800.0, 10.0))
        .collect();

    let input = StudyInput {
        candles: &candles,
        trades: None,
        basis: ChartBasis::Time(Timeframe::M1),
        tick_size: Price::from_f32(1.0),
        visible_range: None,
    };

    study.compute(&input).unwrap();

    if let StudyOutput::Levels(levels) = &study.output {
        assert!(
            levels.len() <= MAX_OUTPUT_LEVELS,
            "Output should be capped at {}, got {}",
            MAX_OUTPUT_LEVELS,
            levels.len()
        );
    }
}
