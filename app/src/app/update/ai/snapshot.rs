//! Chart Snapshot Builder
//!
//! Captures an immutable snapshot of chart data at request time so
//! the async streaming function needs no mutable access to pane state.

use crate::config::UserTimezone;
use crate::screen::dashboard::pane;
use data::domain::assistant::{
    BigTradeSnapshot, ChartSnapshot, DrawingPointSnapshot, DrawingSnapshot,
    FootprintCandleSnapshot, FootprintLevelSnapshot, ProfileLevelSnapshot, ProfileSnapshot,
    StudyOutputSnapshot,
};

const MAX_TRADES: usize = 100_000;
const MAX_STUDY_POINTS: usize = 200;

/// Build a `ChartSnapshot` from a pane's state, if it has chart data.
pub(crate) fn build_chart_snapshot(
    state: &pane::State,
    timezone: UserTimezone,
) -> Option<ChartSnapshot> {
    let ticker = state.get_ticker()?;
    let chart_data = state.chart_data.as_ref()?;

    let kind = state.content.kind();
    let selected_basis = state.settings.selected_basis;
    let timeframe = selected_basis
        .map(|b| format!("{}", b))
        .unwrap_or_else(|| "default".to_string());
    let is_tick_basis = selected_basis.is_some_and(|b| b.is_tick());

    let is_live = state.feed_id.is_some();

    let active_studies: Vec<String> = match &state.content {
        pane::Content::Candlestick { study_ids, .. } | pane::Content::Profile { study_ids, .. } => {
            study_ids.clone()
        }
        _ => vec![],
    };

    let trades_len = chart_data.trades.len();
    let trades_truncated = trades_len > MAX_TRADES;
    let trades = if trades_truncated {
        chart_data.trades[trades_len - MAX_TRADES..].to_vec()
    } else {
        chart_data.trades.clone()
    };

    let tz_label = format!("{}", timezone);

    let date_range_display = if !chart_data.candles.is_empty() {
        let first = chart_data.candles.first().unwrap();
        let last = chart_data.candles.last().unwrap();
        let fmt = |ts: u64| {
            let secs = (ts / 1_000) as i64;
            match timezone {
                UserTimezone::Local => chrono::DateTime::from_timestamp(secs, 0)
                    .map(|dt| {
                        dt.with_timezone(&chrono::Local)
                            .format("%Y-%m-%d %H:%M")
                            .to_string()
                    })
                    .unwrap_or_else(|| "?".to_string()),
                UserTimezone::Utc => chrono::DateTime::from_timestamp(secs, 0)
                    .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                    .unwrap_or_else(|| "?".to_string()),
            }
        };
        Some((fmt(first.time.0), fmt(last.time.0)))
    } else {
        None
    };

    // Extract study snapshots from chart
    let extracted = extract_studies(&state.content);

    // Extract drawing snapshots
    let drawing_snapshots = extract_drawings(&state.content);

    Some(ChartSnapshot {
        ticker: ticker.ticker.as_str().to_string(),
        tick_size: ticker.tick_size,
        contract_size: ticker.contract_size,
        timeframe,
        chart_type: format!("{}", kind),
        is_live,
        candles: chart_data.candles.clone(),
        trades,
        trades_truncated,
        active_studies,
        date_range_display,
        study_snapshots: extracted.snapshots,
        big_trade_markers: extracted.big_trades,
        timezone: tz_label,
        footprint_candles: extracted.footprint_candles,
        profile_snapshots: extracted.profile_snapshots,
        drawing_snapshots,
        visible_price_high: None,
        visible_price_low: None,
        visible_time_start: None,
        visible_time_end: None,
        is_tick_basis,
    })
}

/// All study-derived data extracted from chart content.
struct ExtractedStudyData {
    snapshots: Vec<StudyOutputSnapshot>,
    big_trades: Vec<BigTradeSnapshot>,
    footprint_candles: Vec<FootprintCandleSnapshot>,
    profile_snapshots: Vec<ProfileSnapshot>,
}

