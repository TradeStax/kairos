//! Big Trades study — institutional-scale execution detection.
//!
//! Reconstructs large executions by aggregating consecutive same-side fills
//! within a configurable time window. Each aggregated block produces a
//! marker at the VWAP-weighted price, sized proportionally to the total
//! contract count.
//!
//! ## Absorption detection (optional)
//!
//! When enabled, measures the deviation of realized price impact from
//! expected impact (λ × volume). Blocks with low actual/expected ratio
//! that see price return to the entry zone are flagged as absorption
//! and rendered as zone overlays.

mod absorption;
mod block;
mod params;

use crate::config::{ParameterTab, ParameterValue, StudyConfig};
use crate::core::{
    Study, StudyCapabilities, StudyCategory, StudyInput, StudyMetadata, StudyPlacement, StudyResult,
};
use crate::error::StudyError;
use crate::output::{
    MarkerData, MarkerRenderConfig, MarkerShape, PriceLevel, StudyOutput, TradeMarker,
};
use data::{ChartBasis, Price, Trade};

use absorption::AbsorptionDetector;
use block::{TradeBlock, build_candle_boundaries, flush_block};
use params::{
    AbsorptionParams, ComputeParams, DEFAULT_ABSORPTION_BUY_COLOR, DEFAULT_ABSORPTION_SELL_COLOR,
    DEFAULT_AGGREGATION_WINDOW_MS, DEFAULT_BUY_COLOR, DEFAULT_FILTER_MAX, DEFAULT_FILTER_MIN,
    DEFAULT_SELL_COLOR, DEFAULT_TEXT_COLOR,
};

/// What level of recomputation is needed on the next `compute()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum RecomputeLevel {
    /// No reprocessing — just rebuild output from existing state.
    None,
    /// Absorption params changed — replay blocks through absorption
    /// detector without reprocessing markers.
    AbsorptionOnly,
    /// Marker-affecting params changed — full trade reprocessing.
    Full,
}

/// Detects institutional-scale executions by aggregating consecutive
/// same-side fills within a time window and rendering them as sized
/// markers on the chart.
///
/// When absorption detection is enabled, blocks that show low price
/// impact relative to their volume produce zone overlays via
/// `StudyOutput::Composite(Markers + Levels)`.
pub struct BigTradesStudy {
    config: StudyConfig,
    output: StudyOutput,
    params: Vec<crate::config::ParameterDef>,
    processed_trade_count: usize,
    pending_block: Option<TradeBlock>,
    accumulated_markers: Vec<TradeMarker>,
    cached_render_config: MarkerRenderConfig,
    cached_candle_boundaries: Option<Vec<(u64, u64)>>,
    cached_boundaries_candle_count: usize,
    metadata: StudyMetadata,
    /// Absorption detection engine (always present, active when enabled).
    absorption: AbsorptionDetector,
    /// What kind of recompute the next `compute()` call needs.
    recompute_level: RecomputeLevel,
    /// Tick size from the last `compute()` call, used in `clone_study()`.
    last_tick_size_units: i64,
    /// Running max contracts across accumulated markers, avoids iterating
    /// all markers in `build_marker_render_config()`.
    observed_max_contracts: f64,
}

