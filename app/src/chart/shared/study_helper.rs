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
        .any(|s| s.metadata().placement == study::StudyPlacement::Panel)
}

/// Result of a full study recompute cycle.
pub(crate) struct RecomputeResult {
    /// Diagnostics collected from all studies.
    pub diagnostics: Vec<(String, study::StudyDiagnostic)>,
    /// Whether any study's output changed (ready for cache skip optimization).
    #[allow(dead_code)]
    pub any_changed: bool,
}

/// Run a full recompute of all studies against the given input.
///
/// Returns diagnostics and a change flag that callers can use to skip
/// cache invalidation when nothing moved.
pub(crate) fn recompute_all(
    studies: &mut [Box<dyn study::Study>],
    input: &study::core::StudyInput<'_>,
) -> RecomputeResult {
    let mut diagnostics = Vec::new();
    let mut any_changed = false;

    for s in studies.iter_mut() {
        match s.compute(input) {
            Ok(result) => {
                if result.output_changed {
                    any_changed = true;
                }
                for d in result.diagnostics {
                    diagnostics.push((s.id().to_string(), d));
                }
            }
            Err(e) => {
                log::warn!("Study '{}' compute error: {}", s.id(), e);
                any_changed = true; // assume changed on error for safety
            }
        }
    }

    RecomputeResult {
        diagnostics,
        any_changed,
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