/// Extract study snapshots, big trades, footprint, and profile data
/// from chart content.
fn extract_studies(content: &pane::Content) -> ExtractedStudyData {
    let empty: &[Box<dyn study::Study>] = &[];
    let studies: &[Box<dyn study::Study>] = match content {
        pane::Content::Candlestick { chart, .. } => {
            if let Some(c) = (**chart).as_ref() {
                c.studies()
            } else {
                empty
            }
        }
        pane::Content::Profile { chart, .. } => {
            (**chart).as_ref().map(|c| c.studies()).unwrap_or(empty)
        }
        _ => {
            return ExtractedStudyData {
                snapshots: vec![],
                big_trades: vec![],
                footprint_candles: vec![],
                profile_snapshots: vec![],
            };
        }
    };

    let mut snapshots = Vec::new();
    let mut big_trades = Vec::new();
    let mut footprint_candles = Vec::new();
    let mut profile_snapshots = Vec::new();

    for s in studies {
        let output = s.output();
        let mut snap = StudyOutputSnapshot {
            study_id: s.id().to_string(),
            study_name: s.name().to_string(),
            line_values: vec![],
            bar_values: vec![],
            levels: vec![],
        };

        extract_from_output(
            output,
            &mut snap,
            &mut big_trades,
            &mut footprint_candles,
            &mut profile_snapshots,
        );

        if !snap.line_values.is_empty() || !snap.bar_values.is_empty() || !snap.levels.is_empty() {
            snapshots.push(snap);
        }
    }

    ExtractedStudyData {
        snapshots,
        big_trades,
        footprint_candles,
        profile_snapshots,
    }
}

