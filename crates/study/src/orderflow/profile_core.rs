//! Shared profile computation functions.
//!
//! Reusable logic for building volume profiles from candles or trades,
//! finding POC, and calculating value area. Used by both
//! `VolumeProfileStudy` and `VbpStudy`.

use crate::output::ProfileLevel;
use data::{Candle, Price, Trade};
use std::collections::HashMap;

/// Build a volume profile from candle data.
///
/// Distributes each candle's buy/sell volume across price levels
/// from low to high at `tick_size` increments. Volume is weighted
/// toward the candle body (open-close range) for a more realistic
/// profile shape:
///   - Body (open→close range): 60% of volume
///   - Upper wick: 20% of volume
///   - Lower wick: 20% of volume
pub fn build_profile_from_candles(
    candles: &[Candle],
    tick_size: Price,
    group_quantum: i64,
) -> Vec<ProfileLevel> {
    let mut volume_map: HashMap<i64, (f64, f64)> = HashMap::new();
    let step = group_quantum.max(tick_size.units());

    for c in candles {
        let low_units =
            (c.low.round_to_tick(tick_size).units() / step) * step;
        let high_units =
            ((c.high.round_to_tick(tick_size).units() + step - 1)
                / step)
                * step;

        if step <= 0 || high_units < low_units {
            continue;
        }

        let body_low_units =
            (c.open.min(c.close).round_to_tick(tick_size).units()
                / step)
                * step;
        let body_high_units =
            ((c.open.max(c.close).round_to_tick(tick_size).units()
                + step
                - 1)
                / step)
                * step;

        let buy_vol = c.buy_volume.value();
        let sell_vol = c.sell_volume.value();

        // Count levels in each zone
        let count_levels = |lo: i64, hi: i64| -> f64 {
            if hi < lo { 0.0 } else { ((hi - lo) / step + 1) as f64 }
        };

        let body_count = count_levels(body_low_units, body_high_units);
        let lower_wick_count = if body_low_units > low_units {
            count_levels(low_units, body_low_units - step)
        } else {
            0.0
        };
        let upper_wick_count = if high_units > body_high_units {
            count_levels(body_high_units + step, high_units)
        } else {
            0.0
        };

        // If candle is a doji (no body), distribute evenly
        if body_count <= 1.0
            && lower_wick_count == 0.0
            && upper_wick_count == 0.0
        {
            let total = count_levels(low_units, high_units);
            if total > 0.0 {
                let buy_per = buy_vol / total;
                let sell_per = sell_vol / total;
                let mut p = low_units;
                while p <= high_units {
                    let e =
                        volume_map.entry(p).or_insert((0.0, 0.0));
                    e.0 += buy_per;
                    e.1 += sell_per;
                    p += step;
                }
            }
            continue;
        }

        // Weighted distribution: 60% body, 20% each wick
        let has_lower = lower_wick_count > 0.0;
        let has_upper = upper_wick_count > 0.0;

        // Redistribute wick share to body if wick doesn't exist
        let (body_pct, lower_pct, upper_pct) = match (has_lower, has_upper)
        {
            (true, true) => (0.60, 0.20, 0.20),
            (true, false) => (0.75, 0.25, 0.0),
            (false, true) => (0.75, 0.0, 0.25),
            (false, false) => (1.0, 0.0, 0.0),
        };

        // Distribute body volume
        if body_count > 0.0 {
            let buy_per = (buy_vol * body_pct) / body_count;
            let sell_per = (sell_vol * body_pct) / body_count;
            let mut p = body_low_units;
            while p <= body_high_units {
                let e = volume_map.entry(p).or_insert((0.0, 0.0));
                e.0 += buy_per;
                e.1 += sell_per;
                p += step;
            }
        }

        // Distribute lower wick volume
        if has_lower {
            let buy_per = (buy_vol * lower_pct) / lower_wick_count;
            let sell_per = (sell_vol * lower_pct) / lower_wick_count;
            let mut p = low_units;
            while p < body_low_units {
                let e = volume_map.entry(p).or_insert((0.0, 0.0));
                e.0 += buy_per;
                e.1 += sell_per;
                p += step;
            }
        }

        // Distribute upper wick volume
        if has_upper {
            let buy_per = (buy_vol * upper_pct) / upper_wick_count;
            let sell_per = (sell_vol * upper_pct) / upper_wick_count;
            let mut p = body_high_units + step;
            while p <= high_units {
                let e = volume_map.entry(p).or_insert((0.0, 0.0));
                e.0 += buy_per;
                e.1 += sell_per;
                p += step;
            }
        }
    }

    let mut levels: Vec<ProfileLevel> = volume_map
        .into_iter()
        .map(|(units, (buy, sell))| ProfileLevel {
            price: Price::from_units(units).to_f64(),
            price_units: units,
            buy_volume: buy as f32,
            sell_volume: sell as f32,
        })
        .collect();
    levels.sort_unstable_by_key(|l| l.price_units);
    levels
}

