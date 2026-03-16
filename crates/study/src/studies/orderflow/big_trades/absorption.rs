//! Absorption detection engine for the Big Trades study.
//!
//! Identifies when large aggressive flow is absorbed by passive liquidity
//! without proportional price impact. Based on Kyle (1985) price impact
//! theory: realized impact significantly below expected impact (λ × volume)
//! signals absorption.
//!
//! The pipeline has four phases:
//! 1. **Lambda estimation** — rolling EMA of ticks-per-contract impact
//! 2. **Absorption detection** — score = actual/expected ticks moved
//! 3. **Confirmation** — price rejection within a time window
//! 4. **Zone output** — confirmed zones as `PriceLevel` primitives

use std::collections::VecDeque;

use crate::config::LineStyleValue;
use crate::output::PriceLevel;
use crate::util::math::ema_multiplier;
use data::Price;

use super::block::TradeBlock;
use super::params::AbsorptionParams;

/// Hard cap on pending absorptions to prevent unbounded growth.
const MAX_PENDING: usize = 64;
/// Hard cap on confirmed zones per compute pass.
const MAX_ZONES: usize = 100;

/// Record of a flushed block's volume for threshold estimation.
struct ImpactRecord {
    volume: f64,
}

/// Rolling price impact coefficient estimator.
///
/// Maintains an EMA-smoothed λ (ticks per contract) and running
/// volume statistics for the adaptive volume threshold.
struct ImpactEstimator {
    history: VecDeque<ImpactRecord>,
    max_history: usize,
    lambda_ema: f64,
    alpha: f64,
    volume_sum: f64,
    volume_sum_sq: f64,
    volume_count: usize,
    /// Counter tracking evictions since last full recompute of sums.
    /// When it reaches `max_history`, sums are recomputed from scratch
    /// to eliminate floating-point drift from incremental subtraction.
    eviction_counter: usize,
}

impl ImpactEstimator {
    fn new(max_history: usize, smooth_period: usize) -> Self {
        Self {
            history: VecDeque::with_capacity(max_history.min(256)),
            max_history,
            lambda_ema: 0.0,
            alpha: ema_multiplier(smooth_period.max(1)),
            volume_sum: 0.0,
            volume_sum_sq: 0.0,
            volume_count: 0,
            eviction_counter: 0,
        }
    }

    /// Reset all state, reusing existing allocation.
    fn clear(&mut self, max_history: usize, smooth_period: usize) {
        self.history.clear();
        self.max_history = max_history;
        self.lambda_ema = 0.0;
        self.alpha = ema_multiplier(smooth_period.max(1));
        self.volume_sum = 0.0;
        self.volume_sum_sq = 0.0;
        self.volume_count = 0;
        self.eviction_counter = 0;
    }

    /// Feed a flushed block's impact data into the estimator.
    fn record(&mut self, block: &TradeBlock, tick_size_units: i64) {
        if block.total_qty <= 0.0 || tick_size_units <= 0 {
            return;
        }

        let ticks_moved = (block.vwap_units() - block.first_price_units).unsigned_abs() as f64
            / tick_size_units as f64;
        let impact_per_contract = ticks_moved / block.total_qty;

        // Ring buffer eviction
        if self.history.len() >= self.max_history
            && let Some(old) = self.history.pop_front()
        {
            self.volume_sum -= old.volume;
            self.volume_sum_sq -= old.volume * old.volume;
            self.volume_count = self.volume_count.saturating_sub(1);
            self.eviction_counter += 1;

            // Periodic recompute to eliminate floating-point drift.
            // O(max_history) amortized over max_history evictions = O(1) per call.
            if self.eviction_counter >= self.max_history {
                self.recompute_sums();
                self.eviction_counter = 0;
            }
        }

        self.history.push_back(ImpactRecord {
            volume: block.total_qty,
        });

        self.volume_sum += block.total_qty;
        self.volume_sum_sq += block.total_qty * block.total_qty;
        self.volume_count += 1;

        // EMA update for lambda
        if self.lambda_ema == 0.0 {
            self.lambda_ema = impact_per_contract;
        } else {
            self.lambda_ema =
                self.alpha * impact_per_contract + (1.0 - self.alpha) * self.lambda_ema;
        }
    }

    #[inline]
    fn lambda(&self) -> f64 {
        self.lambda_ema
    }

    #[inline]
    fn sample_count(&self) -> usize {
        self.volume_count
    }

