//! Footprint Display Data
//!
//! Pre-computed footprint structures optimized for rendering.

use super::{DisplayData, ViewportBounds};
use crate::chart::perf::lod::LodLevel;
use data::{Candle, ChartBasis, ChartData, Trade};
use exchange::util::{Price, PriceStep};
use std::collections::BTreeMap;

/// Pre-computed footprint display data
///
/// Contains all footprints for visible candles, ready to render
pub struct FootprintDisplayData {
    /// Footprint for each visible candle (candle_index -> footprint)
    pub candle_footprints: Vec<CandleFootprint>,

    /// Total number of trade clusters across all candles
    pub total_clusters: usize,

    /// Price range covered
    pub price_range: (Price, Price),

    /// LOD level used for this data
    pub lod_level: LodLevel,
}

/// Footprint data for a single candle
#[derive(Clone)]
pub struct CandleFootprint {
    /// Candle index in source data
    pub candle_index: usize,

    /// Candle timestamp
    pub time: u64,

    /// Trade clusters by price (price -> volumes)
    pub clusters: BTreeMap<Price, TradeCluster>,

    /// Point of control (price with max volume)
    pub poc: Option<(Price, f32)>,
}

/// Trade cluster at a price level
#[derive(Clone, Default)]
pub struct TradeCluster {
    pub buy_qty: f32,
    pub sell_qty: f32,
}

impl TradeCluster {
    pub fn total_qty(&self) -> f32 {
        self.buy_qty + self.sell_qty
    }

    pub fn delta(&self) -> f32 {
        self.buy_qty - self.sell_qty
    }
}

/// Extra parameters for building footprint display data
pub struct FootprintParams {
    pub tick_size: PriceStep,
    pub interval_ms: u64,
    pub basis: ChartBasis,
}

impl DisplayData for FootprintDisplayData {
    type SourceData = ChartData;
    type ExtraParams = FootprintParams;

    fn build(
        source: &Self::SourceData,
        bounds: &ViewportBounds,
        lod_level: LodLevel,
        params: &Self::ExtraParams,
    ) -> Self {
        let mut candle_footprints = Vec::new();
        let mut total_clusters = 0;

        let highest = Price::from_units(bounds.price_high);
        let lowest = Price::from_units(bounds.price_low);

        // Determine which candles are visible
        let visible_candles: Vec<(usize, &Candle)> = match params.basis {
            ChartBasis::Time(_) => source
                .candles
                .iter()
                .enumerate()
                .filter(|(_, c)| c.time.0 >= bounds.time_start && c.time.0 <= bounds.time_end)
                .collect(),
            ChartBasis::Tick(_) => {
                // For tick basis, use reverse iteration
                let start_idx = bounds.time_start as usize;
                let end_idx = bounds.time_end as usize;

                source
                    .candles
                    .iter()
                    .enumerate()
                    .rev()
                    .filter(|(idx, _)| *idx >= start_idx && *idx <= end_idx)
                    .collect()
            }
        };

        // Build footprint for each visible candle
        for (candle_idx, candle) in visible_candles {
            // Find trades for this candle
            let candle_start = candle.time.0;
            let candle_end = candle_start + params.interval_ms;

            // Binary search for trade range
            let start_idx = source
                .trades
                .binary_search_by_key(&candle_start, |t| t.time.0)
                .unwrap_or_else(|i| i);

            let end_idx = source.trades[start_idx..]
                .binary_search_by_key(&candle_end, |t| t.time.0)
                .map(|i| start_idx + i)
                .unwrap_or_else(|i| start_idx + i);

            let candle_trades = &source.trades[start_idx..end_idx];

            // Build footprint for this candle
            let mut clusters = BTreeMap::new();

            // Apply LOD decimation to trade processing
            let decimation = lod_level.decimation_factor();

            for (trade_idx, trade) in candle_trades.iter().enumerate() {
                // Apply LOD decimation
                if decimation > 1 && trade_idx % decimation != 0 {
                    continue;
                }

                let price_rounded =
                    Price::from_units(trade.price.units()).round_to_step(params.tick_size);

                // Skip trades outside visible price range
                if price_rounded < lowest || price_rounded > highest {
                    continue;
                }

                let cluster = clusters
                    .entry(price_rounded)
                    .or_insert(TradeCluster::default());

                if trade.is_buy() {
                    cluster.buy_qty += trade.quantity.value() as f32;
                } else {
                    cluster.sell_qty += trade.quantity.value() as f32;
                }
            }

            // Find POC (point of control)
            let poc = clusters
                .iter()
                .max_by(|(_, a), (_, b)| a.total_qty().partial_cmp(&b.total_qty()).unwrap())
                .map(|(price, cluster)| (*price, cluster.total_qty()));

            total_clusters += clusters.len();

            candle_footprints.push(CandleFootprint {
                candle_index: candle_idx,
                time: candle.time.0,
                clusters,
                poc,
            });
        }

        FootprintDisplayData {
            candle_footprints,
            total_clusters,
            price_range: (lowest, highest),
            lod_level,
        }
    }

