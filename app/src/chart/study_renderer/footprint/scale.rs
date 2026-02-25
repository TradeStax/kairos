//! Footprint scaling, grouping, and value formatting utilities.

use crate::chart::ViewState;
use study::output::{
    FootprintCandle, FootprintData, FootprintDataType,
    FootprintLevel, FootprintScaling, TextFormat,
};
use data::ChartBasis;

/// Minimum row height in screen pixels for readable text.
pub(super) const MIN_ROW_PX: f32 = 16.0;

/// Compute the dynamic grouping quantum for automatic mode.
///
/// `factor` is the user's scale factor; larger values produce
/// coarser grouping.
pub(super) fn compute_dynamic_quantum(
    state: &ViewState,
    factor: i64,
    tick_units: i64,
) -> i64 {
    super::super::coord::compute_dynamic_quantum(
        state, MIN_ROW_PX, factor, tick_units,
    )
}

/// Merge footprint levels to a coarser quantum boundary.
///
/// Returns the merged level vec and the new POC index.
pub(super) fn merge_levels_to_quantum(
    levels: &[FootprintLevel],
    target_quantum: i64,
) -> (Vec<FootprintLevel>, Option<usize>) {
    use std::collections::BTreeMap;

    let mut merged: BTreeMap<i64, (f32, f32)> = BTreeMap::new();
    for level in levels {
        let rounded =
            (level.price / target_quantum) * target_quantum;
        let entry = merged.entry(rounded).or_insert((0.0, 0.0));
        entry.0 += level.buy_volume;
        entry.1 += level.sell_volume;
    }

    let result: Vec<FootprintLevel> = merged
        .into_iter()
        .map(|(price, (buy, sell))| FootprintLevel {
            price,
            buy_volume: buy,
            sell_volume: sell,
        })
        .collect();

    let poc_index = result
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| {
            a.total_qty()
                .partial_cmp(&b.total_qty())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(i, _)| i);

    (result, poc_index)
}

/// Format a footprint level value according to the configured format.
/// Footprint values are always whole contract counts, so we avoid
/// trailing `.0` decimals that `abbr_large_numbers` would produce.
pub(super) fn format_value(
    value: f32,
    format: TextFormat,
) -> String {
    match format {
        TextFormat::Automatic => {
            let abs = value.abs();
            let sign = if value < 0.0 { "-" } else { "" };
            if abs >= 1_000_000.0 {
                format!("{}{:.1}m", sign, abs / 1_000_000.0)
            } else if abs >= 10_000.0 {
                format!("{}{:.1}k", sign, abs / 1_000.0)
            } else if abs >= 1_000.0 {
                format!("{}{:.1}k", sign, abs / 1_000.0)
            } else {
                format!("{}{:.0}", sign, abs)
            }
        }
        TextFormat::Normal => format!("{:.0}", value),
        TextFormat::K => {
            if value.abs() >= 1000.0 {
                format!("{:.1}K", value / 1000.0)
            } else {
                format!("{:.0}", value)
            }
        }
    }
}

pub(super) fn effective_cluster_qty(
    scaling: FootprintScaling,
    visible_max: f32,
    levels: &[FootprintLevel],
    data_type: FootprintDataType,
) -> f32 {
    let individual_max = match data_type {
        FootprintDataType::BidAskSplit
        | FootprintDataType::DeltaAndVolume => levels
            .iter()
            .map(|l| l.buy_volume.max(l.sell_volume))
            .fold(0.0_f32, f32::max),
        FootprintDataType::Delta => levels
            .iter()
            .map(|l| l.delta_qty().abs())
            .fold(0.0_f32, f32::max),
        FootprintDataType::Volume => levels
            .iter()
            .map(|l| l.total_qty())
            .fold(0.0_f32, f32::max),
    };

    let safe = |v: f32| if v <= f32::EPSILON { 1.0 } else { v };

    match scaling {
        FootprintScaling::VisibleRange => safe(visible_max),
        FootprintScaling::Datapoint => safe(individual_max),
        FootprintScaling::Hybrid { weight } => {
            let w = weight.clamp(0.0, 1.0);
            safe(visible_max * w + individual_max * (1.0 - w))
        }
        FootprintScaling::Linear
        | FootprintScaling::Sqrt
        | FootprintScaling::Log => safe(visible_max),
    }
}

#[inline]
pub(super) fn scaled_ratio(
    qty: f32,
    max: f32,
    scaling: FootprintScaling,
) -> f32 {
    if max <= f32::EPSILON || qty <= f32::EPSILON {
        return 0.0;
    }
    match scaling {
        FootprintScaling::Sqrt => qty.sqrt() / max.sqrt(),
        FootprintScaling::Log => {
            (1.0 + qty).ln() / (1.0 + max).ln()
        }
        _ => qty / max,
    }
}

pub(super) fn calc_visible_max(
    data: &FootprintData,
    earliest: u64,
    latest: u64,
    basis: &ChartBasis,
    dynamic_quantum: Option<i64>,
) -> f32 {
    let candles_iter: Box<dyn Iterator<Item = &FootprintCandle>> =
        match basis {
            ChartBasis::Time(_) => Box::new(
                data.candles
                    .iter()
                    .filter(move |c| {
                        c.x >= earliest && c.x <= latest
                    }),
            ),
            ChartBasis::Tick(_) => {
                let ea = earliest as usize;
                let la = latest as usize;
                Box::new(
                    data.candles
                        .iter()
                        .rev()
                        .enumerate()
                        .filter(move |(i, _)| {
                            *i >= ea && *i <= la
                        })
                        .map(|(_, c)| c),
                )
            }
        };

    let level_max = |levels: &[FootprintLevel]| -> f32 {
        levels
            .iter()
            .map(|l| match data.data_type {
                FootprintDataType::Volume => l.total_qty(),
                FootprintDataType::BidAskSplit
                | FootprintDataType::DeltaAndVolume => {
                    l.buy_volume.max(l.sell_volume)
                }
                FootprintDataType::Delta => l.delta_qty().abs(),
            })
            .fold(0.0_f32, f32::max)
    };

    candles_iter
        .map(|c| {
            match dynamic_quantum {
                Some(q) if q > c.quantum => {
                    let merged =
                        merge_levels_to_quantum(&c.levels, q);
                    level_max(&merged.0)
                }
                _ => level_max(&c.levels),
            }
        })
        .fold(0.0_f32, f32::max)
}
