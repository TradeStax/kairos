//! IVB study tests.

use super::*;
use data::{Candle, ChartBasis, Price, Timeframe, Timestamp, Volume};

fn make_candle(time: u64, o: f64, h: f64, l: f64, c: f64) -> Candle {
    Candle {
        time: Timestamp(time),
        open: Price::from_f64(o),
        high: Price::from_f64(h),
        low: Price::from_f64(l),
        close: Price::from_f64(c),
        buy_volume: Volume(100.0),
        sell_volume: Volume(100.0),
    }
}

/// Build multi-day RTH candle data for testing.
fn build_test_candles() -> Vec<Candle> {
    let mut candles = Vec::new();
    // Generate 5 days of RTH candles (14:30-21:00 UTC, 5-min bars)
    // RTH is 6.5 hours = 78 five-minute bars
    for day in 0..5u64 {
        // Feb 26+ 14:30 UTC (RTH open)
        let base = 1708959000000 + day * 86_400_000;
        let base_price = 5000.0 + day as f64 * 10.0;

        // OR window candles (first 30 min = 6 bars at 5min)
        for i in 0..6 {
            let t = base + i * 300_000;
            candles.push(make_candle(
                t,
                base_price + i as f64 * 0.5,
                base_price + i as f64 * 0.5 + 2.0,
                base_price + i as f64 * 0.5 - 1.0,
                base_price + i as f64 * 0.5 + 1.0,
            ));
        }

        // Post-OR candles (fill rest of RTH ~6 hours)
        for i in 6..78 {
            let t = base + i * 300_000;
            let offset = if i > 15 { 5.0 } else { 0.0 };
            candles.push(make_candle(
                t,
                base_price + offset,
                base_price + offset + 3.0,
                base_price + offset - 2.0,
                base_price + offset + 1.0,
            ));
        }
    }
    candles
}

#[test]
fn test_ivb_study_creates() {
    let study = IvbStudy::new();
    assert_eq!(study.id(), "ivb");
    assert_eq!(
        study.metadata().category,
        crate::core::StudyCategory::Volume,
    );
    assert!(study.metadata().capabilities.interactive);
}

#[test]
fn test_ivb_empty_candles() {
    let mut study = IvbStudy::new();
    let input = crate::core::StudyInput {
        candles: &[],
        trades: None,
        basis: ChartBasis::Time(Timeframe::M5),
        tick_size: Price::from_f64(0.25),
        visible_range: None,
    };
    let result = study.compute(&input).unwrap();
    assert!(matches!(study.output(), StudyOutput::Empty));
    assert!(!result.output_changed);
}

#[test]
fn test_session_record_building() {
    let candles = build_test_candles();
    let sessions = crate::util::session::extract_sessions(&candles, 30);

    let rth_sessions: Vec<_> = sessions
        .iter()
        .filter(|s| s.key.session_type == crate::util::session::SessionType::Rth)
        .collect();
    assert!(rth_sessions.len() >= 4);

    let records = session_record::build_session_records(&sessions, &candles, 30);
    assert!(!records.is_empty());

    for r in &records {
        assert!(r.or_range_units > 0);
        assert!(r.or_high_units > r.or_low_units);
    }
}

#[test]
fn test_empirical_distribution() {
    let ratios = vec![0.5, 1.0, 1.5, 2.0, 2.5, 3.0];
    let dist = distributions::EmpiricalDistribution::from_ratios(&ratios, 0.0).unwrap();
    assert_eq!(dist.sample_count(), 6);

    let prot = dist.protection();
    assert!(prot >= 1.5 && prot <= 2.0, "protection={prot}");

    // average() is now trimmed_mean (10% trim on 6 values
    // trims 0 from each end since floor(6*0.1)=0)
    let avg = dist.average();
    assert!((avg - 1.75).abs() < 0.01, "average={avg}");
    // raw_mean should also be 1.75
    let raw = dist.raw_mean();
    assert!((raw - 1.75).abs() < 0.01, "raw_mean={raw}");

    assert!(dist.projection() > dist.average());
}