    fn memory_usage(&self) -> usize {
        let footprints_size = self.candle_footprints.len() * std::mem::size_of::<CandleFootprint>();
        let clusters_size = self.total_clusters * std::mem::size_of::<TradeCluster>();
        footprints_size + clusters_size
    }

    fn is_empty(&self) -> bool {
        self.candle_footprints.is_empty()
    }
}

impl FootprintDisplayData {
    /// Get maximum cluster quantity across all candles
    pub fn max_cluster_qty(&self, cluster_kind: data::ClusterKind) -> f32 {
        use data::ClusterKind;

        self.candle_footprints
            .iter()
            .flat_map(|cf| cf.clusters.values())
            .map(|cluster| match cluster_kind {
                ClusterKind::BidAsk => cluster.buy_qty.max(cluster.sell_qty),
                ClusterKind::Delta | ClusterKind::DeltaProfile => cluster.delta().abs(),
                ClusterKind::Volume | ClusterKind::VolumeProfile | ClusterKind::Trades => {
                    cluster.total_qty()
                }
            })
            .fold(0.0_f32, f32::max)
            .max(1.0) // Prevent division by zero
    }

    /// Get footprint for a specific candle
    pub fn get_candle_footprint(&self, candle_index: usize) -> Option<&CandleFootprint> {
        self.candle_footprints
            .iter()
            .find(|cf| cf.candle_index == candle_index)
    }

    /// Iterate over all candle footprints
    pub fn iter(&self) -> impl Iterator<Item = &CandleFootprint> {
        self.candle_footprints.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::{DisplayCacheKey, DisplayDataCache};
    use data::{Price as DataPrice, Quantity, Side, Timestamp, Volume};

    fn create_test_chart_data() -> ChartData {
        let trades = vec![
            Trade::new(
                Timestamp(1000),
                DataPrice::from_f32(100.0),
                Quantity(10.0),
                Side::Buy,
            ),
            Trade::new(
                Timestamp(1500),
                DataPrice::from_f32(101.0),
                Quantity(5.0),
                Side::Sell,
            ),
            Trade::new(
                Timestamp(2000),
                DataPrice::from_f32(100.5),
                Quantity(8.0),
                Side::Buy,
            ),
        ];

        let candles = vec![Candle::new(
            Timestamp(0),
            DataPrice::from_f32(100.0),
            DataPrice::from_f32(101.0),
            DataPrice::from_f32(99.5),
            DataPrice::from_f32(100.5),
            Volume(18.0),
            Volume(5.0),
        )];

        ChartData::from_trades(trades, candles)
    }

    #[test]
    fn test_footprint_display_data_build() {
        let chart_data = create_test_chart_data();
        let bounds = ViewportBounds::new((0, 3000), (9900, 10200));
        let params = FootprintParams {
            tick_size: PriceStep::from_f32(0.25),
            interval_ms: 60_000,
            basis: ChartBasis::Time(data::Timeframe::M1),
        };

        let display_data =
            FootprintDisplayData::build(&chart_data, &bounds, LodLevel::High, &params);

        assert!(!display_data.is_empty());
        assert_eq!(display_data.candle_footprints.len(), 1);
        assert!(display_data.total_clusters > 0);
    }

    #[test]
    fn test_display_cache() {
        let chart_data = create_test_chart_data();
        let bounds = ViewportBounds::new((0, 3000), (9900, 10200));
        let params = FootprintParams {
            tick_size: PriceStep::from_f32(0.25),
            interval_ms: 60_000,
            basis: ChartBasis::Time(data::Timeframe::M1),
        };

        let mut cache = DisplayDataCache::<FootprintDisplayData>::new();

        let key =
            DisplayCacheKey::from_viewport(params.basis, &bounds, LodLevel::High, 1.0, 4.0, 4.0);

        // First access - cache miss
        let _data1 = cache.get_or_build(key.clone(), &chart_data, &bounds, LodLevel::High, &params);
        assert_eq!(cache.stats().misses, 1);
        assert_eq!(cache.stats().hits, 0);

        // Second access - cache hit
        let _data2 = cache.get_or_build(key.clone(), &chart_data, &bounds, LodLevel::High, &params);
        assert_eq!(cache.stats().misses, 1);
        assert_eq!(cache.stats().hits, 1);
    }
}