/// Recursively extract data from a StudyOutput.
fn extract_from_output(
    output: &study::StudyOutput,
    snap: &mut StudyOutputSnapshot,
    big_trades: &mut Vec<BigTradeSnapshot>,
    footprint_candles: &mut Vec<FootprintCandleSnapshot>,
    profile_snapshots: &mut Vec<ProfileSnapshot>,
) {
    match output {
        study::StudyOutput::Lines(lines) => {
            for line in lines {
                let n = line.points.len();
                let start = n.saturating_sub(MAX_STUDY_POINTS);
                let points: Vec<(u64, f32)> = line.points[start..]
                    .iter()
                    .map(|(t, v)| (*t / 1_000, *v))
                    .collect();
                snap.line_values.push((line.label.clone(), points));
            }
        }
        study::StudyOutput::Band {
            upper,
            middle,
            lower,
            ..
        } => {
            for line in [Some(upper), middle.as_ref(), Some(lower)]
                .into_iter()
                .flatten()
            {
                let n = line.points.len();
                let start = n.saturating_sub(MAX_STUDY_POINTS);
                let points: Vec<(u64, f32)> = line.points[start..]
                    .iter()
                    .map(|(t, v)| (*t / 1_000, *v))
                    .collect();
                snap.line_values.push((line.label.clone(), points));
            }
        }
        study::StudyOutput::Bars(bar_series) => {
            for series in bar_series {
                let n = series.points.len();
                let start = n.saturating_sub(MAX_STUDY_POINTS);
                let points: Vec<(u64, f32)> = series.points[start..]
                    .iter()
                    .map(|p| (p.x / 1_000, p.value))
                    .collect();
                snap.bar_values.push((series.label.clone(), points));
            }
        }
        study::StudyOutput::Histogram(bars) => {
            let n = bars.len();
            let start = n.saturating_sub(MAX_STUDY_POINTS);
            let points: Vec<(u64, f32)> = bars[start..]
                .iter()
                .map(|b| (b.x / 1_000, b.value))
                .collect();
            snap.bar_values.push(("Histogram".to_string(), points));
        }
        study::StudyOutput::Levels(lvls) => {
            for lvl in lvls {
                snap.levels.push((lvl.label.clone(), lvl.price));
            }
        }
        study::StudyOutput::Markers(marker_data) => {
            for m in &marker_data.markers {
                big_trades.push(BigTradeSnapshot {
                    time: m.time,
                    price: data::Price::from_units(m.price).to_f64(),
                    quantity: m.contracts,
                    is_buy: m.is_buy,
                });
            }
        }
        study::StudyOutput::Profile(profiles, _config) => {
            for profile in profiles {
                let total_volume: f64 = profile
                    .levels
                    .iter()
                    .map(|l| (l.buy_volume + l.sell_volume) as f64)
                    .sum();
                let poc_price = profile
                    .poc
                    .and_then(|idx| profile.levels.get(idx).map(|l| l.price));
                let (va_high, va_low) = match profile.value_area {
                    Some((hi_idx, lo_idx)) => (
                        profile.levels.get(hi_idx).map(|l| l.price),
                        profile.levels.get(lo_idx).map(|l| l.price),
                    ),
                    None => (None, None),
                };
                let hvn_prices: Vec<f64> = profile
                    .hvn_zones
                    .iter()
                    .map(|(lo, hi)| {
                        let mid = (lo + hi) / 2;
                        data::Price::from_units(mid).to_f64()
                    })
                    .collect();
                let lvn_prices: Vec<f64> = profile
                    .lvn_zones
                    .iter()
                    .map(|(lo, hi)| {
                        let mid = (lo + hi) / 2;
                        data::Price::from_units(mid).to_f64()
                    })
                    .collect();
                let levels: Vec<ProfileLevelSnapshot> = profile
                    .levels
                    .iter()
                    .map(|l| ProfileLevelSnapshot {
                        price: l.price,
                        buy_volume: l.buy_volume,
                        sell_volume: l.sell_volume,
                    })
                    .collect();
                let time_range = profile.time_range.map(|(s, e)| (s / 1_000, e / 1_000));
                profile_snapshots.push(ProfileSnapshot {
                    levels,
                    poc_price,
                    value_area_high: va_high,
                    value_area_low: va_low,
                    total_volume,
                    hvn_prices,
                    lvn_prices,
                    time_range,
                });
            }
        }
        study::StudyOutput::Footprint(fp_data) => {
            for candle in &fp_data.candles {
                let poc_price = candle.poc_index.and_then(|idx| {
                    candle
                        .levels
                        .get(idx)
                        .map(|l| data::Price::from_units(l.price).to_f64())
                });
                let levels: Vec<FootprintLevelSnapshot> = candle
                    .levels
                    .iter()
                    .map(|l| FootprintLevelSnapshot {
                        price: data::Price::from_units(l.price).to_f64(),
                        buy_volume: l.buy_volume,
                        sell_volume: l.sell_volume,
                    })
                    .collect();
                footprint_candles.push(FootprintCandleSnapshot {
                    time_secs: candle.x / 1_000,
                    open: data::Price::from_units(candle.open).to_f64(),
                    high: data::Price::from_units(candle.high).to_f64(),
                    low: data::Price::from_units(candle.low).to_f64(),
                    close: data::Price::from_units(candle.close).to_f64(),
                    poc_price,
                    levels,
                });
            }
        }
        study::StudyOutput::Composite(children) => {
            for child in children {
                extract_from_output(
                    child,
                    snap,
                    big_trades,
                    footprint_candles,
                    profile_snapshots,
                );
            }
        }
        study::StudyOutput::Empty => {}
    }
}

/// Extract drawing snapshots from chart content.
fn extract_drawings(content: &pane::Content) -> Vec<DrawingSnapshot> {
    let chart = match content.drawing_chart() {
        Some(c) => c,
        None => return vec![],
    };
    chart
        .drawings()
        .to_serializable()
        .iter()
        .filter(|d| d.visible && d.tool != crate::drawing::DrawingTool::AiContext)
        .map(|d| DrawingSnapshot {
            id: d.id.0.to_string(),
            tool_type: format!("{}", d.tool),
            points: d
                .points
                .iter()
                .map(|p| DrawingPointSnapshot {
                    price: data::Price::from_units(p.price_units).to_f64(),
                    time_secs: p.time / 1_000,
                })
                .collect(),
            label: d.label.clone(),
            visible: d.visible,
            locked: d.locked,
        })
        .collect()
}