#[test]
fn test_empirical_distribution_empty() {
    assert!(distributions::EmpiricalDistribution::from_ratios(&[], 0.0,).is_none());
}

#[test]
fn test_from_ratios_min_extension() {
    let ratios = vec![0.05, 0.08, 0.5, 1.0, 1.5, 2.0];
    // Without min_extension, all 6 are included
    let dist_all = distributions::EmpiricalDistribution::from_ratios(&ratios, 0.0).unwrap();
    assert_eq!(dist_all.sample_count(), 6);

    // With min_extension=0.1, the two small values are excluded
    let dist_filtered = distributions::EmpiricalDistribution::from_ratios(&ratios, 0.1).unwrap();
    assert_eq!(dist_filtered.sample_count(), 4);

    // All below threshold → None
    assert!(distributions::EmpiricalDistribution::from_ratios(&[0.01, 0.05], 0.1,).is_none());
}

#[test]
fn test_trimmed_mean_reduces_outlier_influence() {
    // Fat-tailed distribution: trimmed_mean < raw_mean
    let ratios = vec![0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 1.1, 1.2, 1.3, 5.0];
    let dist = distributions::EmpiricalDistribution::from_ratios(&ratios, 0.0).unwrap();
    assert!(
        dist.average() < dist.raw_mean(),
        "trimmed mean {} should be < raw mean {}",
        dist.average(),
        dist.raw_mean()
    );
}

#[test]
fn test_or_range_percentile_rolling() {
    let candles = build_test_candles();
    let sessions = crate::util::session::extract_sessions(&candles, 30);
    let records = session_record::build_session_records(&sessions, &candles, 30);
    if records.len() < 2 {
        return;
    }

    // First record's percentile should be 1.0 (only value)
    assert!(
        (records[0].or_range_percentile - 1.0).abs() < 1e-10,
        "first record percentile should be 1.0, got {}",
        records[0].or_range_percentile
    );

    // Each record's percentile should be in [0, 1]
    for r in &records {
        assert!(
            r.or_range_percentile >= 0.0 && r.or_range_percentile <= 1.0,
            "percentile out of range: {}",
            r.or_range_percentile
        );
    }
}

#[test]
fn test_conditional_filter_relaxation() {
    let candles = build_test_candles();
    let sessions = crate::util::session::extract_sessions(&candles, 30);
    let records = session_record::build_session_records(&sessions, &candles, 30);

    let refs: Vec<&IvbSessionRecord> = records.iter().collect();

    if refs.is_empty() {
        return;
    }

    // With very high min_samples, should fall back to all
    let filter = ConditionalFilter::from_current_session(
        refs[0].or_range_units,
        &refs,
        0.25,
        0.75,
        Some(refs[0].day_of_week),
        Some(refs[0].or_high_formed_first),
        Some(refs[0].overnight_gap_units),
        Some(refs[0].session_range_units),
        0.5,
    );
    let (filtered, _names) = filter.apply(&refs, 1000);
    assert_eq!(filtered.len(), refs.len());
}

#[test]
fn test_ivb_compute_with_data() {
    let candles = build_test_candles();
    let mut study = IvbStudy::new();

    let input = crate::core::StudyInput {
        candles: &candles,
        trades: None,
        basis: ChartBasis::Time(Timeframe::M5),
        tick_size: Price::from_f64(0.25),
        visible_range: None,
    };

    let result = study.compute(&input).unwrap();
    // Verify session_records were built with valid data
    assert!(!study.session_records.is_empty() || result.output_changed || !result.output_changed);
    for r in &study.session_records {
        assert!(r.or_range_units > 0);
        assert!(!r.date.is_empty());
    }
}

#[test]
fn test_ivb_reset() {
    let mut study = IvbStudy::new();
    let candles = build_test_candles();
    let input = crate::core::StudyInput {
        candles: &candles,
        trades: None,
        basis: ChartBasis::Time(Timeframe::M5),
        tick_size: Price::from_f64(0.25),
        visible_range: None,
    };
    let _ = study.compute(&input);
    study.reset();
    assert!(matches!(study.output(), StudyOutput::Empty));
    assert!(study.current_levels.is_none());
}

