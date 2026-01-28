//! Naked Point of Control (nPOC) Study
//!
//! Naked POCs are Points of Control that haven't been revisited by price
//! since they were formed. They often act as support/resistance levels.

use super::poc::find_poc_from_trades;
use crate::style;
use data::{Candle, ChartBasis, ClusterKind, Trade};
use exchange::util::{Price, PriceStep};
use iced::theme::palette::Extended;
use iced::widget::canvas::Frame;
use iced::{Point, Size};
use std::collections::BTreeMap;

/// nPOC configuration
#[derive(Debug, Clone, Copy)]
pub struct NpocConfig {
    /// Number of candles to look ahead for revisitation
    pub lookback: usize,
    /// Line height in pixels
    pub line_height: f32,
    /// Line alpha
    pub alpha: f32,
}

impl Default for NpocConfig {
    fn default() -> Self {
        Self {
            lookback: 100,
            line_height: 2.0,
            alpha: 0.5,
        }
    }
}

/// Content gaps for layout calculations
#[derive(Clone, Copy, Debug)]
pub struct ContentGaps {
    /// Space between imb. markers and candle body
    pub marker_to_candle: f32,
    /// Space between candle body and clusters
    pub candle_to_cluster: f32,
    /// Inner space reserved between imb. markers and clusters
    pub marker_to_bars: f32,
}

impl ContentGaps {
    /// Create gaps based on candle width and scaling
    pub fn from_view(candle_width: f32, scaling: f32) -> Self {
        let px = |p: f32| p / scaling;
        let base = (candle_width * 0.2).max(px(2.0));
        Self {
            marker_to_candle: base,
            candle_to_cluster: base,
            marker_to_bars: px(2.0),
        }
    }
}

/// Draw all naked POCs on the chart
#[allow(clippy::too_many_arguments)]
pub fn draw_npocs(
    candles: &[Candle],
    trades: &[Trade],
    basis: &ChartBasis,
    frame: &mut Frame,
    price_to_y: &impl Fn(Price) -> f32,
    interval_to_x: &impl Fn(u64) -> f32,
    candle_width: f32,
    cell_width: f32,
    cell_height: f32,
    tick_size: PriceStep,
    palette: &Extended,
    lookback: usize,
    _visible_earliest: u64,
    visible_latest: u64,
    cluster_kind: ClusterKind,
    spacing: ContentGaps,
    imb_study_on: bool,
) {
    // Calculate POCs for all candles
    let mut pocs: Vec<(usize, Price, f32)> = Vec::new();

    for (idx, candle) in candles.iter().enumerate() {
        let candle_start = candle.time.0;
        let candle_end = if idx + 1 < candles.len() {
            candles[idx + 1].time.0
        } else {
            candle.time.0 + 60000 // default 1 minute
        };

        // Find trades in candle range using binary search
        let start_idx = trades
            .binary_search_by_key(&candle_start, |t| t.time.0)
            .unwrap_or_else(|i| i);

        let end_idx = trades[start_idx..]
            .binary_search_by_key(&candle_end, |t| t.time.0)
            .map(|i| start_idx + i)
            .unwrap_or_else(|i| start_idx + i);

        let candle_trades = &trades[start_idx..end_idx];

        // Build volume profile for this candle
        let mut volume_profile: BTreeMap<Price, f32> = BTreeMap::new();
        for trade in candle_trades {
            let price_rounded = Price::from_units(trade.price.units()).round_to_step(tick_size);
            *volume_profile.entry(price_rounded).or_insert(0.0) += trade.quantity.0 as f32;
        }

        // Find POC
        if let Some((poc_price, poc_volume)) = volume_profile
            .iter()
            .max_by(|(_, v1), (_, v2)| v1.partial_cmp(v2).unwrap())
        {
            pocs.push((idx, *poc_price, *poc_volume));
        }
    }

    // Track naked POCs
    let mut npocs: Vec<(usize, Price)> = Vec::new();

    for (idx, poc_price, _) in &pocs {
        let mut is_naked = true;

        // Check if price was revisited in next `lookback` candles
        for future_idx in (idx + 1)..(idx + 1 + lookback).min(candles.len()) {
            let future_candle = &candles[future_idx];

            let future_low = Price::from_units(future_candle.low.units());
            let future_high = Price::from_units(future_candle.high.units());

            if *poc_price >= future_low && *poc_price <= future_high {
                is_naked = false;
                break;
            }
        }

        if is_naked {
            npocs.push((*idx, *poc_price));
        }
    }

    // Draw nPOC lines
    let naked_color = if palette.is_dark {
        palette.warning.weak.color.scale_alpha(0.5)
    } else {
        palette.warning.strong.color
    };

    let line_height = cell_height.min(2.0);
    let bar_width_factor: f32 = 0.9;
    let inset = (cell_width * (1.0 - bar_width_factor)) / 2.0;

    let candle_lane_factor: f32 = match cluster_kind {
        ClusterKind::VolumeProfile | ClusterKind::DeltaProfile => 0.25,
        ClusterKind::BidAsk | ClusterKind::Delta | ClusterKind::Volume | ClusterKind::Trades => 1.0,
    };

    let start_x_for = |cell_center_x: f32| -> f32 {
        match cluster_kind {
            ClusterKind::BidAsk | ClusterKind::Delta | ClusterKind::Volume | ClusterKind::Trades => {
                cell_center_x + (candle_width / 2.0) + spacing.candle_to_cluster
            }
            ClusterKind::VolumeProfile | ClusterKind::DeltaProfile => {
                let content_left = (cell_center_x - (cell_width / 2.0)) + inset;
                let candle_lane_left = content_left
                    + if imb_study_on {
                        candle_width + spacing.marker_to_candle
                    } else {
                        0.0
                    };
                candle_lane_left + candle_width * candle_lane_factor + spacing.candle_to_cluster
            }
        }
    };

    let rightmost_x = interval_to_x(visible_latest);

    for (candle_idx, npoc_price) in npocs {
        let candle_time = match basis {
            ChartBasis::Time(_) => candles[candle_idx].time.0,
            ChartBasis::Tick(_) => {
                let reverse_idx = candles.len() - 1 - candle_idx;
                reverse_idx as u64
            }
        };

        let start_x = interval_to_x(candle_time);
        let cell_center_x = start_x;
        let line_start_x = start_x_for(cell_center_x);
        let line_end_x = rightmost_x;

        let y = price_to_y(npoc_price);

        // Draw horizontal line from candle to right edge
        frame.fill_rectangle(
            Point::new(line_start_x, y - (line_height / 2.0)),
            Size::new(line_end_x - line_start_x, line_height),
            naked_color,
        );
    }
}
