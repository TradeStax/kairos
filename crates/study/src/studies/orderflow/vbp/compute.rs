//! VBP computation logic.
//!
//! Contains the core `compute()` implementation for `VbpStudy`
//! plus helper functions: candle range resolution, trade filtering,
//! developing feature computation, and anchored VWAP calculation.

use crate::output::{NodeDetectionMethod, VbpSplitPeriod};
use data::{Candle, Price, Trade};

use super::VbpStudy;

/// Milliseconds in one UTC calendar day.
const MS_PER_DAY: u64 = 86_400_000;

/// Time-series point: (timestamp_ms, value).
pub(super) type TimeSeries = Vec<(u64, f32)>;

/// Developing-feature series: (timestamp_ms, price_units).
type DevelopingSeries = Vec<(u64, i64)>;

/// Result of computing developing POC, peak, and valley series.
type DevelopingFeatures = (DevelopingSeries, DevelopingSeries, DevelopingSeries);

impl VbpStudy {
    /// Resolve candle range for Custom period mode.
    pub(super) fn resolve_custom_range<'a>(&self, candles: &'a [Candle]) -> &'a [Candle] {
        let start = self.config.get_int("custom_start", 0) as u64;
        let end = self.config.get_int("custom_end", 0) as u64;
        if start == 0 && end == 0 {
            candles
        } else {
            Self::slice_by_time(candles, start, end)
        }
    }

    /// Split candles into time-based or count-based segments.
    ///
    /// Returns up to `max_profiles` segments (the most recent
    /// ones). Each segment is a contiguous candle sub-slice.
    pub(super) fn split_candles_into_segments(
        candles: &[Candle],
        split: VbpSplitPeriod,
        max_profiles: usize,
    ) -> Vec<&[Candle]> {
        if candles.is_empty() {
            return Vec::new();
        }

        let segments = match split {
            VbpSplitPeriod::Day => split_by_duration(candles, MS_PER_DAY),
            VbpSplitPeriod::Hours(h) => {
                let ms = h as u64 * 3_600_000;
                split_by_duration(candles, ms)
            }
            VbpSplitPeriod::Minutes(m) => {
                let ms = m as u64 * 60_000;
                split_by_duration(candles, ms)
            }
            VbpSplitPeriod::Contracts(n) => split_by_count(candles, n as usize),
        };

        // Keep the most recent `max_profiles` segments.
        if segments.len() > max_profiles {
            segments[segments.len() - max_profiles..].to_vec()
        } else {
            segments
        }
    }

    /// Binary-search slice of candles by timestamp range.
    pub(super) fn slice_by_time(candles: &[Candle], start: u64, end: u64) -> &[Candle] {
        let start_idx = candles.partition_point(|c| c.time.to_millis() < start);
        let end_idx = candles.partition_point(|c| c.time.to_millis() <= end);
        &candles[start_idx..end_idx]
    }

    /// Filter trades to the resolved candle time range.
    pub(super) fn filter_trades<'a>(trades: &'a [Trade], candles: &[Candle]) -> &'a [Trade] {
        if candles.is_empty() || trades.is_empty() {
            return &[];
        }
        let start = candles.first().map(|c| c.time.to_millis()).unwrap_or(0);
        let end = candles.last().map(|c| c.time.to_millis()).unwrap_or(0);

        let start_idx = trades.partition_point(|t| t.time.to_millis() < start);
        let end_idx = trades.partition_point(|t| t.time.to_millis() <= end);
        &trades[start_idx..end_idx]
    }

    /// Compute developing features (POC, peak, valley) in a
    /// single pass over candles. Builds incremental volume profile
    /// and extracts all three developing series.
    pub(super) fn compute_developing_features(
        candle_slice: &[Candle],
        tick_size: Price,
        group_quantum: i64,
        hvn_method: NodeDetectionMethod,
        hvn_threshold: f32,
        lvn_method: NodeDetectionMethod,
        lvn_threshold: f32,
        need_poc: bool,
        need_peak: bool,
        need_valley: bool,
    ) -> DevelopingFeatures {
        use crate::output::NodeDetectionMethod as NDM;
        use std::collections::HashMap;

        let step = group_quantum.max(tick_size.units()).max(1);
        let cap = candle_slice
            .iter()
            .map(|c| {
                let lo = c.low.round_to_tick(tick_size).units() / step;
                let hi = (c.high.round_to_tick(tick_size).units() + step - 1) / step;
                (hi - lo + 1) as usize
            })
            .max()
            .unwrap_or(64);
        let mut volume_map: HashMap<i64, f64> = HashMap::with_capacity(cap * 2);
        let mut poc_price = 0i64;
        let mut poc_vol = 0.0f64;

        // Running accumulators for stats
        let mut total_vol = 0.0f64;
        let mut sum_sq = 0.0f64;
        let mut max_vol = 0.0f64;
        let mut n_levels = 0usize;

        let n = candle_slice.len();
        let mut poc_pts = if need_poc {
            Vec::with_capacity(n)
        } else {
            Vec::new()
        };
        let mut peak_pts = if need_peak {
            Vec::with_capacity(n)
        } else {
            Vec::new()
        };
        let mut valley_pts = if need_valley {
            Vec::with_capacity(n)
        } else {
            Vec::new()
        };

        let mut last_peak_price = 0i64;
        let mut last_valley_price = 0i64;

        for c in candle_slice {
            let low = (c.low.round_to_tick(tick_size).units() / step) * step;
            let high = ((c.high.round_to_tick(tick_size).units() + step - 1) / step) * step;
            let vol = c.volume() as f64;
            let cnt = if high >= low {
                ((high - low) / step + 1) as f64
            } else {
                1.0
            };
            let vol_per = vol / cnt;

            let mut p = low;
            while p <= high {
                let entry = volume_map.entry(p).or_insert_with(|| {
                    n_levels += 1;
                    0.0
                });
                *entry += vol_per;
                // Update running stats
                total_vol += vol_per;
                sum_sq += (*entry) * (*entry) - (*entry - vol_per) * (*entry - vol_per);
                if *entry > max_vol {
                    max_vol = *entry;
                }
                // POC tracking
                if *entry > poc_vol {
                    poc_vol = *entry;
                    poc_price = p;
                }
                p += step;
            }

            let ts = c.time.to_millis();

            if need_poc {
                poc_pts.push((ts, poc_price));
            }

            // Compute peak/valley from running profile
            if (need_peak || need_valley) && n_levels >= 3 && total_vol > 0.0 {
                let mean = total_vol / n_levels as f64;
                let var = sum_sq / n_levels as f64 - mean * mean;
                let std_dev = var.max(0.0).sqrt();

                if need_peak {
                    let hvn_cutoff = match hvn_method {
                        NDM::Percentile => {
                            let mut vols: Vec<f64> = volume_map.values().copied().collect();
                            let idx = ((hvn_threshold * (vols.len() - 1) as f32) as usize)
                                .min(vols.len() - 1);
                            vols.select_nth_unstable_by(idx, |a, b| a.partial_cmp(b).unwrap());
                            vols[idx]
                        }
                        NDM::Relative => max_vol * hvn_threshold as f64,
                        NDM::StdDev => {
                            if std_dev < f64::EPSILON {
                                max_vol + 1.0
                            } else {
                                mean + std_dev * hvn_threshold as f64
                            }
                        }
                    };

                    let mut best_price = last_peak_price;
                    let mut best_vol = 0.0f64;
                    for (&price, &v) in &volume_map {
                        if v >= hvn_cutoff && v > best_vol {
                            best_vol = v;
                            best_price = price;
                        }
                    }
                    if best_vol > 0.0 {
                        last_peak_price = best_price;
                    }
                    peak_pts.push((ts, last_peak_price));
                }

                if need_valley {
                    let lvn_cutoff = match lvn_method {
                        NDM::Percentile => {
                            let mut vols: Vec<f64> = volume_map.values().copied().collect();
                            let idx = ((lvn_threshold * (vols.len() - 1) as f32) as usize)
                                .min(vols.len() - 1);
                            vols.select_nth_unstable_by(idx, |a, b| a.partial_cmp(b).unwrap());
                            vols[idx]
                        }
                        NDM::Relative => max_vol * lvn_threshold as f64,
                        NDM::StdDev => {
                            if std_dev < f64::EPSILON {
                                -1.0
                            } else {
                                (mean - std_dev * lvn_threshold as f64).max(0.0)
                            }
                        }
                    };

                    // Sort entries by price to find local
                    // minima -- avoids selecting tail levels.
                    let mut sorted: Vec<(i64, f64)> =
                        volume_map.iter().map(|(&p, &v)| (p, v)).collect();
                    sorted.sort_unstable_by_key(|&(p, _)| p);

                    let mut best_price = last_valley_price;
                    let mut best_vol = f64::MAX;
                    for j in 1..sorted.len().saturating_sub(1) {
                        let v = sorted[j].1;
                        if v <= lvn_cutoff
                            && v > 0.0
                            && v < sorted[j - 1].1
                            && v < sorted[j + 1].1
                            && v < best_vol
                        {
                            best_vol = v;
                            best_price = sorted[j].0;
                        }
                    }
                    if best_vol < f64::MAX {
                        last_valley_price = best_price;
                    }
                    valley_pts.push((ts, last_valley_price));
                }
            } else {
                if need_peak {
                    peak_pts.push((ts, last_peak_price));
                }
                if need_valley {
                    valley_pts.push((ts, last_valley_price));
                }
            }
        }

        (poc_pts, peak_pts, valley_pts)
    }

    /// Compute anchored VWAP over the candle slice.
    pub(super) fn compute_vwap(
        candle_slice: &[Candle],
        show_bands: bool,
        band_mult: f32,
    ) -> (TimeSeries, TimeSeries, TimeSeries) {
        let mut cum_tp_vol: f64 = 0.0;
        let mut cum_vol: f64 = 0.0;
        let mut cum_tp2_vol: f64 = 0.0;

        let n = candle_slice.len();
        let mut vwap_pts = Vec::with_capacity(n);
        let mut upper_pts = Vec::with_capacity(n);
        let mut lower_pts = Vec::with_capacity(n);

        for c in candle_slice {
            let tp = (c.high.to_f32() + c.low.to_f32() + c.close.to_f32()) as f64 / 3.0;
            let vol = c.volume() as f64;
            let ts = c.time.to_millis();

            cum_tp_vol += tp * vol;
            cum_vol += vol;
            cum_tp2_vol += tp * tp * vol;

            if cum_vol > 0.0 {
                let vwap = cum_tp_vol / cum_vol;
                vwap_pts.push((ts, vwap as f32));

                if show_bands {
                    let variance = (cum_tp2_vol / cum_vol) - (vwap * vwap);
                    let std_dev = if variance > 0.0 { variance.sqrt() } else { 0.0 };
                    let mult = band_mult as f64;
                    upper_pts.push((ts, (vwap + std_dev * mult) as f32));
                    lower_pts.push((ts, (vwap - std_dev * mult) as f32));
                }
            } else {
                vwap_pts.push((ts, tp as f32));
                if show_bands {
                    upper_pts.push((ts, tp as f32));
                    lower_pts.push((ts, tp as f32));
                }
            }
        }

        (vwap_pts, upper_pts, lower_pts)
    }
}

/// Split candles into segments at fixed time-interval boundaries.
///
/// Each segment contains candles whose timestamp falls in the
/// same `floor(ts / ms_interval)` bucket.
fn split_by_duration(candles: &[Candle], ms_interval: u64) -> Vec<&[Candle]> {
    if candles.is_empty() || ms_interval == 0 {
        return Vec::new();
    }

    let mut segments = Vec::new();
    let mut seg_start = 0usize;
    let mut cur_bucket = candles[0].time.to_millis() / ms_interval;

    for i in 1..candles.len() {
        let bucket = candles[i].time.to_millis() / ms_interval;
        if bucket != cur_bucket {
            segments.push(&candles[seg_start..i]);
            seg_start = i;
            cur_bucket = bucket;
        }
    }
    // Push the last segment
    if seg_start < candles.len() {
        segments.push(&candles[seg_start..]);
    }
    segments
}

/// Split candles into segments of `n` candles each.
fn split_by_count(candles: &[Candle], n: usize) -> Vec<&[Candle]> {
    if candles.is_empty() || n == 0 {
        return Vec::new();
    }
    candles.chunks(n).collect()
}