/// Build a volume profile from raw trades.
///
/// Groups trades by price level (rounded to `tick_size`) with
/// exact bid/ask attribution from trade side.
pub fn build_profile_from_trades(
    trades: &[Trade],
    tick_size: Price,
    group_quantum: i64,
) -> Vec<ProfileLevel> {
    let mut volume_map: HashMap<i64, (f64, f64)> =
        HashMap::with_capacity(trades.len() / 4 + 16);

    for t in trades {
        let price_units = if group_quantum > 0 {
            (t.price.round_to_tick(tick_size).units()
                / group_quantum)
                * group_quantum
        } else {
            t.price.round_to_tick(tick_size).units()
        };
        let entry =
            volume_map.entry(price_units).or_insert((0.0, 0.0));
        let qty = t.quantity.value();
        if t.is_buy() {
            entry.0 += qty;
        } else {
            entry.1 += qty;
        }
    }

    let mut levels: Vec<ProfileLevel> = volume_map
        .into_iter()
        .map(|(units, (buy, sell))| ProfileLevel {
            price: Price::from_units(units).to_f64(),
            price_units: units,
            buy_volume: buy as f32,
            sell_volume: sell as f32,
        })
        .collect();
    levels.sort_unstable_by_key(|l| l.price_units);
    levels
}

/// Find the index of the level with highest total volume (POC).
pub fn find_poc_index(levels: &[ProfileLevel]) -> Option<usize> {
    levels
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| {
            let total_a = a.buy_volume + a.sell_volume;
            let total_b = b.buy_volume + b.sell_volume;
            total_a
                .partial_cmp(&total_b)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(i, _)| i)
}

/// Calculate value area by expanding from POC until `percentage`
/// of total volume is captured.
///
/// Returns `(vah_index, val_index)`.
pub fn calculate_value_area(
    levels: &[ProfileLevel],
    poc_idx: usize,
    percentage: f64,
) -> Option<(usize, usize)> {
    if levels.is_empty() {
        return None;
    }

    let total_volume: f32 =
        levels.iter().map(|l| l.buy_volume + l.sell_volume).sum();
    let target = total_volume * percentage as f32;

    let mut accumulated =
        levels[poc_idx].buy_volume + levels[poc_idx].sell_volume;
    let mut upper = poc_idx;
    let mut lower = poc_idx;

    while accumulated < target
        && (lower > 0 || upper < levels.len() - 1)
    {
        let up_vol = if upper + 1 < levels.len() {
            levels[upper + 1].buy_volume
                + levels[upper + 1].sell_volume
        } else {
            0.0
        };
        let down_vol = if lower > 0 {
            levels[lower - 1].buy_volume
                + levels[lower - 1].sell_volume
        } else {
            0.0
        };

        if up_vol >= down_vol && upper + 1 < levels.len() {
            upper += 1;
            accumulated += up_vol;
        } else if lower > 0 {
            lower -= 1;
            accumulated += down_vol;
        } else if upper + 1 < levels.len() {
            upper += 1;
            accumulated += up_vol;
        } else {
            break;
        }
    }

    Some((upper, lower))
}