impl BigTradesStudy {
    pub fn new() -> Self {
        let params = params::build_parameter_defs();

        let mut config = StudyConfig::new("big_trades");
        for p in &params {
            config.set(p.key.clone(), p.default.clone());
        }

        let absorption_params = AbsorptionParams {
            enabled: false,
            lambda_window: 50,
            lambda_smooth: 20,
            score_threshold: 0.25,
            volume_k: 2.0,
            confirm_window_ms: 20000,
            buy_zone_color: DEFAULT_ABSORPTION_BUY_COLOR,
            sell_zone_color: DEFAULT_ABSORPTION_SELL_COLOR,
            zone_opacity: 0.30,
            show_zone_labels: true,
        };

        let mut study = Self {
            config,
            output: StudyOutput::Empty,
            params,
            processed_trade_count: 0,
            pending_block: None,
            accumulated_markers: Vec::new(),
            cached_render_config: MarkerRenderConfig::default(),
            cached_candle_boundaries: None,
            cached_boundaries_candle_count: 0,
            metadata: StudyMetadata {
                name: "Big Trades".into(),
                category: StudyCategory::OrderFlow,
                placement: StudyPlacement::Overlay,
                description: "Aggregated institutional-scale trade bubbles".into(),
                config_version: 1,
                capabilities: StudyCapabilities {
                    incremental: true,
                    needs_trades: true,
                    ..StudyCapabilities::default()
                },
            },
            absorption: AbsorptionDetector::new(Price::from_f32(0.25).units(), &absorption_params),
            recompute_level: RecomputeLevel::Full,
            last_tick_size_units: Price::from_f32(0.25).units(),
            observed_max_contracts: 0.0,
        };
        study.cached_render_config = study.build_marker_render_config();
        study
    }

    fn read_params(&self) -> ComputeParams {
        ComputeParams {
            filter_min: self.config.get_int("filter_min", DEFAULT_FILTER_MIN) as f64,
            filter_max: self.config.get_int("filter_max", DEFAULT_FILTER_MAX) as f64,
            window_ms: self
                .config
                .get_int("aggregation_window_ms", DEFAULT_AGGREGATION_WINDOW_MS)
                as u64,
            buy_color: self.config.get_color("buy_color", DEFAULT_BUY_COLOR),
            sell_color: self.config.get_color("sell_color", DEFAULT_SELL_COLOR),
            show_text: self.config.get_bool("show_text", true),
            show_debug: self.config.get_bool("show_debug", false),
        }
    }

    fn read_absorption_params(&self) -> AbsorptionParams {
        AbsorptionParams {
            enabled: self.config.get_bool("absorption_enabled", false),
            lambda_window: self.config.get_int("absorption_lambda_window", 50) as usize,
            lambda_smooth: self.config.get_int("absorption_lambda_smooth", 20) as usize,
            score_threshold: self.config.get_float("absorption_score_threshold", 0.25),
            volume_k: self.config.get_float("absorption_volume_k", 2.0),
            confirm_window_ms: self.config.get_int("absorption_confirm_ms", 20000) as u64,
            buy_zone_color: self
                .config
                .get_color("absorption_buy_zone_color", DEFAULT_ABSORPTION_BUY_COLOR),
            sell_zone_color: self
                .config
                .get_color("absorption_sell_zone_color", DEFAULT_ABSORPTION_SELL_COLOR),
            zone_opacity: self.config.get_float("absorption_zone_opacity", 0.30) as f32,
            show_zone_labels: self.config.get_bool("absorption_show_labels", true),
        }
    }