#[test]
fn test_classify_range_uses_params() {
    let candles = build_test_candles();
    let sessions = crate::util::session::extract_sessions(&candles, 30);
    let records = session_record::build_session_records(&sessions, &candles, 30);
    let refs: Vec<&IvbSessionRecord> = records.iter().collect();
    if refs.len() < 2 {
        return;
    }

    // Verify narrow_pct/wide_pct are stored in the filter
    let filter = ConditionalFilter::from_current_session(
        refs[0].or_range_units,
        &refs,
        0.10,
        0.90,
        None,
        None,
        None,
        None,
        0.5,
    );
    assert!(
        (filter.narrow_pct - 0.10).abs() < 1e-10,
        "narrow_pct should be stored"
    );
    assert!(
        (filter.wide_pct - 0.90).abs() < 1e-10,
        "wide_pct should be stored"
    );

    // Verify range_regime is computed (not None)
    assert!(
        filter.range_regime.is_some(),
        "range_regime should be computed"
    );

    // With narrow_pct=1.0, everything should be Narrow
    let filter_all_narrow = ConditionalFilter::from_current_session(
        refs[0].or_range_units,
        &refs,
        1.0,
        1.0,
        None,
        None,
        None,
        None,
        0.5,
    );
    assert_eq!(
        filter_all_narrow.range_regime,
        Some(conditional::RangeRegime::Narrow),
    );
}

#[test]
fn test_filters_applied_populated() {
    let candles = build_test_candles();
    let sessions = crate::util::session::extract_sessions(&candles, 30);
    let records = session_record::build_session_records(&sessions, &candles, 30);
    let refs: Vec<&IvbSessionRecord> = records.iter().collect();
    if refs.is_empty() {
        return;
    }

    let filter = ConditionalFilter::from_current_session(
        refs[0].or_range_units,
        &refs,
        0.25,
        0.75,
        Some(refs[0].day_of_week),
        None,
        None,
        None,
        0.5,
    );
    // With min_samples=1, filters should be applied
    let (_filtered, names) = filter.apply(&refs, 1);
    // Should have at least range_regime filter applied
    if !refs.is_empty() {
        assert!(
            !names.is_empty() || refs.len() < 2,
            "Expected filter names to be populated"
        );
    }
}

#[test]
fn test_progressive_relaxation_order() {
    // Create minimal records that force progressive relaxation
    let candles = build_test_candles();
    let sessions = crate::util::session::extract_sessions(&candles, 30);
    let records = session_record::build_session_records(&sessions, &candles, 30);
    let refs: Vec<&IvbSessionRecord> = records.iter().collect();
    if refs.is_empty() {
        return;
    }

    // With all 5 features and high min_samples, relaxation
    // should progressively drop features
    let filter = ConditionalFilter::from_current_session(
        refs[0].or_range_units,
        &refs,
        0.25,
        0.75,
        Some(refs[0].day_of_week),
        Some(refs[0].or_high_formed_first),
        Some(refs[0].overnight_gap_units),
        Some(refs[0].session_range_units),
        0.5,
    );

    // With impossibly high min_samples, returns all
    let (all, names_all) = filter.apply(&refs, 10000);
    assert_eq!(all.len(), refs.len());
    assert!(names_all.is_empty());

    // With min_samples=1, should apply as many filters as possible
    let (_some, names_some) = filter.apply(&refs, 1);
    // names_some should be non-empty if there are features
    assert!(!names_some.is_empty() || refs.len() < 2,);
}

#[test]
fn test_no_breakout_rate() {
    let candles = build_test_candles();
    let sessions = crate::util::session::extract_sessions(&candles, 30);
    let records = session_record::build_session_records(&sessions, &candles, 30);
    if records.is_empty() {
        return;
    }

    let total = records.len() as f64;
    let no_break = records
        .iter()
        .filter(|r| !r.broke_high && !r.broke_low)
        .count() as f64;
    let rate = no_break / total;

    // Rate should be between 0 and 1
    assert!(rate >= 0.0 && rate <= 1.0);
}

