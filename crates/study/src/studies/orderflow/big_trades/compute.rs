//! Core computation routines for the Big Trades study.
//!
//! Extracted from `mod.rs` to keep the Study trait implementation
//! focused on orchestration while heavy processing lives here.

use crate::output::{MarkerRenderConfig, StudyOutput, TradeMarker, ZoneRect};
use data::{ChartBasis, Trade};

use super::BigTradesStudy;
use super::absorption::AbsorptionDetector;
use super::block::{TradeBlock, block_x, flush_block};
use super::params::{AbsorptionParams, ComputeParams};

impl BigTradesStudy {
    /// Core processing loop: aggregates trades into blocks and flushes
    /// big-trade markers. When absorption is enabled, also registers
    /// candidates and finalizes them on candle boundary crossings.
    pub(super) fn process_trades(
        trades: &[Trade],
        pending: &mut Option<TradeBlock>,
        markers: &mut Vec<TradeMarker>,
        params: &ComputeParams,
        candles: &[data::Candle],
        basis: &ChartBasis,
        candle_boundaries: Option<&[(u64, u64)]>,
        absorption: Option<(&mut AbsorptionDetector, &AbsorptionParams)>,
    ) {
        let is_time_based = matches!(basis, ChartBasis::Time(_));
        let num_candles = candles.len();
        let window_ms = params.window_ms;
        let (mut detector, _abs_params) = match absorption {
            Some((d, p)) => (Some(d), Some(p)),
            None => (None, None),
        };

        let mut candle_idx: usize = 0;

        for trade in trades {
            let qty = trade.quantity.value();
            if qty <= 0.0 {
                continue;
            }

            let price_units = trade.price.units();
            let time = trade.time.0;
            let is_buy = trade.side.is_buy();

            // O(amortised 1) candle lookup via forward linear scan.
            // Also track candle_idx for absorption finalization.
            let candle_open = if is_time_based && num_candles > 0 {
                let prev_idx = candle_idx;
                while candle_idx + 1 < num_candles && candles[candle_idx + 1].time.0 <= time {
                    candle_idx += 1;
                }
                if candle_idx > prev_idx
                    && let Some(det) = &mut detector
                {
                    det.try_finalize(candle_idx, candles);
                }
                candles[candle_idx].time.0
            } else if !is_time_based && num_candles > 0 {
                let prev_idx = candle_idx;
                while candle_idx + 1 < num_candles && candles[candle_idx + 1].time.0 <= time {
                    candle_idx += 1;
                }
                if candle_idx > prev_idx
                    && let Some(det) = &mut detector
                {
                    det.try_finalize(candle_idx, candles);
                }
                0
            } else {
                0
            };

            if let Some(block) = pending {
                let same_candle = !is_time_based || candle_open == block.candle_open;

                if block.is_buy == is_buy
                    && time.saturating_sub(block.last_time) <= window_ms
                    && same_candle
                {
                    block.vwap_numerator += price_units as f64 * qty;
                    block.total_qty += qty;
                    block.last_time = time;
                    block.fill_count += 1;
                    block.min_price_units = block.min_price_units.min(price_units);
                    block.max_price_units = block.max_price_units.max(price_units);
                } else {
                    // Flush current block — register as absorption candidate
                    // if it passes filters
                    let marker = flush_block(block, params, candles, basis, candle_boundaries);
                    if let Some(m) = marker {
                        if let Some(det) = &mut detector {
                            let x = block_x(block, candles, basis, candle_boundaries);
                            det.register_candidate(block, candle_idx, x);
                        }
                        markers.push(m);
                    }

                    *pending = Some(TradeBlock::new(is_buy, price_units, qty, time, candle_open));
                }
            } else {
                *pending = Some(TradeBlock::new(is_buy, price_units, qty, time, candle_open));
            }
        }
    }