    /// Core processing loop: aggregates trades into blocks and flushes
    /// big-trade markers. When absorption is enabled, also feeds blocks
    /// into the absorption detector and checks pending absorptions.
    ///
    /// # Performance
    ///
    /// - Candle lookup uses a linear scan (O(n+m) total) instead of
    ///   binary search per trade (O(n log m)), exploiting chronological
    ///   ordering of the trade array.
    /// - Absorption pending check is guarded by `has_pending()` to skip
    ///   the inner loop when there's nothing to check.
    fn process_trades(
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
        let (mut detector, abs_params) = match absorption {
            Some((d, p)) => (Some(d), Some(p)),
            None => (None, None),
        };

        // Linear scan index for O(1) amortised candle-open lookup.
        // Trades are chronologically ordered, so we only ever advance
        // this index forward.
        let mut candle_idx: usize = 0;

        for trade in trades {
            let qty = trade.quantity.value();
            if qty <= 0.0 {
                continue;
            }

            let price_units = trade.price.units();
            let time = trade.time.0;
            let is_buy = trade.side.is_buy();

            // O(amortised 1) candle lookup via forward linear scan
            let candle_open = if is_time_based && num_candles > 0 {
                while candle_idx + 1 < num_candles && candles[candle_idx + 1].time.0 <= time {
                    candle_idx += 1;
                }
                candles[candle_idx].time.0
            } else {
                0
            };

            // Check pending absorptions (skip if nothing pending)
            if let Some(det) = &mut detector
                && det.has_pending()
            {
                det.check_pending(price_units, time, candle_open);
            }

            if let Some(block) = pending {
                let same_candle = !is_time_based || candle_open == block.candle_open;

                if block.is_buy == is_buy
                    && time.saturating_sub(block.last_time) <= window_ms
                    && same_candle
                {
                    // Extend current block
                    block.vwap_numerator += price_units as f64 * qty;
                    block.total_qty += qty;
                    block.last_time = time;
                    block.fill_count += 1;
                    block.min_price_units = block.min_price_units.min(price_units);
                    block.max_price_units = block.max_price_units.max(price_units);
                } else {
                    // Flush current block
                    let marker = flush_block(block, params, candles, basis, candle_boundaries);
                    let x = marker.as_ref().map_or(0, |m| m.time);

                    // Feed to absorption estimator
                    if let Some(det) = &mut detector
                        && let Some(ap) = abs_params
                    {
                        det.on_block_flushed(block, ap, x);
                    }

                    if let Some(m) = marker {
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
    ///
    /// This is much cheaper than a full recompute because:
    /// - No marker allocation or label formatting
    /// - No `flush_block` binary search per block
    /// - `check_pending` runs per-block instead of per-trade
    fn replay_absorption_only(
        trades: &[Trade],
        basis: &ChartBasis,
        candles: &[data::Candle],
        window_ms: u64,
        detector: &mut AbsorptionDetector,
        abs_params: &AbsorptionParams,
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
                while candle_idx + 1 < num_candles
                    && candles[candle_idx + 1].time.0 <= time
                {
                    candle_idx += 1;
                }
                candles[candle_idx].time.0
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
                    // Flush block to absorption only (no markers)
                    detector.check_pending(
                        block.vwap_units(),
                        block.last_time,
                        block.mid_time(),
                    );
                    detector.on_block_flushed(block, abs_params, block.mid_time());
                    *block = TradeBlock::new(is_buy, price_units, qty, time, candle_open);
                }
            } else {
                pending = Some(TradeBlock::new(is_buy, price_units, qty, time, candle_open));
            }
        }

        // Feed final pending block
        if let Some(block) = &pending {
            detector.check_pending(block.vwap_units(), block.last_time, block.mid_time());
            detector.on_block_flushed(block, abs_params, block.mid_time());
        }
    }

    /// Build output from accumulated markers + optional pending marker
    /// + absorption zones.
    fn rebuild_output(
        accumulated: &[TradeMarker],
        pending_marker: Option<&TradeMarker>,
        render_config: &MarkerRenderConfig,
        absorption_levels: Option<Vec<PriceLevel>>,
    ) -> StudyOutput {
        let total = accumulated.len() + pending_marker.is_some() as usize;
        let has_zones = absorption_levels.as_ref().is_some_and(|l| !l.is_empty());

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
                let levels = absorption_levels.unwrap();
                StudyOutput::Composite(vec![m, StudyOutput::Levels(levels)])
            }
            (Some(m), false) => m,
            (None, true) => StudyOutput::Levels(absorption_levels.unwrap()),
            (None, false) => StudyOutput::Empty,
        }
    }

    /// Collect absorption levels if enabled, or None.
    #[inline]
    fn collect_absorption_levels(
        detector: &mut AbsorptionDetector,
        params: &AbsorptionParams,
    ) -> Option<Vec<PriceLevel>> {
        if !params.enabled {
            return None;
        }
        let levels = detector.price_levels(params);
        if levels.is_empty() {
            None
        } else {
            Some(levels.to_vec())
        }
    }