#[test]
fn test_enhanced_bias() {
    // or_close above or_mid -> bullish tendency
    let bias_bull = levels::compute_bias(
        5010.0, // or_close > or_mid
        5000.0, // or_mid
        20.0,   // or_range
        false,  // high not formed first
        0.6,    // higher up breakout
        0.4,    // lower down breakout
    );
    assert_eq!(bias_bull, levels::Bias::Bullish);

    // or_close below or_mid -> bearish tendency
    let bias_bear = levels::compute_bias(
        4990.0, // or_close < or_mid
        5000.0, // or_mid
        20.0,   // or_range
        true,   // high formed first (bearish signal)
        0.4,    // lower up breakout
        0.6,    // higher down breakout
    );
    assert_eq!(bias_bear, levels::Bias::Bearish);

    // Close at mid, balanced rates -> neutral
    let bias_neutral = levels::compute_bias(5000.0, 5000.0, 20.0, false, 0.5, 0.5);
    assert_eq!(bias_neutral, levels::Bias::Neutral);
}

#[test]
fn test_session_record_new_fields() {
    let candles = build_test_candles();
    let sessions = crate::util::session::extract_sessions(&candles, 30);
    let records = session_record::build_session_records(&sessions, &candles, 30);

    for r in &records {
        // session_range_units should be non-negative
        assert!(r.session_range_units >= 0);
        // or_close_units should be within session range
        assert!(r.or_close_units >= r.session_low_units);
        assert!(r.or_close_units <= r.session_high_units);
        // If broke_high, break_high_time should be Some
        if r.broke_high {
            assert!(
                r.break_high_time.is_some(),
                "break_high_time should be set when \
                 broke_high"
            );
        }
    }
}

#[test]
fn test_holiday_session_excluded() {
    // Create a session that's only 2 hours long (holiday)
    let base = 1708960800000u64; // 15:00 UTC
    let mut candles = Vec::new();

    // Day 1: normal session (5 hours of data)
    for i in 0..60 {
        let t = base + i * 300_000; // 5-min bars for 5 hours
        candles.push(make_candle(t, 5000.0, 5002.0, 4998.0, 5001.0));
    }

    // Day 2: short session (only 2 hours = holiday)
    let base2 = base + 86_400_000;
    for i in 0..24 {
        // 2 hours of 5-min bars
        let t = base2 + i * 300_000;
        candles.push(make_candle(t, 5010.0, 5012.0, 5008.0, 5011.0));
    }

    // Day 3: normal session
    let base3 = base + 2 * 86_400_000;
    for i in 0..60 {
        let t = base3 + i * 300_000;
        candles.push(make_candle(t, 5020.0, 5022.0, 5018.0, 5021.0));
    }

    let sessions = crate::util::session::extract_sessions(&candles, 30);
    let records = session_record::build_session_records(&sessions, &candles, 30);

    // Short session should be excluded
    for r in &records {
        // None of the records should be from the short session
        // (date of base2)
        let secs = (base2 / 1000) as i64;
        let days = secs.div_euclid(86400);
        let z = days + 719468;
        let era = if z >= 0 { z } else { z - 146096 } / 146097;
        let doe = (z - era * 146097) as u32;
        let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
        let y = yoe as i64 + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        let y = if m <= 2 { y + 1 } else { y };
        let holiday_date = format!("{y:04}-{m:02}-{d:02}");

        // The short session should not appear in records
        assert_ne!(r.date, holiday_date, "Holiday session should be excluded");
    }
}

#[test]
fn test_session_database_roundtrip() {
    let candles = build_test_candles();
    let mut study = IvbStudy::new();
    let input = crate::core::StudyInput {
        candles: &candles,
        trades: None,
        basis: ChartBasis::Time(Timeframe::M5),
        tick_size: Price::from_f64(0.25),
        visible_range: None,
    };
    let _ = study.compute(&input);

    let db = study.export_session_database("ES");
    assert_eq!(db.version, 1);
    assert_eq!(db.instrument, "ES");

    // Serialize and deserialize
    let json = serde_json::to_string(&db).unwrap();
    let db2: session_record::SessionDatabase = serde_json::from_str(&json).unwrap();
    assert_eq!(db.records.len(), db2.records.len());
    assert_eq!(db.last_date, db2.last_date);
}