/// Detect high-volume and low-volume nodes in a profile.
///
/// Uses the specified detection method and threshold for each node
/// type, plus a minimum prominence filter.
///
/// Returns `(hvn_nodes, lvn_nodes)`.
pub fn detect_volume_nodes(
    levels: &[ProfileLevel],
    hvn_method: crate::output::NodeDetectionMethod,
    hvn_threshold: f32,
    lvn_method: crate::output::NodeDetectionMethod,
    lvn_threshold: f32,
    min_prominence: f32,
) -> (Vec<crate::output::VolumeNode>, Vec<crate::output::VolumeNode>)
{
    use crate::output::{NodeDetectionMethod, VolumeNode};

    if levels.len() < 3 {
        return (Vec::new(), Vec::new());
    }

    let volumes: Vec<f32> = levels
        .iter()
        .map(|l| l.buy_volume + l.sell_volume)
        .collect();

    let mut total_vol = 0.0_f32;
    let mut max_vol = 0.0_f32;
    let mut sum_sq = 0.0_f32;
    for &v in &volumes {
        total_vol += v;
        max_vol = max_vol.max(v);
        sum_sq += v * v;
    }
    if total_vol <= 0.0 {
        return (Vec::new(), Vec::new());
    }

    let mean_vol = total_vol / volumes.len() as f32;
    let variance =
        sum_sq / volumes.len() as f32 - mean_vol * mean_vol;
    let std_dev = variance.max(0.0).sqrt();

    // Compute HVN threshold value
    let hvn_cutoff = match hvn_method {
        NodeDetectionMethod::Percentile => {
            percentile_value(&volumes, hvn_threshold)
        }
        NodeDetectionMethod::Relative => max_vol * hvn_threshold,
        NodeDetectionMethod::StdDev => {
            // When std_dev is 0 (flat), require strictly above mean
            if std_dev < f32::EPSILON {
                max_vol + 1.0
            } else {
                mean_vol + std_dev * hvn_threshold
            }
        }
    };

    // Compute LVN threshold value
    let lvn_cutoff = match lvn_method {
        NodeDetectionMethod::Percentile => {
            percentile_value(&volumes, lvn_threshold)
        }
        NodeDetectionMethod::Relative => max_vol * lvn_threshold,
        NodeDetectionMethod::StdDev => {
            // When std_dev is 0 (flat), require strictly below mean
            if std_dev < f32::EPSILON {
                -1.0
            } else {
                (mean_vol - std_dev * lvn_threshold).max(0.0)
            }
        }
    };

    let mut hvn = Vec::new();
    let mut lvn = Vec::new();

    for (i, level) in levels.iter().enumerate() {
        let vol = volumes[i];

        // HVN: volume >= cutoff and passes prominence check
        if vol >= hvn_cutoff && prominence(i, &volumes) >= min_prominence {
            hvn.push(VolumeNode {
                price_units: level.price_units,
                price: level.price,
                volume: vol,
            });
        }

        // LVN: volume <= cutoff and passes prominence check
        if vol <= lvn_cutoff
            && vol > 0.0
            && prominence_inverse(i, &volumes) >= min_prominence
        {
            lvn.push(VolumeNode {
                price_units: level.price_units,
                price: level.price,
                volume: vol,
            });
        }
    }

    (hvn, lvn)
}