    pub fn build_marker_render_config(&self) -> MarkerRenderConfig {
        let shape_str = self.config.get_choice("marker_shape", "Circle");
        let shape = match shape_str {
            "Square" => MarkerShape::Square,
            "Text Only" => MarkerShape::TextOnly,
            _ => MarkerShape::Circle,
        };

        let filter_min = self.config.get_int("filter_min", DEFAULT_FILTER_MIN) as f64;
        let filter_max = self.config.get_int("filter_max", DEFAULT_FILTER_MAX) as f64;

        let scale_min = filter_min.max(1.0);
        let observed_max = self.observed_max_contracts;
        let scale_max = if filter_max > 0.0 {
            filter_max
        } else if observed_max > scale_min {
            observed_max
        } else {
            scale_min * 10.0
        };

        MarkerRenderConfig {
            shape,
            hollow: self.config.get_bool("hollow", false),
            scale_min,
            scale_max,
            min_size: self.config.get_float("min_size", 8.0) as f32,
            max_size: self.config.get_float("max_size", 36.0) as f32,
            min_opacity: self.config.get_float("min_opacity", 0.10) as f32,
            max_opacity: self.config.get_float("max_opacity", 0.60) as f32,
            show_text: self.config.get_bool("show_text", true),
            text_size: self.config.get_float("text_size", 10.0) as f32,
            text_color: self.config.get_color("text_color", DEFAULT_TEXT_COLOR),
        }
    }
}

impl Default for BigTradesStudy {
    fn default() -> Self {
        Self::new()
    }
}

impl Study for BigTradesStudy {
    fn id(&self) -> &str {
        "big_trades"
    }

    fn metadata(&self) -> &StudyMetadata {
        &self.metadata
    }