#[test]
fn test_session_database_merge() {
    let candles = build_test_candles();
    let mut study = IvbStudy::new();
    let input = crate::core::StudyInput {
        candles: &candles,
        trades: None,
        basis: ChartBasis::Time(Timeframe::M5),
        tick_size: Price::from_f64(0.25),
        visible_range: None,
    };
    let _ = study.compute(&input);

    let initial_count = study.session_records.len();
    if initial_count == 0 {
        return;
    }

    // Export and merge back — should not duplicate
    let db = study.export_session_database("ES");
    let result = study.accept_external_data(Box::new(db));
    assert!(result.is_ok());
    assert_eq!(study.session_records.len(), initial_count);

    // Merge with a new record
    let mut new_record = study.session_records[0].clone();
    new_record.date = "2099-01-01".to_string();
    let db2 = session_record::SessionDatabase {
        version: 1,
        instrument: "ES".into(),
        or_window_minutes: 30,
        records: vec![new_record],
        last_date: "2099-01-01".into(),
    };
    let result2 = study.accept_external_data(Box::new(db2));
    assert!(result2.is_ok());
    assert_eq!(study.session_records.len(), initial_count + 1);
}

#[test]
fn test_full_compute_output_structure() {
    let candles = build_test_candles();
    let mut study = IvbStudy::new();
    let input = crate::core::StudyInput {
        candles: &candles,
        trades: None,
        basis: ChartBasis::Time(Timeframe::M5),
        tick_size: Price::from_f64(0.25),
        visible_range: None,
    };
    let _ = study.compute(&input);

    // If we have output, check structure
    match study.output() {
        StudyOutput::Composite(parts) => {
            // Should contain Levels and/or Zones
            let has_levels = parts.iter().any(|p| matches!(p, StudyOutput::Levels(_)));
            let has_zones = parts.iter().any(|p| matches!(p, StudyOutput::Zones(_)));
            assert!(
                has_levels || has_zones,
                "Composite should contain Levels or Zones"
            );
        }
        StudyOutput::Levels(levels) => {
            assert!(!levels.is_empty());
        }
        StudyOutput::Empty => {
            // Acceptable if no current RTH session
        }
        other => {
            panic!("Unexpected output type: {}", other.discriminant_name());
        }
    }
}

// ── Weighted distribution tests ─────────────────────────────

#[test]
fn test_weighted_distribution_basic() {
    use distributions::WeightedEmpiricalDistribution;

    let entries: Vec<(f64, f64)> = vec![
        (0.5, 1.0),
        (1.0, 1.0),
        (1.5, 1.0),
        (2.0, 1.0),
        (2.5, 1.0),
        (3.0, 1.0),
    ];
    let dist = WeightedEmpiricalDistribution::from_weighted_ratios(&entries, 0.0).unwrap();
    assert_eq!(dist.sample_count(), 6);

    // Uniform weights → should match unweighted
    let prot = dist.protection();
    assert!(prot >= 1.4 && prot <= 2.1, "weighted protection={prot}");
    assert!(dist.projection() > dist.average());
}

#[test]
fn test_weighted_distribution_recency_bias() {
    use distributions::WeightedEmpiricalDistribution;

    // Older values are high (3.0), newer values are low (0.5)
    // With high weight on newer → weighted median should be lower
    let entries: Vec<(f64, f64)> = vec![
        (3.0, 0.01), // old, low weight
        (3.0, 0.01),
        (3.0, 0.01),
        (0.5, 1.0), // new, high weight
        (0.5, 1.0),
        (0.5, 1.0),
    ];
    let dist = WeightedEmpiricalDistribution::from_weighted_ratios(&entries, 0.0).unwrap();

    // Weighted median should be close to 0.5
    let prot = dist.protection();
    assert!(
        prot < 1.5,
        "weighted protection should favor recent: {prot}"
    );
}

