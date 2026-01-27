//! Heatmap Display Data
//!
//! Pre-computed heatmap structures optimized for rendering.

use super::{DisplayData, ViewportBounds};
use crate::chart::lod::LodLevel;
use data::{ChartBasis, ChartData, DepthSnapshot, Price as DataPrice, Trade};
use std::collections::BTreeMap;

/// Pre-computed heatmap display data
pub struct HeatmapDisplayData {
    /// Depth rectangles ready to render (price, time_start, time_end, quantity, is_bid)
    pub depth_rects: Vec<DepthRect>,

    /// Trade markers ready to render (time, price, quantity, is_sell)
    pub trade_markers: Vec<TradeMarker>,

    /// Maximum depth quantity (for color scaling)
    pub max_depth_qty: f32,

    /// Maximum trade quantity (for sizing)
    pub max_trade_qty: f32,

    /// LOD level used
    pub lod_level: LodLevel,
}

/// Depth rectangle for rendering
#[derive(Debug, Clone)]
pub struct DepthRect {
    pub price_units: i64,
    pub time_start: u64,
    pub time_end: u64,
    pub quantity: f32,
    pub is_bid: bool,
}

/// Trade marker for rendering
#[derive(Debug, Clone)]
pub struct TradeMarker {
    pub time: u64,
    pub price_units: i64,
    pub quantity: f32,
    pub is_sell: bool,
}

/// Extra parameters for building heatmap display data
pub struct HeatmapParams {
    pub basis: ChartBasis,
    pub tick_size: DataPrice,
    pub order_size_filter: f32,
    pub trade_size_filter: f32,
    pub max_trade_markers: usize,
}

impl DisplayData for HeatmapDisplayData {
    type SourceData = ChartData;
    type ExtraParams = HeatmapParams;

    fn build(
        source: &Self::SourceData,
        bounds: &ViewportBounds,
        lod_level: LodLevel,
        params: &Self::ExtraParams,
    ) -> Self {
        let mut depth_rects = Vec::new();
        let mut trade_markers = Vec::new();
        let mut max_depth_qty = 0.0_f32;
        let mut max_trade_qty = 0.0_f32;

        // Build depth rectangles from snapshots
        if let Some(depth_snapshots) = &source.depth_snapshots {
            let decimation = lod_level.decimation_factor();

            for (snapshot_idx, snapshot) in depth_snapshots.iter().enumerate() {
                // Apply LOD decimation to snapshots
                if decimation > 1 && snapshot_idx % decimation != 0 {
                    continue;
                }

                let time = snapshot.time.to_millis();

                // Skip snapshots outside viewport
                if time < bounds.time_start || time > bounds.time_end {
                    continue;
                }

                // Calculate bucket time and duration
                let bucket_time = calculate_bucket_time(time, params.basis);
                let bucket_duration = calculate_bucket_duration(params.basis);

                // Process bids
                for (price, qty) in &snapshot.bids {
                    let qty_value = qty.value() as f32;

                    if qty_value <= params.order_size_filter {
                        continue;
                    }

                    let price_units = price.to_units();

                    // Skip prices outside viewport
                    if price_units < bounds.price_low || price_units > bounds.price_high {
                        continue;
                    }

                    depth_rects.push(DepthRect {
                        price_units,
                        time_start: bucket_time,
                        time_end: bucket_time + bucket_duration,
                        quantity: qty_value,
                        is_bid: true,
                    });

                    max_depth_qty = max_depth_qty.max(qty_value);
                }

                // Process asks
                for (price, qty) in &snapshot.asks {
                    let qty_value = qty.value() as f32;

                    if qty_value <= params.order_size_filter {
                        continue;
                    }

                    let price_units = price.to_units();

                    // Skip prices outside viewport
                    if price_units < bounds.price_low || price_units > bounds.price_high {
                        continue;
                    }

                    depth_rects.push(DepthRect {
                        price_units,
                        time_start: bucket_time,
                        time_end: bucket_time + bucket_duration,
                        quantity: qty_value,
                        is_bid: false,
                    });

                    max_depth_qty = max_depth_qty.max(qty_value);
                }
            }
        }

        // Build trade markers
        let decimation = lod_level.decimation_factor();
        let max_markers = params.max_trade_markers.min(lod_level.max_render_count());

        let mut trade_index = 0;
        let mut rendered_count = 0;

        for trade in &source.trades {
            // Apply LOD decimation
            if decimation > 1 && trade_index % decimation != 0 {
                trade_index += 1;
                continue;
            }

            // Enforce render budget
            if rendered_count >= max_markers {
                break;
            }

            let time = trade.time.0;

            // Skip trades outside viewport time range
            if time < bounds.time_start || time > bounds.time_end {
                trade_index += 1;
                continue;
            }

            let price_units = trade.price.to_units();

            // Skip trades outside viewport price range
            if price_units < bounds.price_low || price_units > bounds.price_high {
                trade_index += 1;
                continue;
            }

            let qty = trade.quantity.value() as f32;

            // Skip small trades
            if qty <= params.trade_size_filter {
                trade_index += 1;
                continue;
            }

            trade_markers.push(TradeMarker {
                time,
                price_units,
                quantity: qty,
                is_sell: trade.is_sell(),
            });

            max_trade_qty = max_trade_qty.max(qty);
            rendered_count += 1;
            trade_index += 1;
        }

        HeatmapDisplayData {
            depth_rects,
            trade_markers,
            max_depth_qty: max_depth_qty.max(1.0),
            max_trade_qty: max_trade_qty.max(1.0),
            lod_level,
        }
    }

