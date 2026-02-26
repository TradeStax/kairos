//! Shared study management helpers used by KlineChart and ProfileChart.
//!
//! These free functions centralize the common patterns for adding, removing,
//! and updating studies so that each chart type's `studies.rs` is a thin wrapper.

use data::FuturesTickerInfo;
use data::Price as DomainPrice;

/// Compute a `StudyInput` from chart data fields common to all chart types.
///
/// `visible_range` may be `None` when the chart has not yet rendered a frame.
pub(crate) fn build_study_input<'a>(
    candles: &'a [data::Candle],
    trades: &'a [data::Trade],
    basis: data::ChartBasis,
    ticker_info: &FuturesTickerInfo,
    visible_range: Option<(u64, u64)>,
) -> study::core::StudyInput<'a> {
    study::core::StudyInput {
        candles,
        trades: Some(trades),
        basis,
        tick_size: DomainPrice::from_f32(ticker_info.tick_size),
        visible_range,
    }
}

/// Remove the study with the given ID and return whether any panel studies remain.
///
/// Mutates `studies` in place.  Returns `true` if at least one panel study remains
/// after removal (used by callers to decide whether to clear the splits vector).
pub(crate) fn remove_study_by_id(studies: &mut Vec<Box<dyn study::Study>>, id: &str) -> bool {
    studies.retain(|s| s.id() != id);
    studies
        .iter()
        .any(|s| s.placement() == study::StudyPlacement::Panel)
}

/// Run a full recompute of all studies against the given input.
pub(crate) fn recompute_all(
    studies: &mut [Box<dyn study::Study>],
    input: &study::core::StudyInput<'_>,
) {
    for s in studies.iter_mut() {
        if let Err(e) = s.compute(input) {
            log::warn!("Study '{}' compute error: {}", s.id(), e);
        }
    }
}

/// Incrementally update all studies with newly appended trades.
pub(crate) fn append_trades_to_studies(
    studies: &mut [Box<dyn study::Study>],
    trades: &[data::Trade],
    input: &study::core::StudyInput<'_>,
) {
    for s in studies.iter_mut() {
        if let Err(e) = s.append_trades(trades, input) {
            log::warn!("Study '{}' append error: {}", s.id(), e);
        }
    }
}