/// Detect volume zones and single peak/valley in a profile.
///
/// Builds contiguous HVN and LVN zones, plus the single dominant
/// peak (highest-volume HVN-qualifying level) and deepest valley
/// (lowest-volume LVN-qualifying level).
///
/// Returns `(hvn_zones, lvn_zones, peak, valley)`.
pub fn detect_volume_zones(
    levels: &[ProfileLevel],
    hvn_method: crate::output::NodeDetectionMethod,
    hvn_threshold: f32,
    lvn_method: crate::output::NodeDetectionMethod,
    lvn_threshold: f32,
    min_prominence: f32,
) -> (
    Vec<(i64, i64)>,
    Vec<(i64, i64)>,
    Option<crate::output::VolumeNode>,
    Option<crate::output::VolumeNode>,
) {
    use crate::output::{NodeDetectionMethod, VolumeNode};

    if levels.len() < 3 {
        return (Vec::new(), Vec::new(), None, None);
    }

    let volumes: Vec<f32> = levels
        .iter()
        .map(|l| l.buy_volume + l.sell_volume)
        .collect();

    let mut total_vol = 0.0_f32;
    let mut max_vol = 0.0_f32;
    let mut sum_sq = 0.0_f32;
    for &v in &volumes {
        total_vol += v;
        max_vol = max_vol.max(v);
        sum_sq += v * v;
    }
    if total_vol <= 0.0 {
        return (Vec::new(), Vec::new(), None, None);
    }

    let mean_vol = total_vol / volumes.len() as f32;
    let variance =
        sum_sq / volumes.len() as f32 - mean_vol * mean_vol;
    let std_dev = variance.max(0.0).sqrt();

    // Compute HVN cutoff
    let hvn_cutoff = match hvn_method {
        NodeDetectionMethod::Percentile => {
            percentile_value(&volumes, hvn_threshold)
        }
        NodeDetectionMethod::Relative => max_vol * hvn_threshold,
        NodeDetectionMethod::StdDev => {
            if std_dev < f32::EPSILON {
                max_vol + 1.0
            } else {
                mean_vol + std_dev * hvn_threshold
            }
        }
    };

    // Compute LVN cutoff
    let lvn_cutoff = match lvn_method {
        NodeDetectionMethod::Percentile => {
            percentile_value(&volumes, lvn_threshold)
        }
        NodeDetectionMethod::Relative => max_vol * lvn_threshold,
        NodeDetectionMethod::StdDev => {
            if std_dev < f32::EPSILON {
                -1.0
            } else {
                (mean_vol - std_dev * lvn_threshold).max(0.0)
            }
        }
    };

    // Build HVN zones — contiguous runs of levels above cutoff
    let mut hvn_zones = Vec::new();
    let mut zone_start: Option<i64> = None;
    for (i, level) in levels.iter().enumerate() {
        if volumes[i] >= hvn_cutoff {
            if zone_start.is_none() {
                zone_start = Some(level.price_units);
            }
        } else if let Some(start) = zone_start.take() {
            let end = levels[i - 1].price_units;
            hvn_zones.push((start, end));
        }
    }
    if let Some(start) = zone_start {
        hvn_zones.push((
            start,
            levels.last().unwrap().price_units,
        ));
    }

    // Build LVN zones — contiguous runs of levels below cutoff
    let mut lvn_zones = Vec::new();
    let mut zone_start: Option<i64> = None;
    for (i, level) in levels.iter().enumerate() {
        if volumes[i] <= lvn_cutoff && volumes[i] > 0.0 {
            if zone_start.is_none() {
                zone_start = Some(level.price_units);
            }
        } else if let Some(start) = zone_start.take() {
            let end = levels[i - 1].price_units;
            lvn_zones.push((start, end));
        }
    }
    if let Some(start) = zone_start {
        lvn_zones.push((
            start,
            levels.last().unwrap().price_units,
        ));
    }

    // Find single dominant peak: max-volume level above HVN
    // cutoff. No prominence filter — for single-peak selection
    // the cutoff already ensures "high volume", and prominence
    // would reject the POC when neighbors are similarly high.
    let mut peak: Option<VolumeNode> = None;
    for (i, level) in levels.iter().enumerate() {
        if volumes[i] >= hvn_cutoff {
            let better = match &peak {
                Some(p) => volumes[i] > p.volume,
                None => true,
            };
            if better {
                peak = Some(VolumeNode {
                    price_units: level.price_units,
                    price: level.price,
                    volume: volumes[i],
                });
            }
        }
    }

    // Find single deepest valley: min-volume level below LVN
    // cutoff that is also a local minimum (both neighbors have
    // strictly higher volume). Skip edge levels — tail levels
    // naturally have low volume from candle distribution and
    // are not meaningful valleys.
    let mut valley: Option<VolumeNode> = None;
    for i in 1..volumes.len() - 1 {
        let vol = volumes[i];
        if vol <= lvn_cutoff
            && vol > 0.0
            && vol < volumes[i - 1]
            && vol < volumes[i + 1]
        {
            let better = match &valley {
                Some(v) => vol < v.volume,
                None => true,
            };
            if better {
                valley = Some(VolumeNode {
                    price_units: levels[i].price_units,
                    price: levels[i].price,
                    volume: vol,
                });
            }
        }
    }

    (hvn_zones, lvn_zones, peak, valley)
}