    /// Adaptive volume threshold: mean + k × std_dev.
    fn volume_threshold(&self, k: f64) -> f64 {
        if self.volume_count < 2 {
            return f64::MAX;
        }
        let n = self.volume_count as f64;
        let mean = self.volume_sum / n;
        let variance = (self.volume_sum_sq / n - mean * mean).max(0.0);
        mean + k * variance.sqrt()
    }

    /// Recompute running sums from scratch to eliminate floating-point drift.
    fn recompute_sums(&mut self) {
        self.volume_sum = self.history.iter().map(|r| r.volume).sum();
        self.volume_sum_sq = self.history.iter().map(|r| r.volume * r.volume).sum();
        self.volume_count = self.history.len();
    }
}

/// Pending absorption waiting for price rejection confirmation.
struct PendingAbsorption {
    is_buy: bool,
    volume: f64,
    expected_ticks: f64,
    score: f64,
    confirmation_deadline: u64,
    zone_center_units: i64,
    zone_min_units: i64,
    zone_max_units: i64,
    start_x: u64,
}

/// Confirmed absorption zone for rendering.
pub(super) struct AbsorptionZone {
    pub center_price_units: i64,
    pub zone_half_width_units: i64,
    pub start_x: u64,
    pub end_x: u64,
    pub is_buy: bool,
    pub strength: f32,
    pub volume: f64,
}

/// Top-level absorption detection engine.
pub(super) struct AbsorptionDetector {
    estimator: ImpactEstimator,
    pending: Vec<PendingAbsorption>,
    confirmed_zones: Vec<AbsorptionZone>,
    tick_size_units: i64,
    min_samples: usize,
    /// Cached price levels, rebuilt only when `confirmed_zones` changes.
    cached_levels: Vec<PriceLevel>,
    cached_levels_zone_count: usize,
}

impl AbsorptionDetector {
    pub fn new(tick_size_units: i64, params: &AbsorptionParams) -> Self {
        Self {
            estimator: ImpactEstimator::new(params.lambda_window, params.lambda_smooth),
            pending: Vec::new(),
            confirmed_zones: Vec::new(),
            tick_size_units: tick_size_units.max(1),
            min_samples: 5,
            cached_levels: Vec::new(),
            cached_levels_zone_count: 0,
        }
    }

    /// Reset all state while reusing existing heap allocations.
    pub fn reset(&mut self) {
        let period = ((2.0 / self.estimator.alpha) - 1.0).round().max(1.0) as usize;
        self.estimator.clear(self.estimator.max_history, period);
        self.pending.clear();
        self.confirmed_zones.clear();
        self.cached_levels.clear();
        self.cached_levels_zone_count = 0;
    }

    /// Reset all state with a new tick size, reusing allocations.
    pub fn reset_with_tick_size(&mut self, tick_size_units: i64, params: &AbsorptionParams) {
        self.tick_size_units = tick_size_units.max(1);
        self.estimator
            .clear(params.lambda_window, params.lambda_smooth);
        self.pending.clear();
        self.confirmed_zones.clear();
        self.cached_levels.clear();
        self.cached_levels_zone_count = 0;
    }

    /// Whether there are pending absorptions that need checking.
    #[inline]
    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    /// Process a flushed trade block.
    ///
    /// Phase 1: updates lambda estimator (always).
    /// Phase 2: checks for absorption signal if block is large enough.
    pub fn on_block_flushed(
        &mut self,
        block: &TradeBlock,
        params: &AbsorptionParams,
        start_x: u64,
    ) {
        // Phase 1: always update lambda
        self.estimator.record(block, self.tick_size_units);

        // Phase 2: check for absorption
        if self.estimator.sample_count() < self.min_samples {
            return;
        }
        let lambda = self.estimator.lambda();
        if lambda <= 0.0 {
            return;
        }

        let vol_threshold = self.estimator.volume_threshold(params.volume_k);
        if block.total_qty < vol_threshold {
            return;
        }

        // Don't exceed pending capacity
        if self.pending.len() >= MAX_PENDING {
            return;
        }

        let tick_f = self.tick_size_units as f64;
        let actual_ticks =
            (block.vwap_units() - block.first_price_units).unsigned_abs() as f64 / tick_f;
        let expected_ticks = block.total_qty * lambda;

        let score = if expected_ticks > 0.0 {
            (actual_ticks / expected_ticks).min(2.0)
        } else {
            2.0
        };

        if score < params.score_threshold {
            self.pending.push(PendingAbsorption {
                is_buy: block.is_buy,
                volume: block.total_qty,
                expected_ticks,
                score,
                confirmation_deadline: block.last_time.saturating_add(params.confirm_window_ms),
                zone_center_units: block.vwap_units(),
                zone_min_units: block.min_price_units,
                zone_max_units: block.max_price_units,
                start_x,
            });
        }
    }