    fn memory_usage(&self) -> usize {
        let depth_size = self.depth_rects.len() * std::mem::size_of::<DepthRect>();
        let trade_size = self.trade_markers.len() * std::mem::size_of::<TradeMarker>();
        depth_size + trade_size
    }

    fn is_empty(&self) -> bool {
        self.depth_rects.is_empty() && self.trade_markers.is_empty()
    }
}

/// Calculate bucket time for heatmap
fn calculate_bucket_time(time: u64, basis: ChartBasis) -> u64 {
    match basis {
        ChartBasis::Time(timeframe) => {
            let interval = timeframe.to_millis();
            (time / interval) * interval
        }
        ChartBasis::Tick(_) => {
            // Round to nearest second for tick basis
            (time / 1000) * 1000
        }
    }
}

/// Calculate bucket duration for heatmap
fn calculate_bucket_duration(basis: ChartBasis) -> u64 {
    match basis {
        ChartBasis::Time(timeframe) => timeframe.to_millis(),
        ChartBasis::Tick(_) => 1000, // 1 second default
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use data::{Quantity, Side, Timestamp, Volume};

    fn create_test_data() -> ChartData {
        let trades = vec![
            Trade::new(
                Timestamp(1000),
                DataPrice::from_f32(100.0),
                Quantity(10.0),
                Side::Buy,
            ),
            Trade::new(
                Timestamp(2000),
                DataPrice::from_f32(101.0),
                Quantity(15.0),
                Side::Sell,
            ),
        ];

        let candles = vec![data::Candle::new(
            Timestamp(0),
            DataPrice::from_f32(100.0),
            DataPrice::from_f32(101.0),
            DataPrice::from_f32(99.5),
            DataPrice::from_f32(100.5),
            Volume(10.0),
            Volume(15.0),
        )];

        ChartData::from_trades(trades, candles)
    }

    #[test]
    fn test_heatmap_display_data() {
        let chart_data = create_test_data();
        let bounds = ViewportBounds::new((0, 3000), (9900, 10200));
        let params = HeatmapParams {
            basis: ChartBasis::Time(data::Timeframe::M1),
            tick_size: DataPrice::from_f32(0.25),
            order_size_filter: 0.0,
            trade_size_filter: 0.0,
            max_trade_markers: 1000,
        };

        let display = HeatmapDisplayData::build(&chart_data, &bounds, LodLevel::High, &params);

        assert!(!display.is_empty());
        assert_eq!(display.trade_markers.len(), 2);
        assert!(display.max_trade_qty > 0.0);
    }

    #[test]
    fn test_lod_decimation() {
        let chart_data = create_test_data();
        let bounds = ViewportBounds::new((0, 3000), (9900, 10200));
        let params = HeatmapParams {
            basis: ChartBasis::Time(data::Timeframe::M1),
            tick_size: DataPrice::from_f32(0.25),
            order_size_filter: 0.0,
            trade_size_filter: 0.0,
            max_trade_markers: 1000,
        };

        // High LOD - all trades
        let high_lod = HeatmapDisplayData::build(&chart_data, &bounds, LodLevel::High, &params);

        // Low LOD - decimated trades
        let low_lod = HeatmapDisplayData::build(&chart_data, &bounds, LodLevel::Low, &params);

        // Low LOD should have fewer or equal markers
        assert!(low_lod.trade_markers.len() <= high_lod.trade_markers.len());
    }
}
