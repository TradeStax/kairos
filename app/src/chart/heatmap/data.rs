//! Heatmap data structures and aggregation logic
//!
//! This module contains the internal data structures for organizing
//! depth snapshots and trades into efficient lookup structures.

use data::{ChartBasis, Depth as DepthSnapshot, Price as DataPrice, Trade as DomainTrade};
use std::collections::BTreeMap;

/// A single depth run - continuous orderbook presence at a price level
#[derive(Debug, Clone)]
pub struct DepthRun {
    /// Start time of depth presence
    pub start_time: u64,
    /// End time of depth presence
    pub until_time: u64,
    /// Quantity at this price level
    pub qty: f32,
    /// True if bid, false if ask
    pub is_bid: bool,
}

impl DepthRun {
    pub fn qty(&self) -> f32 {
        self.qty
    }
}

/// Trade data point for a time bucket
#[derive(Debug, Clone)]
pub struct TradeDataPoint {
    /// Grouped trades by (price_units, is_sell) -> qty
    /// BTreeMap for O(log M) lookup instead of O(M) linear scan
    pub grouped_trades: BTreeMap<(i64, bool), f32>,
    /// Total buy volume in bucket
    pub buy_volume: f32,
    /// Total sell volume in bucket
    pub sell_volume: f32,
}

impl Default for TradeDataPoint {
    fn default() -> Self {
        Self {
            grouped_trades: BTreeMap::new(),
            buy_volume: 0.0,
            sell_volume: 0.0,
        }
    }
}

impl TradeDataPoint {
    /// Iterate grouped trades as (price, qty, is_sell) tuples
    pub fn iter_trades(&self) -> impl Iterator<Item = (DataPrice, f32, bool)> + '_ {
        self.grouped_trades
            .iter()
            .map(|(&(price_units, is_sell), &qty)| {
                (DataPrice::from_units(price_units), qty, is_sell)
            })
    }
}

/// Aggregated heatmap data structure
///
/// This structure organizes historical depth snapshots and trades
/// into efficient lookup structures for rendering.
pub struct HeatmapData {
    /// Price level -> depth runs over time
    /// Key: price in units (i64), Value: list of depth runs
    pub depth_by_price: BTreeMap<i64, Vec<DepthRun>>,

    /// Time bucket -> aggregated trades
    /// Key: bucket time (u64), Value: trade data
    pub trades_by_time: BTreeMap<u64, TradeDataPoint>,

    /// Latest depth snapshot timestamp
    pub latest_depth_time: u64,

    /// Candle timestamp lookup for tick-based charts
    /// Key: candle start time, Value: candle end time
    candle_time_map: Option<BTreeMap<u64, u64>>,
}

impl HeatmapData {
    /// Create empty heatmap data
    pub fn new() -> Self {
        Self {
            depth_by_price: BTreeMap::new(),
            trades_by_time: BTreeMap::new(),
            latest_depth_time: 0,
            candle_time_map: None,
        }
    }

    /// Create heatmap data with candle time map for tick-based charts
    pub fn new_with_candles(candles: &[data::Candle], basis: ChartBasis) -> Self {
        let candle_time_map = match basis {
            ChartBasis::Tick(_) => {
                // Build map of candle timestamps for tick-based bucketing
                let mut map = BTreeMap::new();
                for (idx, candle) in candles.iter().enumerate() {
                    let candle_start = candle.time.to_millis();
                    let candle_end = if idx + 1 < candles.len() {
                        candles[idx + 1].time.to_millis()
                    } else {
                        candle_start + 1000 // Default 1 second for last candle
                    };
                    map.insert(candle_start, candle_end);
                }
                Some(map)
            }
            ChartBasis::Time(_) => None, // Time-based uses simple floor division
        };

        Self {
            depth_by_price: BTreeMap::new(),
            trades_by_time: BTreeMap::new(),
            latest_depth_time: 0,
            candle_time_map,
        }
    }