    /// Phase 3: Check pending absorptions against subsequent price action.
    ///
    /// Uses `swap_remove` for O(1) removal. Order of pending items
    /// does not matter for correctness.
    ///
    /// `current_x` is the X coordinate (timestamp or candle index)
    /// used as `end_x` when confirming a zone.
    pub fn check_pending(&mut self, trade_price_units: i64, trade_time: u64, current_x: u64) {
        let tick_f = self.tick_size_units as f64;
        let mut i = 0;
        while i < self.pending.len() {
            let p = &self.pending[i];
            let half_width = (p.zone_max_units - p.zone_min_units).max(self.tick_size_units);

            let expected_displacement = if p.is_buy {
                p.expected_ticks * tick_f
            } else {
                -(p.expected_ticks * tick_f)
            };
            let expected_price = p.zone_center_units as f64 + expected_displacement;

            // Price followed through beyond expected → not absorption
            let followed_through = if p.is_buy {
                trade_price_units as f64 > expected_price
            } else {
                (trade_price_units as f64) < expected_price
            };

            if followed_through {
                self.pending.swap_remove(i);
                continue;
            }

            // Deadline passed without price returning to zone → reject
            let deadline_passed = trade_time > p.confirmation_deadline;
            if deadline_passed {
                self.pending.swap_remove(i);
                continue;
            }

            // Price returned to zone → confirmed absorption
            let in_zone = (trade_price_units - p.zone_center_units).abs() <= half_width;
            if in_zone {
                let p = self.pending.swap_remove(i);
                if self.confirmed_zones.len() < MAX_ZONES {
                    self.confirmed_zones.push(AbsorptionZone {
                        center_price_units: p.zone_center_units,
                        zone_half_width_units: (p.zone_max_units - p.zone_min_units)
                            .max(self.tick_size_units),
                        start_x: p.start_x,
                        end_x: current_x,
                        is_buy: p.is_buy,
                        strength: p.score as f32,
                        volume: p.volume,
                    });
                }
                continue;
            }

            i += 1;
        }
    }

    /// Get confirmed zones (test only).
    #[cfg(test)]
    pub fn zones(&self) -> &[AbsorptionZone] {
        &self.confirmed_zones
    }