    fn tab_labels(&self) -> Option<&[(&'static str, ParameterTab)]> {
        static LABELS: &[(&str, ParameterTab)] = &[
            ("Data", ParameterTab::Parameters),
            ("Style", ParameterTab::Style),
            ("Display", ParameterTab::Display),
            ("Absorption", ParameterTab::Absorption),
        ];
        Some(LABELS)
    }

    fn parameters(&self) -> &[crate::config::ParameterDef] {
        &self.params
    }

    fn config(&self) -> &StudyConfig {
        &self.config
    }

    fn config_mut(&mut self) -> &mut StudyConfig {
        &mut self.config
    }

    fn set_parameter(&mut self, key: &str, value: ParameterValue) -> Result<(), StudyError> {
        let params = self.parameters();
        let def =
            params
                .iter()
                .find(|p| p.key == key)
                .ok_or_else(|| StudyError::InvalidParameter {
                    key: key.to_string(),
                    reason: "unknown parameter".to_string(),
                })?;
        def.validate(&value)
            .map_err(|reason| StudyError::InvalidParameter {
                key: key.to_string(),
                reason,
            })?;
        self.config_mut().set(key, value);
        self.cached_render_config = self.build_marker_render_config();

        // Classify the parameter change to determine the minimum
        // recompute level needed. Uses max() so a higher-priority
        // level isn't downgraded by a subsequent lower-priority change.
        const MARKER_KEYS: &[&str] = &[
            "filter_min",
            "filter_max",
            "aggregation_window_ms",
        ];
        const ABSORPTION_KEYS: &[&str] = &[
            "absorption_enabled",
            "absorption_lambda_window",
            "absorption_lambda_smooth",
            "absorption_score_threshold",
            "absorption_volume_k",
            "absorption_confirm_ms",
        ];
        if MARKER_KEYS.contains(&key) {
            self.recompute_level = self.recompute_level.max(RecomputeLevel::Full);
        } else if ABSORPTION_KEYS.contains(&key) {
            self.recompute_level = self.recompute_level.max(RecomputeLevel::AbsorptionOnly);
        }

        Ok(())
    }

    fn compute(&mut self, input: &StudyInput) -> Result<StudyResult, StudyError> {
        let trades = match input.trades {
            Some(t) if !t.is_empty() => t,
            _ => {
                self.output = StudyOutput::Empty;
                self.processed_trade_count = 0;
                self.pending_block = None;
                self.accumulated_markers.clear();
                self.observed_max_contracts = 0.0;
                self.cached_candle_boundaries = None;
                self.cached_boundaries_candle_count = 0;
                self.absorption.reset();
                return Ok(StudyResult::unchanged());
            }
        };

        self.last_tick_size_units = input.tick_size.units();

        let recompute = self.recompute_level;
        let already_processed = self.processed_trade_count > 0
            && trades.len() == self.processed_trade_count;

        // Fast path: style-only change with same trades — just rebuild output.
        if recompute == RecomputeLevel::None && already_processed {
            let params = self.read_params();
            let abs_params = self.read_absorption_params();
            self.cached_render_config = self.build_marker_render_config();

            let pending_marker = self.pending_block.as_ref().and_then(|block| {
                flush_block(
                    block,
                    &params,
                    input.candles,
                    &input.basis,
                    self.cached_candle_boundaries.as_deref(),
                )
            });

            let absorption_levels =
                Self::collect_absorption_levels(&mut self.absorption, &abs_params);

            self.output = Self::rebuild_output(
                &self.accumulated_markers,
                pending_marker.as_ref(),
                &self.cached_render_config,
                absorption_levels,
            );
            return Ok(StudyResult::ok());
        }

        // Medium path: absorption param changed but markers are intact.
        // Replay blocks through absorption without rebuilding markers.
        if recompute == RecomputeLevel::AbsorptionOnly && already_processed {
            self.recompute_level = RecomputeLevel::None;
            let params = self.read_params();
            let abs_params = self.read_absorption_params();

            // Reset and replay absorption only
            self.absorption
                .reset_with_tick_size(input.tick_size.units(), &abs_params);

            if abs_params.enabled {
                Self::replay_absorption_only(
                    trades,
                    &input.basis,
                    input.candles,
                    params.window_ms,
                    &mut self.absorption,
                    &abs_params,
                );
            }

            self.cached_render_config = self.build_marker_render_config();
            let pending_marker = self.pending_block.as_ref().and_then(|block| {
                flush_block(
                    block,
                    &params,
                    input.candles,
                    &input.basis,
                    self.cached_candle_boundaries.as_deref(),
                )
            });
            let absorption_levels =
                Self::collect_absorption_levels(&mut self.absorption, &abs_params);

            self.output = Self::rebuild_output(
                &self.accumulated_markers,
                pending_marker.as_ref(),
                &self.cached_render_config,
                absorption_levels,
            );
            return Ok(StudyResult::ok());
        }

        self.recompute_level = RecomputeLevel::None;

        let params = self.read_params();
        let abs_params = self.read_absorption_params();

        // Reuse candle boundaries if candle count unchanged
        if input.candles.len() != self.cached_boundaries_candle_count {
            self.cached_candle_boundaries =
                build_candle_boundaries(input.candles, &input.basis);
            self.cached_boundaries_candle_count = input.candles.len();
        }

        // Reset detector with correct tick size, reusing allocations
        self.absorption
            .reset_with_tick_size(input.tick_size.units(), &abs_params);

        // Reuse accumulated_markers allocation
        self.accumulated_markers.clear();
        self.observed_max_contracts = 0.0;
        self.pending_block = None;

        let absorption_arg = if abs_params.enabled {
            Some((&mut self.absorption, &abs_params))
        } else {
            None
        };

        BigTradesStudy::process_trades(
            trades,
            &mut self.pending_block,
            &mut self.accumulated_markers,
            &params,
            input.candles,
            &input.basis,
            self.cached_candle_boundaries.as_deref(),
            absorption_arg,
        );

        // Update observed max from newly added markers
        for m in &self.accumulated_markers {
            if m.contracts > self.observed_max_contracts {
                self.observed_max_contracts = m.contracts;
            }
        }

        // Feed the final pending block to absorption (it won't be
        // flushed inside process_trades since there's no next trade).
        if abs_params.enabled
            && let Some(block) = &self.pending_block
        {
            let marker = flush_block(
                block,
                &params,
                input.candles,
                &input.basis,
                self.cached_candle_boundaries.as_deref(),
            );
            let x = marker.as_ref().map_or(0, |m| m.time);
            self.absorption.on_block_flushed(block, &abs_params, x);
        }

        let pending_marker = self.pending_block.as_ref().and_then(|block| {
            flush_block(
                block,
                &params,
                input.candles,
                &input.basis,
                self.cached_candle_boundaries.as_deref(),
            )
        });

        self.processed_trade_count = trades.len();
        self.cached_render_config = self.build_marker_render_config();

        let absorption_levels = Self::collect_absorption_levels(&mut self.absorption, &abs_params);

        self.output = Self::rebuild_output(
            &self.accumulated_markers,
            pending_marker.as_ref(),
            &self.cached_render_config,
            absorption_levels,
        );
        Ok(StudyResult::ok())
    }

    fn append_trades(
        &mut self,
        _new_trades: &[Trade],
        input: &StudyInput,
    ) -> Result<StudyResult, StudyError> {
        let trades = match input.trades {
            Some(t) if !t.is_empty() => t,
            _ => return Ok(StudyResult::unchanged()),
        };

        if self.processed_trade_count == 0 {
            return self.compute(input);
        }

        if self.processed_trade_count >= trades.len() {
            return Ok(StudyResult::unchanged());
        }
        let new_slice = &trades[self.processed_trade_count..];

        let params = self.read_params();
        let abs_params = self.read_absorption_params();

        if input.candles.len() != self.cached_boundaries_candle_count {
            self.cached_candle_boundaries = build_candle_boundaries(input.candles, &input.basis);
            self.cached_boundaries_candle_count = input.candles.len();
        }

        let markers_before = self.accumulated_markers.len();

        let absorption_arg = if abs_params.enabled {
            Some((&mut self.absorption, &abs_params))
        } else {
            None
        };

        BigTradesStudy::process_trades(
            new_slice,
            &mut self.pending_block,
            &mut self.accumulated_markers,
            &params,
            input.candles,
            &input.basis,
            self.cached_candle_boundaries.as_deref(),
            absorption_arg,
        );

        self.processed_trade_count = trades.len();

        let markers_changed = self.accumulated_markers.len() != markers_before;

        // Update observed max for any new markers
        if markers_changed {
            for m in &self.accumulated_markers[markers_before..] {
                if m.contracts > self.observed_max_contracts {
                    self.observed_max_contracts = m.contracts;
                }
            }
        }

        let pending_marker = self.pending_block.as_ref().and_then(|block| {
            flush_block(
                block,
                &params,
                input.candles,
                &input.basis,
                self.cached_candle_boundaries.as_deref(),
            )
        });

        if markers_changed || pending_marker.is_some() {
            if markers_changed {
                self.cached_render_config = self.build_marker_render_config();
            }

            let absorption_levels = Self::collect_absorption_levels(&mut self.absorption, &abs_params);

            self.output = Self::rebuild_output(
                &self.accumulated_markers,
                pending_marker.as_ref(),
                &self.cached_render_config,
                absorption_levels,
            );
            return Ok(StudyResult::ok());
        }
        Ok(StudyResult::unchanged())
    }

    fn output(&self) -> &StudyOutput {
        &self.output
    }

    fn reset(&mut self) {
        self.output = StudyOutput::Empty;
        self.processed_trade_count = 0;
        self.pending_block = None;
        self.accumulated_markers.clear();
        self.observed_max_contracts = 0.0;
        self.cached_candle_boundaries = None;
        self.cached_boundaries_candle_count = 0;
        self.absorption.reset();
        self.recompute_level = RecomputeLevel::Full;
    }

    fn clone_study(&self) -> Box<dyn Study> {
        let abs_params = self.read_absorption_params();
        Box::new(Self {
            config: self.config.clone(),
            output: self.output.clone(),
            params: self.params.clone(),
            processed_trade_count: 0,
            pending_block: None,
            accumulated_markers: Vec::new(),
            cached_render_config: self.cached_render_config,
            cached_candle_boundaries: None,
            cached_boundaries_candle_count: 0,
            metadata: self.metadata.clone(),
            absorption: AbsorptionDetector::new(self.last_tick_size_units, &abs_params),
            recompute_level: RecomputeLevel::Full,
            last_tick_size_units: self.last_tick_size_units,
            observed_max_contracts: 0.0,
        })
    }
}

#[cfg(test)]
mod tests;