    /// Add a depth snapshot to the heatmap
    ///
    /// This processes the snapshot into depth runs, bucketing by the chart basis.
    /// Each price level gets a run for the duration of the bucket.
    pub fn add_depth_snapshot(
        &mut self,
        snapshot: &DepthSnapshot,
        basis: ChartBasis,
        _tick_size: DataPrice,
    ) {
        let time = snapshot.time;
        let bucket_time = self.bucket_time(time, basis);
        let bucket_duration = self.bucket_duration(bucket_time, basis);

        let new_until = bucket_time + bucket_duration;

        // Process bids (buy orders)
        for (price, qty) in &snapshot.bids {
            let price_units = *price;
            let qty_f32 = *qty;
            let runs = self.depth_by_price.entry(price_units).or_default();
            // Coalesce with the last run if qty and side match and times are
            // adjacent
            if let Some(last) = runs.last_mut()
                && last.is_bid
                && last.qty == qty_f32
                && last.until_time >= bucket_time
            {
                last.until_time = new_until;
                continue;
            }
            runs.push(DepthRun {
                start_time: bucket_time,
                until_time: new_until,
                qty: qty_f32,
                is_bid: true,
            });
        }

        // Process asks (sell orders)
        for (price, qty) in &snapshot.asks {
            let price_units = *price;
            let qty_f32 = *qty;
            let runs = self.depth_by_price.entry(price_units).or_default();
            // Coalesce with the last run if qty and side match and times are
            // adjacent
            if let Some(last) = runs.last_mut()
                && !last.is_bid
                && last.qty == qty_f32
                && last.until_time >= bucket_time
            {
                last.until_time = new_until;
                continue;
            }
            runs.push(DepthRun {
                start_time: bucket_time,
                until_time: new_until,
                qty: qty_f32,
                is_bid: false,
            });
        }

        self.latest_depth_time = time;
    }

    /// Add a trade to the heatmap
    ///
    /// This aggregates trades into time buckets and groups them by price level.
    pub fn add_trade(&mut self, trade: &DomainTrade, basis: ChartBasis, tick_size: DataPrice) {
        let time = trade.time.to_millis();
        let bucket_time = self.bucket_time(time, basis);

        let entry = self.trades_by_time.entry(bucket_time).or_default();

        let qty = trade.quantity.value() as f32;
        let is_sell = trade.is_sell();

        // Accumulate volume
        if is_sell {
            entry.sell_volume += qty;
        } else {
            entry.buy_volume += qty;
        }

        // Group trades at the same price level — O(log M) BTreeMap lookup
        let price_rounded = trade.price.round_to_step(tick_size);
        *entry
            .grouped_trades
            .entry((price_rounded.units(), is_sell))
            .or_insert(0.0) += qty;
    }

    /// Calculate bucket time based on chart basis
    ///
    /// For time-based charts: Uses timeframe bucketing (floor to interval)
    /// For tick-based charts: Uses candle timestamp from map or falls back to second rounding
    fn bucket_time(&self, time: u64, basis: ChartBasis) -> u64 {
        match basis {
            ChartBasis::Time(timeframe) => {
                let interval = timeframe.to_milliseconds();
                (time / interval) * interval
            }
            ChartBasis::Tick(_) => {
                // Try to find the candle this time belongs to
                if let Some(ref candle_map) = self.candle_time_map {
                    // Find the candle whose time range contains this trade/depth
                    // Use range query to find the greatest key <= time
                    candle_map
                        .range(..=time)
                        .next_back()
                        .and_then(|(candle_start, candle_end)| {
                            if time >= *candle_start && time < *candle_end {
                                Some(*candle_start)
                            } else {
                                None
                            }
                        })
                        .unwrap_or_else(|| {
                            // Fallback: round to nearest second
                            (time / 1000) * 1000
                        })
                } else {
                    // Fallback for when candle map not available
                    (time / 1000) * 1000
                }
            }
        }
    }

    /// Get bucket duration in milliseconds
    ///
    /// For tick-based charts with candle map, returns actual duration to next candle
    fn bucket_duration(&self, time: u64, basis: ChartBasis) -> u64 {
        match basis {
            ChartBasis::Time(timeframe) => timeframe.to_milliseconds(),
            ChartBasis::Tick(_) => {
                // Try to get actual candle duration from map
                if let Some(ref candle_map) = self.candle_time_map {
                    candle_map
                        .range(..=time)
                        .next_back()
                        .map(|(candle_start, candle_end)| candle_end - candle_start)
                        .unwrap_or(1000) // Default 1 second
                } else {
                    1000 // Default 1 second
                }
            }
        }
    }

    /// Iterate over depth runs in a time/price range
    ///
    /// Optimized with viewport culling:
    /// 1. BTreeMap range query for price filtering (O(log n) instead of O(n))
    /// 2. Early rejection of time-out-of-bounds runs
    /// 3. Minimal allocations
    pub fn iter_depth_filtered(
        &self,
        earliest: u64,
        latest: u64,
        highest: DataPrice,
        lowest: DataPrice,
    ) -> impl Iterator<Item = (&i64, &Vec<DepthRun>)> {
        let highest_units = highest.units();
        let lowest_units = lowest.units();

        // OPTIMIZATION: BTreeMap::range() gives us O(log n) price filtering
        self.depth_by_price
            .range(lowest_units..=highest_units)
            .filter(move |(_, runs)| {
                // OPTIMIZATION: Early rejection - check if ANY run intersects time range
                // A run intersects if: run.until_time > earliest AND run.start_time < latest
                runs.iter()
                    .any(|run| run.until_time > earliest && run.start_time < latest)
            })
    }