    /// Get confirmed zones as `PriceLevel` primitives for rendering.
    /// Uses an internal cache — only rebuilds when the zone count changes.
    pub fn price_levels(&mut self, params: &AbsorptionParams) -> &[PriceLevel] {
        if self.confirmed_zones.len() == self.cached_levels_zone_count {
            return &self.cached_levels;
        }

        self.cached_levels.clear();
        self.cached_levels.reserve(self.confirmed_zones.len());

        for zone in &self.confirmed_zones {
            let base_color = if zone.is_buy {
                params.buy_zone_color
            } else {
                params.sell_zone_color
            };
            let opacity = (1.0 - zone.strength) * params.zone_opacity;

            let label = if params.show_zone_labels {
                let side = if zone.is_buy { "Buy" } else { "Sell" };
                format!(
                    "{} Abs {:.0} ({:.0}%)",
                    side,
                    zone.volume,
                    zone.strength * 100.0
                )
            } else {
                String::new()
            };

            let center_f64 = Price::from_units(zone.center_price_units).to_f64();
            let half_width_f64 = Price::from_units(zone.zone_half_width_units).to_f64() / 2.0;

            self.cached_levels.push(PriceLevel {
                price: center_f64,
                label,
                color: base_color,
                style: LineStyleValue::Dashed,
                opacity,
                show_label: params.show_zone_labels,
                fill_above: None,
                fill_below: None,
                width: 1.0,
                start_x: Some(zone.start_x),
                end_x: Some(zone.end_x),
                zone_half_width: Some(half_width_f64),
                tooltip_data: None,
            });
        }

        self.cached_levels_zone_count = self.confirmed_zones.len();
        &self.cached_levels
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::studies::orderflow::big_trades::params::{
        DEFAULT_ABSORPTION_BUY_COLOR, DEFAULT_ABSORPTION_SELL_COLOR,
    };

    fn default_absorption_params() -> AbsorptionParams {
        AbsorptionParams {
            enabled: true,
            lambda_window: 50,
            lambda_smooth: 20,
            score_threshold: 0.25,
            volume_k: 2.0,
            confirm_window_ms: 20000,
            buy_zone_color: DEFAULT_ABSORPTION_BUY_COLOR,
            sell_zone_color: DEFAULT_ABSORPTION_SELL_COLOR,
            zone_opacity: 0.30,
            show_zone_labels: true,
        }
    }

    fn make_block(
        is_buy: bool,
        first_price: i64,
        vwap_price: i64,
        qty: f64,
        first_time: u64,
        last_time: u64,
        min_price: i64,
        max_price: i64,
    ) -> TradeBlock {
        TradeBlock {
            is_buy,
            vwap_numerator: vwap_price as f64 * qty,
            total_qty: qty,
            first_time,
            last_time,
            fill_count: (qty as u32).max(1),
            min_price_units: min_price,
            max_price_units: max_price,
            candle_open: first_time,
            first_price_units: first_price,
        }
    }

    // ES tick size = 0.25 => in Price units (10^8): 25_000_000
    const ES_TICK_UNITS: i64 = 25_000_000;
    // ES price 5000.00 in units
    const ES_5000: i64 = 500_000_000_000;

    #[test]
    fn test_estimator_records_impact() {
        let mut est = ImpactEstimator::new(50, 20);
        let block = make_block(
            true,
            ES_5000,
            ES_5000 + 2 * ES_TICK_UNITS,
            100.0,
            1000,
            1040,
            ES_5000,
            ES_5000 + 2 * ES_TICK_UNITS,
        );
        est.record(&block, ES_TICK_UNITS);
        assert_eq!(est.sample_count(), 1);
        assert!(
            (est.lambda() - 0.02).abs() < 1e-10,
            "lambda: {}",
            est.lambda()
        );
    }

    #[test]
    fn test_estimator_ema_smoothing() {
        let mut est = ImpactEstimator::new(50, 3);
        // alpha for period 3 = 2/4 = 0.5

        let b1 = make_block(
            true,
            ES_5000,
            ES_5000 + 2 * ES_TICK_UNITS,
            100.0,
            1000,
            1040,
            ES_5000,
            ES_5000 + 2 * ES_TICK_UNITS,
        );
        est.record(&b1, ES_TICK_UNITS);
        assert!((est.lambda() - 0.02).abs() < 1e-10);

        // EMA = 0.5 * 0.04 + 0.5 * 0.02 = 0.03
        let b2 = make_block(
            true,
            ES_5000,
            ES_5000 + 4 * ES_TICK_UNITS,
            100.0,
            2000,
            2040,
            ES_5000,
            ES_5000 + 4 * ES_TICK_UNITS,
        );
        est.record(&b2, ES_TICK_UNITS);
        assert!(
            (est.lambda() - 0.03).abs() < 1e-10,
            "lambda: {}",
            est.lambda()
        );
    }

    #[test]
    fn test_estimator_ring_buffer_evicts() {
        let mut est = ImpactEstimator::new(3, 20);
        for i in 0..5 {
            let block = make_block(
                true,
                ES_5000,
                ES_5000 + ES_TICK_UNITS,
                (50 + i * 10) as f64,
                i as u64 * 1000,
                i as u64 * 1000 + 40,
                ES_5000,
                ES_5000 + ES_TICK_UNITS,
            );
            est.record(&block, ES_TICK_UNITS);
        }
        assert_eq!(est.history.len(), 3);
        assert_eq!(est.sample_count(), 3);
    }

    #[test]
    fn test_volume_threshold() {
        let mut est = ImpactEstimator::new(50, 20);
        for i in 0..5 {
            let vol = 50.0 + i as f64 * 10.0;
            let block = make_block(
                true,
                ES_5000,
                ES_5000 + ES_TICK_UNITS,
                vol,
                i as u64 * 1000,
                i as u64 * 1000 + 40,
                ES_5000,
                ES_5000 + ES_TICK_UNITS,
            );
            est.record(&block, ES_TICK_UNITS);
        }
        // mean=70, var=(400+100+0+100+400)/5=200, std=~14.14
        let threshold = est.volume_threshold(2.0);
        let expected = 70.0 + 2.0 * (200.0_f64).sqrt();
        assert!(
            (threshold - expected).abs() < 1e-6,
            "threshold: {} expected: {}",
            threshold,
            expected
        );
    }

    #[test]
    fn test_detector_no_detection_below_min_samples() {
        let params = default_absorption_params();
        let mut det = AbsorptionDetector::new(ES_TICK_UNITS, &params);
        let block = make_block(true, ES_5000, ES_5000, 200.0, 1000, 1040, ES_5000, ES_5000);
        det.on_block_flushed(&block, &params, 1000);
        assert!(det.pending.is_empty());
        assert!(det.zones().is_empty());
    }

    #[test]
    fn test_detector_absorption_detected() {
        let params = default_absorption_params();
        let mut det = AbsorptionDetector::new(ES_TICK_UNITS, &params);

        for i in 0..10 {
            let block = make_block(
                true,
                ES_5000,
                ES_5000 + 2 * ES_TICK_UNITS,
                50.0,
                i * 1000,
                i * 1000 + 40,
                ES_5000,
                ES_5000 + 2 * ES_TICK_UNITS,
            );
            det.on_block_flushed(&block, &params, i * 1000);
        }

        let absorption_block = make_block(
            true, ES_5000, ES_5000, 500.0, 10000, 10040, ES_5000, ES_5000,
        );
        det.on_block_flushed(&absorption_block, &params, 10000);

        assert_eq!(
            det.pending.len(),
            1,
            "expected 1 pending, got {}",
            det.pending.len()
        );
        assert!(det.pending[0].score < params.score_threshold);
    }

    #[test]
    fn test_detector_confirmation_by_price_return() {
        let params = default_absorption_params();
        let mut det = AbsorptionDetector::new(ES_TICK_UNITS, &params);

        for i in 0..10 {
            let block = make_block(
                true,
                ES_5000,
                ES_5000 + 2 * ES_TICK_UNITS,
                50.0,
                i * 1000,
                i * 1000 + 40,
                ES_5000,
                ES_5000 + 2 * ES_TICK_UNITS,
            );
            det.on_block_flushed(&block, &params, i * 1000);
        }

        let block = make_block(
            true, ES_5000, ES_5000, 500.0, 10000, 10040, ES_5000, ES_5000,
        );
        det.on_block_flushed(&block, &params, 10000);
        assert_eq!(det.pending.len(), 1);

        det.check_pending(ES_5000, 11000, 11000);
        assert!(det.pending.is_empty());
        assert_eq!(det.zones().len(), 1);
        assert!(det.zones()[0].is_buy);
        assert_eq!(det.zones()[0].end_x, 11000);
    }

    #[test]
    fn test_detector_rejection_by_follow_through() {
        let params = default_absorption_params();
        let mut det = AbsorptionDetector::new(ES_TICK_UNITS, &params);

        for i in 0..10 {
            let block = make_block(
                true,
                ES_5000,
                ES_5000 + 2 * ES_TICK_UNITS,
                50.0,
                i * 1000,
                i * 1000 + 40,
                ES_5000,
                ES_5000 + 2 * ES_TICK_UNITS,
            );
            det.on_block_flushed(&block, &params, i * 1000);
        }

        let block = make_block(
            true, ES_5000, ES_5000, 500.0, 10000, 10040, ES_5000, ES_5000,
        );
        det.on_block_flushed(&block, &params, 10000);
        assert_eq!(det.pending.len(), 1);

        let far_above = ES_5000 + 100 * ES_TICK_UNITS;
        det.check_pending(far_above, 11000, 11000);
        assert!(det.pending.is_empty());
        assert!(det.zones().is_empty(), "should have been rejected");
    }

    #[test]
    fn test_detector_confirmation_by_deadline() {
        let params = default_absorption_params();
        let mut det = AbsorptionDetector::new(ES_TICK_UNITS, &params);

        for i in 0..10 {
            let block = make_block(
                true,
                ES_5000,
                ES_5000 + 2 * ES_TICK_UNITS,
                50.0,
                i * 1000,
                i * 1000 + 40,
                ES_5000,
                ES_5000 + 2 * ES_TICK_UNITS,
            );
            det.on_block_flushed(&block, &params, i * 1000);
        }

        let block = make_block(
            true, ES_5000, ES_5000, 500.0, 10000, 10040, ES_5000, ES_5000,
        );
        det.on_block_flushed(&block, &params, 10000);
        assert_eq!(det.pending.len(), 1);

        let slight_move = ES_5000 + ES_TICK_UNITS;
        det.check_pending(slight_move, 10040 + params.confirm_window_ms + 1, 31000);
        assert!(det.pending.is_empty());
        assert!(
            det.zones().is_empty(),
            "deadline expiry should reject, not confirm"
        );
    }

    #[test]
    fn test_sell_side_absorption() {
        let params = default_absorption_params();
        let mut det = AbsorptionDetector::new(ES_TICK_UNITS, &params);

        for i in 0..10 {
            let block = make_block(
                false,
                ES_5000,
                ES_5000 - 2 * ES_TICK_UNITS,
                50.0,
                i * 1000,
                i * 1000 + 40,
                ES_5000 - 2 * ES_TICK_UNITS,
                ES_5000,
            );
            det.on_block_flushed(&block, &params, i * 1000);
        }

        let block = make_block(
            false, ES_5000, ES_5000, 500.0, 10000, 10040, ES_5000, ES_5000,
        );
        det.on_block_flushed(&block, &params, 10000);
        assert_eq!(det.pending.len(), 1);

        det.check_pending(ES_5000, 11000, 11000);
        assert_eq!(det.zones().len(), 1);
        assert!(!det.zones()[0].is_buy);
    }

    #[test]
    fn test_price_levels() {
        let params = default_absorption_params();
        let mut det = AbsorptionDetector::new(ES_TICK_UNITS, &params);

        det.confirmed_zones.push(AbsorptionZone {
            center_price_units: ES_5000,
            zone_half_width_units: 2 * ES_TICK_UNITS,
            start_x: 1000,
            end_x: 2000,
            is_buy: true,
            strength: 0.1,
            volume: 200.0,
        });

        let levels = det.price_levels(&params).to_vec();
        assert_eq!(levels.len(), 1);
        assert!(levels[0].zone_half_width.is_some());
        assert_eq!(levels[0].start_x, Some(1000));
        assert_eq!(levels[0].end_x, Some(2000));
        assert!(levels[0].show_label);
        assert!(levels[0].label.contains("Buy Abs"));
    }

    #[test]
    fn test_reset_clears_state() {
        let params = default_absorption_params();
        let mut det = AbsorptionDetector::new(ES_TICK_UNITS, &params);

        let block = make_block(
            true,
            ES_5000,
            ES_5000 + ES_TICK_UNITS,
            50.0,
            1000,
            1040,
            ES_5000,
            ES_5000 + ES_TICK_UNITS,
        );
        det.estimator.record(&block, ES_TICK_UNITS);
        det.confirmed_zones.push(AbsorptionZone {
            center_price_units: ES_5000,
            zone_half_width_units: ES_TICK_UNITS,
            start_x: 1000,
            end_x: 2000,
            is_buy: true,
            strength: 0.1,
            volume: 50.0,
        });

        det.reset();
        assert_eq!(det.estimator.sample_count(), 0);
        assert!(det.pending.is_empty());
        assert!(det.confirmed_zones.is_empty());
    }

    #[test]
    fn test_zero_tick_size_no_panic() {
        let params = default_absorption_params();
        let mut det = AbsorptionDetector::new(0, &params);
        let block = make_block(true, ES_5000, ES_5000, 100.0, 1000, 1040, ES_5000, ES_5000);
        // tick_size_units is clamped to 1 in constructor, so record works
        det.on_block_flushed(&block, &params, 1000);
    }

    #[test]
    fn test_zero_volume_block_ignored() {
        let mut est = ImpactEstimator::new(50, 20);
        let block = make_block(true, ES_5000, ES_5000, 0.0, 1000, 1040, ES_5000, ES_5000);
        est.record(&block, ES_TICK_UNITS);
        assert_eq!(est.sample_count(), 0);
    }

    #[test]
    fn test_has_pending() {
        let params = default_absorption_params();
        let mut det = AbsorptionDetector::new(ES_TICK_UNITS, &params);
        assert!(!det.has_pending());

        // Seed estimator and trigger a pending
        for i in 0..10 {
            let block = make_block(
                true,
                ES_5000,
                ES_5000 + 2 * ES_TICK_UNITS,
                50.0,
                i * 1000,
                i * 1000 + 40,
                ES_5000,
                ES_5000 + 2 * ES_TICK_UNITS,
            );
            det.on_block_flushed(&block, &params, i * 1000);
        }
        let block = make_block(
            true, ES_5000, ES_5000, 500.0, 10000, 10040, ES_5000, ES_5000,
        );
        det.on_block_flushed(&block, &params, 10000);
        assert!(det.has_pending());
    }

    #[test]
    fn test_pending_cap() {
        let params = AbsorptionParams {
            score_threshold: 1.0, // everything passes
            volume_k: 0.0,        // threshold = mean (very low)
            ..default_absorption_params()
        };
        let mut det = AbsorptionDetector::new(ES_TICK_UNITS, &params);

        // Seed estimator
        for i in 0..10 {
            let block = make_block(
                true,
                ES_5000,
                ES_5000 + 2 * ES_TICK_UNITS,
                50.0,
                i * 1000,
                i * 1000 + 40,
                ES_5000,
                ES_5000 + 2 * ES_TICK_UNITS,
            );
            det.on_block_flushed(&block, &params, i * 1000);
        }

        // Try to create more than MAX_PENDING
        for i in 0..100 {
            let block = make_block(
                true,
                ES_5000,
                ES_5000,
                500.0,
                (20 + i) * 1000,
                (20 + i) * 1000 + 40,
                ES_5000,
                ES_5000,
            );
            det.on_block_flushed(&block, &params, (20 + i) * 1000);
        }

        assert!(
            det.pending.len() <= MAX_PENDING,
            "pending {} > cap {}",
            det.pending.len(),
            MAX_PENDING
        );
    }

    #[test]
    fn test_reset_with_tick_size() {
        let params = default_absorption_params();
        let mut det = AbsorptionDetector::new(ES_TICK_UNITS, &params);

        // Add some state
        for i in 0..10 {
            let block = make_block(
                true,
                ES_5000,
                ES_5000 + 2 * ES_TICK_UNITS,
                50.0,
                i * 1000,
                i * 1000 + 40,
                ES_5000,
                ES_5000 + 2 * ES_TICK_UNITS,
            );
            det.on_block_flushed(&block, &params, i * 1000);
        }

        let new_tick = 12_500_000; // NQ tick size
        det.reset_with_tick_size(new_tick, &params);
        assert_eq!(det.tick_size_units, new_tick);
        assert_eq!(det.estimator.sample_count(), 0);
        assert!(det.pending.is_empty());
        assert!(det.confirmed_zones.is_empty());
    }

    #[test]
    fn test_estimator_clear_reuses_allocation() {
        let mut est = ImpactEstimator::new(50, 20);
        for i in 0..10 {
            let block = make_block(
                true,
                ES_5000,
                ES_5000 + ES_TICK_UNITS,
                50.0,
                i * 1000,
                i * 1000 + 40,
                ES_5000,
                ES_5000 + ES_TICK_UNITS,
            );
            est.record(&block, ES_TICK_UNITS);
        }
        let cap_before = est.history.capacity();

        est.clear(50, 20);
        assert_eq!(est.sample_count(), 0);
        assert_eq!(est.lambda(), 0.0);
        // VecDeque capacity should be preserved (no reallocation)
        assert!(est.history.capacity() >= cap_before);
    }

    #[test]
    fn test_zone_has_bounded_x() {
        let params = default_absorption_params();
        let mut det = AbsorptionDetector::new(ES_TICK_UNITS, &params);

        for i in 0..10 {
            let block = make_block(
                true,
                ES_5000,
                ES_5000 + 2 * ES_TICK_UNITS,
                50.0,
                i * 1000,
                i * 1000 + 40,
                ES_5000,
                ES_5000 + 2 * ES_TICK_UNITS,
            );
            det.on_block_flushed(&block, &params, i * 1000);
        }

        let block = make_block(
            true, ES_5000, ES_5000, 500.0, 10000, 10040, ES_5000, ES_5000,
        );
        det.on_block_flushed(&block, &params, 10000);
        assert_eq!(det.pending.len(), 1);

        det.check_pending(ES_5000, 15000, 15000);
        assert_eq!(det.zones().len(), 1);
        let zone = &det.zones()[0];
        assert!(
            zone.start_x < zone.end_x,
            "start_x ({}) should be < end_x ({})",
            zone.start_x,
            zone.end_x
        );
    }
}