/// Compute the Nth percentile value from a set of volumes.
fn percentile_value(volumes: &[f32], pct: f32) -> f32 {
    let mut working: Vec<f32> = volumes.to_vec();
    let idx = ((pct * (working.len() - 1) as f32) as usize)
        .min(working.len() - 1);
    working.select_nth_unstable_by(idx, |a, b| {
        a.partial_cmp(b).unwrap()
    });
    working[idx]
}

/// Prominence for HVN: ratio of this level's volume to its nearest
/// neighbors. A peak surrounded by lower values has high prominence.
fn prominence(idx: usize, volumes: &[f32]) -> f32 {
    let vol = volumes[idx];
    if vol <= 0.0 {
        return 0.0;
    }
    let left = if idx > 0 { volumes[idx - 1] } else { vol };
    let right =
        if idx + 1 < volumes.len() { volumes[idx + 1] } else { vol };
    let neighbor_max = left.max(right);
    if neighbor_max <= 0.0 {
        return 1.0;
    }
    // How much this level exceeds its neighbors (0.0 = same, 1.0 = double)
    ((vol - neighbor_max) / neighbor_max).max(0.0)
}

/// Prominence for LVN: ratio of how much lower this level is compared
/// to its neighbors. A valley surrounded by higher values has high
/// prominence.
fn prominence_inverse(idx: usize, volumes: &[f32]) -> f32 {
    let vol = volumes[idx];
    let left = if idx > 0 { volumes[idx - 1] } else { vol };
    let right =
        if idx + 1 < volumes.len() { volumes[idx + 1] } else { vol };
    let neighbor_min = left.min(right);
    if neighbor_min <= 0.0 || vol <= 0.0 {
        return 0.0;
    }
    // How much neighbors exceed this level (0.0 = same, 1.0 = double)
    ((neighbor_min - vol) / vol).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use data::{Candle, Timestamp, Volume};

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
    fn test_build_profile_from_candles() {
        let candles = vec![
            make_candle(1000, 100.0, 102.0, 99.0, 101.0, 100.0, 50.0),
            make_candle(2000, 101.0, 103.0, 100.0, 102.0, 80.0, 60.0),
        ];
        let tick_size = Price::from_f32(1.0);
        let levels = build_profile_from_candles(
            &candles,
            tick_size,
            tick_size.units(),
        );

        assert!(!levels.is_empty());
        assert!(levels.len() <= 5);
        for level in &levels {
            assert!(level.buy_volume >= 0.0);
            assert!(level.sell_volume >= 0.0);
        }
    }

    #[test]
    fn test_find_poc() {
        let levels = vec![
            ProfileLevel {
                price: 99.0,
                price_units: Price::from_f64(99.0).units(),
                buy_volume: 10.0,
                sell_volume: 5.0,
            },
            ProfileLevel {
                price: 100.0,
                price_units: Price::from_f64(100.0).units(),
                buy_volume: 50.0,
                sell_volume: 40.0,
            },
            ProfileLevel {
                price: 101.0,
                price_units: Price::from_f64(101.0).units(),
                buy_volume: 20.0,
                sell_volume: 10.0,
            },
        ];

        let poc = find_poc_index(&levels);
        assert_eq!(poc, Some(1));
    }

    #[test]
    fn test_value_area() {
        let levels = vec![
            ProfileLevel {
                price: 98.0,
                price_units: Price::from_f64(98.0).units(),
                buy_volume: 5.0,
                sell_volume: 5.0,
            },
            ProfileLevel {
                price: 99.0,
                price_units: Price::from_f64(99.0).units(),
                buy_volume: 20.0,
                sell_volume: 10.0,
            },
            ProfileLevel {
                price: 100.0,
                price_units: Price::from_f64(100.0).units(),
                buy_volume: 50.0,
                sell_volume: 40.0,
            },
            ProfileLevel {
                price: 101.0,
                price_units: Price::from_f64(101.0).units(),
                buy_volume: 15.0,
                sell_volume: 15.0,
            },
            ProfileLevel {
                price: 102.0,
                price_units: Price::from_f64(102.0).units(),
                buy_volume: 5.0,
                sell_volume: 5.0,
            },
        ];

        let poc_idx = 2;
        let va = calculate_value_area(&levels, poc_idx, 0.7);
        assert!(va.is_some());

        let (vah, val) = va.unwrap();
        assert!(vah >= poc_idx);
        assert!(val <= poc_idx);
    }

    // ── detect_volume_nodes tests ────────────────────────────────

    fn make_profile_levels(volumes: &[(f64, f32)]) -> Vec<ProfileLevel> {
        volumes
            .iter()
            .enumerate()
            .map(|(i, &(price, vol))| ProfileLevel {
                price,
                price_units: Price::from_f64(price).units(),
                buy_volume: vol * 0.6,
                sell_volume: vol * 0.4,
            })
            .collect()
    }

    #[test]
    fn test_detect_nodes_known_peaks() {
        use crate::output::NodeDetectionMethod;
        // Profile: low, low, HIGH, low, low, HIGH, low
        let levels = make_profile_levels(&[
            (98.0, 10.0),
            (99.0, 15.0),
            (100.0, 100.0), // HVN
            (101.0, 12.0),
            (102.0, 8.0),   // LVN
            (103.0, 90.0),  // HVN
            (104.0, 20.0),
        ]);

        let (hvn, lvn) = detect_volume_nodes(
            &levels,
            NodeDetectionMethod::Relative,
            0.8,
            NodeDetectionMethod::Relative,
            0.15,
            0.0,
        );

        // HVN at 100 and 103
        assert!(hvn.len() >= 2);
        let hvn_prices: Vec<f64> =
            hvn.iter().map(|n| n.price).collect();
        assert!(hvn_prices.contains(&100.0));
        assert!(hvn_prices.contains(&103.0));

        // LVN at 102 (8.0 <= 0.15 * 100 = 15.0)
        assert!(!lvn.is_empty());
        let lvn_prices: Vec<f64> =
            lvn.iter().map(|n| n.price).collect();
        assert!(lvn_prices.contains(&102.0));
    }

    #[test]
    fn test_detect_nodes_empty() {
        use crate::output::NodeDetectionMethod;
        let (hvn, lvn) = detect_volume_nodes(
            &[],
            NodeDetectionMethod::Percentile,
            0.9,
            NodeDetectionMethod::Percentile,
            0.1,
            0.0,
        );
        assert!(hvn.is_empty());
        assert!(lvn.is_empty());
    }

    #[test]
    fn test_detect_nodes_single_level() {
        use crate::output::NodeDetectionMethod;
        let levels = make_profile_levels(&[(100.0, 50.0)]);
        let (hvn, lvn) = detect_volume_nodes(
            &levels,
            NodeDetectionMethod::Relative,
            0.8,
            NodeDetectionMethod::Relative,
            0.2,
            0.0,
        );
        assert!(hvn.is_empty());
        assert!(lvn.is_empty());
    }

    #[test]
    fn test_detect_nodes_flat_distribution() {
        use crate::output::NodeDetectionMethod;
        // All levels have the same volume — no peaks or valleys
        let levels = make_profile_levels(&[
            (98.0, 50.0),
            (99.0, 50.0),
            (100.0, 50.0),
            (101.0, 50.0),
            (102.0, 50.0),
        ]);
        let (hvn, lvn) = detect_volume_nodes(
            &levels,
            NodeDetectionMethod::StdDev,
            1.0,
            NodeDetectionMethod::StdDev,
            1.0,
            0.0,
        );
        // StdDev of flat = 0, so cutoff = mean ± 0 = mean
        // No level exceeds mean+0, and no level is below mean-0 > 0
        assert!(hvn.is_empty());
        assert!(lvn.is_empty());
    }

    #[test]
    fn test_detect_nodes_prominence_filtering() {
        use crate::output::NodeDetectionMethod;
        // Slight peak that won't pass high prominence threshold
        let levels = make_profile_levels(&[
            (98.0, 45.0),
            (99.0, 50.0),  // barely higher than neighbors
            (100.0, 48.0),
            (101.0, 100.0), // real peak
            (102.0, 20.0),
        ]);

        // With high prominence requirement
        let (hvn, _) = detect_volume_nodes(
            &levels,
            NodeDetectionMethod::Relative,
            0.4,
            NodeDetectionMethod::Relative,
            0.2,
            0.5, // requires 50% prominence over neighbors
        );

        // Only the real peak at 101 should survive
        let hvn_prices: Vec<f64> =
            hvn.iter().map(|n| n.price).collect();
        assert!(hvn_prices.contains(&101.0));
        assert!(!hvn_prices.contains(&99.0));
    }

    // ── detect_volume_zones tests ────────────────────────────────

    #[test]
    fn test_detect_zones_two_clusters() {
        use crate::output::NodeDetectionMethod;
        // Two HVN clusters separated by a low-volume gap
        let levels = make_profile_levels(&[
            (98.0, 80.0),  // HVN cluster 1
            (99.0, 90.0),  // HVN cluster 1
            (100.0, 85.0), // HVN cluster 1
            (101.0, 10.0), // gap
            (102.0, 12.0), // gap
            (103.0, 88.0), // HVN cluster 2
            (104.0, 95.0), // HVN cluster 2
        ]);

        let (hvn_zones, _, _, _) = detect_volume_zones(
            &levels,
            NodeDetectionMethod::Relative,
            0.8,
            NodeDetectionMethod::Relative,
            0.15,
            0.0,
        );

        assert_eq!(
            hvn_zones.len(),
            2,
            "Should find two HVN zones"
        );
        // First zone covers 98-100
        assert_eq!(
            hvn_zones[0].0,
            Price::from_f64(98.0).units()
        );
        assert_eq!(
            hvn_zones[0].1,
            Price::from_f64(100.0).units()
        );
        // Second zone covers 103-104
        assert_eq!(
            hvn_zones[1].0,
            Price::from_f64(103.0).units()
        );
        assert_eq!(
            hvn_zones[1].1,
            Price::from_f64(104.0).units()
        );
    }

    #[test]
    fn test_detect_zones_single_level() {
        use crate::output::NodeDetectionMethod;
        // Single qualifying HVN level forms a zone of width 1
        let levels = make_profile_levels(&[
            (98.0, 10.0),
            (99.0, 10.0),
            (100.0, 100.0), // single HVN
            (101.0, 10.0),
            (102.0, 10.0),
        ]);

        let (hvn_zones, _, _, _) = detect_volume_zones(
            &levels,
            NodeDetectionMethod::Relative,
            0.9,
            NodeDetectionMethod::Relative,
            0.15,
            0.0,
        );

        assert_eq!(hvn_zones.len(), 1);
        // Zone start == zone end (single level)
        assert_eq!(hvn_zones[0].0, hvn_zones[0].1);
    }

    #[test]
    fn test_detect_zones_peak_selects_max_volume() {
        use crate::output::NodeDetectionMethod;
        // Multiple HVN-qualifying levels — peak is max volume
        let levels = make_profile_levels(&[
            (98.0, 78.0),  // above cutoff
            (99.0, 80.0),  // above cutoff
            (100.0, 79.0), // above cutoff
            (101.0, 20.0),
            (102.0, 100.0), // dominant peak (max volume)
            (103.0, 20.0),
        ]);

        let (_, _, peak, _) = detect_volume_zones(
            &levels,
            NodeDetectionMethod::Relative,
            0.75,
            NodeDetectionMethod::Relative,
            0.15,
            0.3,
        );

        assert!(peak.is_some());
        let peak = peak.unwrap();
        assert_eq!(peak.price, 102.0);
        assert_eq!(peak.volume, 100.0);
    }

    #[test]
    fn test_detect_zones_valley_selection() {
        use crate::output::NodeDetectionMethod;
        // Valley is the min-volume local minimum (both neighbors
        // higher) among LVN-qualifying interior levels
        let levels = make_profile_levels(&[
            (98.0, 50.0),
            (99.0, 8.0),  // local min, LVN
            (100.0, 80.0),
            (101.0, 5.0), // local min, deepest valley
            (102.0, 70.0),
            (103.0, 12.0), // local min, LVN
            (104.0, 60.0),
        ]);

        let (_, _, _, valley) = detect_volume_zones(
            &levels,
            NodeDetectionMethod::Relative,
            0.8,
            NodeDetectionMethod::Relative,
            0.2,
            0.0,
        );

        assert!(valley.is_some());
        let valley = valley.unwrap();
        assert_eq!(valley.price, 101.0);
        assert_eq!(valley.volume, 5.0);
    }

    #[test]
    fn test_detect_zones_valley_rejects_tails() {
        use crate::output::NodeDetectionMethod;
        // Bell-curve profile: tails have lowest volume but are
        // NOT local minima (monotonically decreasing).
        // No interior local minimum exists → no valley.
        let levels = make_profile_levels(&[
            (97.0, 3.0),   // tail
            (98.0, 8.0),   // tail
            (99.0, 30.0),
            (100.0, 80.0),
            (101.0, 100.0), // peak
            (102.0, 70.0),
            (103.0, 25.0),
            (104.0, 6.0),  // tail
            (105.0, 2.0),  // tail
        ]);

        let (_, _, _, valley) = detect_volume_zones(
            &levels,
            NodeDetectionMethod::Relative,
            0.8,
            NodeDetectionMethod::Relative,
            0.15,
            0.0,
        );

        // No interior local minimum exists — valley should be
        // None (tails are excluded by local-min requirement)
        assert!(
            valley.is_none(),
            "Tail levels should not be selected as valleys"
        );
    }

    #[test]
    fn test_detect_zones_empty() {
        use crate::output::NodeDetectionMethod;
        let (hvn_z, lvn_z, peak, valley) = detect_volume_zones(
            &[],
            NodeDetectionMethod::Percentile,
            0.9,
            NodeDetectionMethod::Percentile,
            0.1,
            0.0,
        );
        assert!(hvn_z.is_empty());
        assert!(lvn_z.is_empty());
        assert!(peak.is_none());
        assert!(valley.is_none());
    }
}