    /// Get the latest orderbook snapshot at a specific time
    ///
    /// This returns the most recent depth run for each price level.
    pub fn latest_order_runs(
        &self,
        highest: DataPrice,
        lowest: DataPrice,
        latest_time: u64,
    ) -> impl Iterator<Item = (DataPrice, &DepthRun)> {
        let highest_units = highest.units();
        let lowest_units = lowest.units();

        self.depth_by_price
            .range(lowest_units..=highest_units)
            .filter_map(move |(price_units, runs)| {
                runs.iter()
                    .filter(|run| run.start_time <= latest_time && run.until_time >= latest_time)
                    .max_by_key(|run| run.start_time)
                    .map(|run| (DataPrice::from_units(*price_units), run))
            })
    }

    /// Calculate maximum depth quantity in range (for color scaling)
    pub fn max_depth_qty_in_range(
        &self,
        earliest: u64,
        latest: u64,
        highest: DataPrice,
        lowest: DataPrice,
        min_qty_filter: f32,
    ) -> f32 {
        self.iter_depth_filtered(earliest, latest, highest, lowest)
            .flat_map(|(_, runs)| runs.iter())
            .filter(|run| run.qty > min_qty_filter)
            .map(|run| run.qty)
            .fold(f32::MIN, f32::max)
            .max(1.0)
    }

    /// Calculate maximum trade quantity in range (for circle sizing)
    pub fn max_trade_qty_in_range(&self, earliest: u64, latest: u64) -> f32 {
        self.trades_by_time
            .range(earliest..=latest)
            .flat_map(|(_, dp)| dp.grouped_trades.values())
            .copied()
            .fold(f32::MIN, f32::max)
            .max(1.0)
    }

    /// Calculate maximum aggregate volume in range (for volume bar scaling)
    pub fn max_aggr_volume_in_range(&self, earliest: u64, latest: u64) -> f32 {
        self.trades_by_time
            .range(earliest..=latest)
            .map(|(_, dp)| dp.buy_volume + dp.sell_volume)
            .fold(f32::MIN, f32::max)
            .max(1.0)
    }
}

impl HeatmapData {
    /// Clear trade data while preserving depth runs.
    /// Used during replay rebuild to reset trade-related state.
    pub fn clear_trades(&mut self) {
        self.trades_by_time.clear();
        self.candle_time_map = None;
    }
}

impl Default for HeatmapData {
    fn default() -> Self {
        Self::new()
    }
}

/// Quantity scales for rendering
#[derive(Debug, Clone, Copy)]
pub struct QtyScale {
    pub max_trade_qty: f32,
    pub max_aggr_volume: f32,
    pub max_depth_qty: f32,
}

/// Cached volume profile data
#[derive(Debug, Clone)]
pub struct VolumeProfile {
    /// Per-tick (buy_volume, sell_volume) pairs
    pub profile: Vec<(f32, f32)>,
    /// Maximum aggregate volume across all ticks
    pub max_aggr_volume: f32,
    /// First tick price (lowest rounded)
    pub first_tick_units: i64,
    /// Price step in units
    pub step_units: i64,
}

/// Key identifying a specific volume profile computation
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VolumeProfileKey {
    pub earliest: u64,
    pub latest: u64,
    pub first_tick_units: i64,
    pub last_tick_units: i64,
    pub step_units: i64,
}

impl HeatmapData {
    /// Compute a volume profile for the given time and price range.
    pub fn compute_volume_profile(
        &self,
        time_range: std::ops::RangeInclusive<u64>,
        first_tick: DataPrice,
        last_tick: DataPrice,
        step_units: i64,
        num_ticks: usize,
    ) -> VolumeProfile {
        let step_as_price = DataPrice::from_units(step_units);
        let first_tick_units = first_tick.units();
        let last_tick_units = last_tick.units();

        let mut profile = vec![(0.0f32, 0.0f32); num_ticks];
        let mut max_aggr_volume = 0.0f32;

        self.trades_by_time.range(time_range).for_each(|(_, dp)| {
            for (&(price_units, is_sell), &qty) in &dp.grouped_trades {
                let price = DataPrice::from_units(price_units);
                let grouped_price = if is_sell {
                    price.round_to_side_step(true, step_as_price)
                } else {
                    price.round_to_side_step(false, step_as_price)
                };

                let grouped_units = grouped_price.units();
                if grouped_units < first_tick_units || grouped_units > last_tick_units {
                    continue;
                }

                let index = ((grouped_units - first_tick_units) / step_units) as usize;

                if let Some(entry) = profile.get_mut(index) {
                    if is_sell {
                        entry.1 += qty;
                    } else {
                        entry.0 += qty;
                    }
                    max_aggr_volume = max_aggr_volume.max(entry.0 + entry.1);
                }
            }
        });

        VolumeProfile {
            profile,
            max_aggr_volume,
            first_tick_units,
            step_units,
        }
    }
}