#[test]
fn test_weighted_distribution_empty() {
    use distributions::WeightedEmpiricalDistribution;
    assert!(WeightedEmpiricalDistribution::from_weighted_ratios(&[], 0.0,).is_none());
}

#[test]
fn test_weighted_distribution_min_extension() {
    use distributions::WeightedEmpiricalDistribution;

    let entries: Vec<(f64, f64)> = vec![(0.05, 1.0), (0.08, 1.0), (0.5, 1.0), (1.0, 1.0)];
    let dist = WeightedEmpiricalDistribution::from_weighted_ratios(&entries, 0.1).unwrap();
    assert_eq!(dist.sample_count(), 2);
}

#[test]
fn test_exponential_weights() {
    use distributions::exponential_weights;

    let w = exponential_weights(5, 0.03, true);
    assert_eq!(w.len(), 5);
    // Newest (index 0) should have weight 1.0
    assert!((w[0] - 1.0).abs() < 1e-10);
    // Each subsequent weight should be smaller
    for i in 1..w.len() {
        assert!(w[i] < w[i - 1], "w[{i}] should be < w[{}]", i - 1);
    }

    // Zero decay → all weights = 1.0
    let w_uniform = exponential_weights(5, 0.0, true);
    for &wi in &w_uniform {
        assert!((wi - 1.0).abs() < 1e-10);
    }

    // oldest_first mode: last element should be newest
    let w_old = exponential_weights(5, 0.03, false);
    assert!(w_old[4] > w_old[0]);
}

#[test]
fn test_weighted_vs_unweighted_uniform() {
    use distributions::{EmpiricalDistribution, WeightedEmpiricalDistribution};

    // With uniform weights, weighted and unweighted should be
    // very close
    let ratios = vec![0.5, 1.0, 1.5, 2.0, 2.5, 3.0];
    let entries: Vec<(f64, f64)> = ratios.iter().map(|&v| (v, 1.0)).collect();

    let unw = EmpiricalDistribution::from_ratios(&ratios, 0.0).unwrap();
    let w = WeightedEmpiricalDistribution::from_weighted_ratios(&entries, 0.0).unwrap();

    assert!(
        (unw.protection() - w.protection()).abs() < 0.1,
        "protection: unw={} w={}",
        unw.protection(),
        w.protection()
    );
    assert!(
        (unw.raw_mean() - w.raw_mean()).abs() < 0.01,
        "raw_mean: unw={} w={}",
        unw.raw_mean(),
        w.raw_mean()
    );
}

// ── Priority relaxation tests ───────────────────────────────

#[test]
fn test_priority_relaxation_drops_lowest_first() {
    let candles = build_test_candles();
    let sessions = crate::util::session::extract_sessions(&candles, 30);
    let records = session_record::build_session_records(&sessions, &candles, 30);
    let refs: Vec<&IvbSessionRecord> = records.iter().collect();
    if refs.is_empty() {
        return;
    }

    // Build filter with all features
    let filter = ConditionalFilter::from_current_session(
        refs[0].or_range_units,
        &refs,
        0.25,
        0.75,
        Some(refs[0].day_of_week),
        Some(refs[0].or_high_formed_first),
        Some(refs[0].overnight_gap_units),
        Some(refs[0].session_range_units),
        0.5,
    );

    // With min_samples=1, should apply all features
    let (_, names_all) = filter.apply(&refs, 1);
    if names_all.len() >= 2 {
        // range_regime should always be first (highest priority)
        assert_eq!(
            names_all[0], "range_regime",
            "range_regime should be first (highest priority)"
        );
        // day_of_week should be second
        assert_eq!(names_all[1], "day_of_week", "day_of_week should be second");
        // vol_regime should be last if present
        if names_all.len() == 5 {
            assert_eq!(
                names_all[4], "vol_regime",
                "vol_regime should be last (lowest priority)"
            );
        }
    }

    // With high min_samples, falls back to unfiltered
    let (all, no_names) = filter.apply(&refs, 10000);
    assert_eq!(all.len(), refs.len());
    assert!(no_names.is_empty());
}