    /// Replay trade blocks through the absorption detector without
    /// rebuilding markers. Used when only absorption params changed.
    pub(super) fn replay_absorption_only(
        trades: &[Trade],
        basis: &ChartBasis,
        candles: &[data::Candle],
        window_ms: u64,
        compute_params: &ComputeParams,
        candle_boundaries: Option<&[(u64, u64)]>,
        detector: &mut AbsorptionDetector,
        _abs_params: &AbsorptionParams,
    ) {
        let is_time_based = matches!(basis, ChartBasis::Time(_));
        let num_candles = candles.len();
        let mut candle_idx: usize = 0;
        let mut pending: Option<TradeBlock> = None;

        for trade in trades {
            let qty = trade.quantity.value();
            if qty <= 0.0 {
                continue;
            }

            let price_units = trade.price.units();
            let time = trade.time.0;
            let is_buy = trade.side.is_buy();

            let candle_open = if is_time_based && num_candles > 0 {
                let prev_idx = candle_idx;
                while candle_idx + 1 < num_candles && candles[candle_idx + 1].time.0 <= time {
                    candle_idx += 1;
                }
                if candle_idx > prev_idx {
                    detector.try_finalize(candle_idx, candles);
                }
                candles[candle_idx].time.0
            } else if !is_time_based && num_candles > 0 {
                let prev_idx = candle_idx;
                while candle_idx + 1 < num_candles && candles[candle_idx + 1].time.0 <= time {
                    candle_idx += 1;
                }
                if candle_idx > prev_idx {
                    detector.try_finalize(candle_idx, candles);
                }
                0
            } else {
                0
            };

            if let Some(block) = &mut pending {
                let same_candle = !is_time_based || candle_open == block.candle_open;

                if block.is_buy == is_buy
                    && time.saturating_sub(block.last_time) <= window_ms
                    && same_candle
                {
                    block.vwap_numerator += price_units as f64 * qty;
                    block.total_qty += qty;
                    block.last_time = time;
                    block.fill_count += 1;
                    block.min_price_units = block.min_price_units.min(price_units);
                    block.max_price_units = block.max_price_units.max(price_units);
                } else {
                    let marker =
                        flush_block(block, compute_params, candles, basis, candle_boundaries);
                    if marker.is_some() {
                        let x = block_x(block, candles, basis, candle_boundaries);
                        detector.register_candidate(block, candle_idx, x);
                    }
                    *block = TradeBlock::new(is_buy, price_units, qty, time, candle_open);
                }
            } else {
                pending = Some(TradeBlock::new(is_buy, price_units, qty, time, candle_open));
            }
        }

        // Feed final pending block
        if let Some(block) = &pending {
            let marker = flush_block(block, compute_params, candles, basis, candle_boundaries);
            if marker.is_some() {
                let x = block_x(block, candles, basis, candle_boundaries);
                detector.register_candidate(block, candle_idx, x);
            }
        }

        detector.finalize_all(candles);
    }

    /// Build output from accumulated markers + optional pending marker
    /// + absorption zones.
    pub(super) fn rebuild_output(
        accumulated: &[TradeMarker],
        pending_marker: Option<&TradeMarker>,
        render_config: &MarkerRenderConfig,
        absorption_levels: Option<Vec<ZoneRect>>,
    ) -> StudyOutput {
        let total = accumulated.len() + pending_marker.is_some() as usize;
        let has_zones = absorption_levels.as_ref().is_some_and(|z| !z.is_empty());

        if total == 0 && !has_zones {
            return StudyOutput::Empty;
        }

        let markers_output = if total > 0 {
            let mut markers = Vec::with_capacity(total);
            markers.extend_from_slice(accumulated);
            if let Some(pm) = pending_marker {
                markers.push(pm.clone());
            }
            Some(StudyOutput::Markers(MarkerData {
                markers,
                render_config: *render_config,
            }))
        } else {
            None
        };

        match (markers_output, has_zones) {
            (Some(m), true) => {
                let zones = absorption_levels.unwrap();
                StudyOutput::Composite(vec![m, StudyOutput::Zones(zones)])
            }
            (Some(m), false) => m,
            (None, true) => StudyOutput::Zones(absorption_levels.unwrap()),
            (None, false) => StudyOutput::Empty,
        }
    }

    /// Collect absorption zones if enabled, or None.
    #[inline]
    pub(super) fn collect_absorption_zones(
        detector: &mut AbsorptionDetector,
        params: &AbsorptionParams,
        visible_range: Option<(u64, u64)>,
    ) -> Option<Vec<ZoneRect>> {
        if !params.enabled {
            return None;
        }
        let zones = detector.zone_rects(params, visible_range);
        if zones.is_empty() {
            None
        } else {
            Some(zones.to_vec())
        }
    }
}

use crate::output::MarkerData;
