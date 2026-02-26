//! Footprint study renderer
//!
//! Renders `FootprintData` (from the footprint study) onto a chart canvas.
//! Migrated from `chart::candlestick::footprint` to work with abstract
//! study output types instead of the internal `BTreeMap<Price, TradeGroup>`.

mod box_mode;
mod cell;
mod scale;

use crate::chart::ViewState;
use crate::chart::perf::LodCalculator;
use data::ChartBasis;
use iced::Size;
use iced::theme::palette::Extended;
use iced::widget::canvas::Frame;
use study::output::{
    FootprintCandlePosition, FootprintData, FootprintDataType, FootprintGroupingMode,
    FootprintLevel, TextFormat,
};

use cell::draw_footprint_candle_clusters;
use scale::{
    calc_visible_max, compute_dynamic_quantum, effective_cluster_qty, merge_levels_to_quantum,
};

use data::Price;

/// Ratio of cell width occupied by cluster bars
const BAR_WIDTH_FACTOR: f32 = 0.9;
/// Alpha for cluster bar backgrounds when text labels are visible
const BAR_ALPHA_WITH_TEXT: f32 = 0.25;
/// Alpha for POC highlight background
const POC_HIGHLIGHT_ALPHA: f32 = 0.15;
/// Maximum price levels that receive text labels per candle
const TEXT_BUDGET: usize = 40;
/// Maximum text size in pixels
const MAX_TEXT_SIZE: f32 = 14.0;
/// Padding subtracted from text size
const TEXT_SIZE_PADDING: f32 = 2.0;
/// Ratio of cell width used as the candle width for footprint
const FOOTPRINT_CANDLE_WIDTH_RATIO: f32 = 0.8;

// ── Internal types ───────────────────────────────────────────────────

#[derive(Clone, Copy)]
struct ContentGaps {
    candle_to_cluster: f32,
}

impl ContentGaps {
    fn from_view(candle_width: f32, scaling: f32) -> Self {
        let px = |p: f32| p / scaling;
        let base = (candle_width * 0.2).max(px(2.0));
        Self {
            candle_to_cluster: base,
        }
    }
}

struct ClusterLayout {
    x_position: f32,
    cell_width: f32,
    row_height: f32,
    candle_width: f32,
    candle_position: FootprintCandlePosition,
    bar_marker_width: f32,
    spacing: ContentGaps,
}

/// Compute row height from adjacent levels in the footprint candle.
/// When levels are spaced by the grouping quantum, adjacent levels
/// will be `quantum` price units apart, yielding a proportionally
/// larger row height than a single-tick `cell_height`.
fn compute_row_height(
    levels: &[FootprintLevel],
    price_to_y: &impl Fn(i64) -> f32,
    fallback: f32,
) -> f32 {
    if levels.len() >= 2 {
        let y0 = price_to_y(levels[0].price);
        let y1 = price_to_y(levels[1].price);
        (y1 - y0).abs().max(1.0)
    } else {
        fallback
    }
}

struct ClusterStyle<'a> {
    palette: &'a Extended,
    text_size: f32,
    show_text: bool,
    text_format: TextFormat,
    show_zero_values: bool,
}

struct ProfileArea {
    bars_left: f32,
    bars_width: f32,
    candle_center_x: f32,
}

impl ProfileArea {
    fn new(
        content_left: f32,
        content_right: f32,
        candle_width: f32,
        gaps: ContentGaps,
        position: FootprintCandlePosition,
        bar_marker_width: f32,
    ) -> Self {
        let candle_lane_width = candle_width * bar_marker_width;
        match position {
            FootprintCandlePosition::None => Self {
                bars_left: content_left,
                bars_width: (content_right - content_left).max(0.0),
                candle_center_x: 0.0,
            },
            FootprintCandlePosition::Left | FootprintCandlePosition::Center => {
                let bars_left = content_left + candle_lane_width + gaps.candle_to_cluster;
                Self {
                    bars_left,
                    bars_width: (content_right - bars_left).max(0.0),
                    candle_center_x: content_left + (candle_lane_width / 2.0),
                }
            }
            FootprintCandlePosition::Right => {
                let bars_right = content_right - candle_lane_width - gaps.candle_to_cluster;
                Self {
                    bars_left: content_left,
                    bars_width: (bars_right - content_left).max(0.0),
                    candle_center_x: content_right - (candle_lane_width / 2.0),
                }
            }
        }
    }
}

struct BidAskArea {
    bid_area_left: f32,
    bid_area_right: f32,
    ask_area_left: f32,
    ask_area_right: f32,
    candle_center_x: f32,
}

impl BidAskArea {
    fn new(
        x_position: f32,
        content_left: f32,
        content_right: f32,
        candle_width: f32,
        spacing: ContentGaps,
        candle_position: FootprintCandlePosition,
        bar_marker_width: f32,
    ) -> Self {
        if candle_position == FootprintCandlePosition::None {
            return Self {
                bid_area_left: x_position,
                bid_area_right: content_right,
                ask_area_left: content_left,
                ask_area_right: x_position,
                candle_center_x: x_position,
            };
        }

        let candle_body_width = candle_width * bar_marker_width;
        let candle_left = x_position - (candle_body_width / 2.0);
        let candle_right = x_position + (candle_body_width / 2.0);
        let ask_area_right = candle_left - spacing.candle_to_cluster;
        let bid_area_left = candle_right + spacing.candle_to_cluster;

        Self {
            bid_area_left,
            bid_area_right: content_right,
            ask_area_left: content_left,
            ask_area_right,
            candle_center_x: x_position,
        }
    }
}

/// Render footprint study output onto a chart canvas frame.
pub fn render_footprint(
    frame: &mut Frame,
    data: &FootprintData,
    state: &ViewState,
    bounds: Size,
    palette: &Extended,
) {
    if data.candles.is_empty() {
        return;
    }

    let region = state.visible_region(bounds);
    let (earliest, latest) = state.interval_range(&region);

    let price_to_y = |price_units: i64| -> f32 { state.price_to_y(Price::from_units(price_units)) };

    // Calculate LOD for text visibility
    let visible_candle_count = match &state.basis {
        ChartBasis::Time(_) => {
            let first = data.candles.partition_point(|c| c.x < earliest);
            let last = data.candles.partition_point(|c| c.x <= latest);
            last.saturating_sub(first)
        }
        ChartBasis::Tick(_) => {
            let ea = earliest as usize;
            let la = latest as usize;
            la.saturating_sub(ea) + 1
        }
    };

    let lod = LodCalculator::new(
        state.scaling,
        state.cell_width,
        visible_candle_count,
        bounds.width,
    );
    let lod_level = lod.calculate_lod();

    let candle_width = FOOTPRINT_CANDLE_WIDTH_RATIO * state.cell_width;
    let cell_width_unscaled = state.cell_width * state.scaling;

    let content_spacing = ContentGaps::from_view(candle_width, state.scaling);

    let min_text_w = match data.data_type {
        FootprintDataType::Volume | FootprintDataType::Delta => 80.0,
        FootprintDataType::BidAskSplit | FootprintDataType::DeltaAndVolume => 120.0,
    };

    // Width-based text limit (viewport-level, doesn't change per candle)
    let from_w = (cell_width_unscaled * FOOTPRINT_CANDLE_WIDTH_RATIO)
        .round()
        .min(MAX_TEXT_SIZE)
        - TEXT_SIZE_PADDING;
    let lod_text_ok = lod_level.show_text();

    // Compute dynamic quantum for automatic grouping mode
    let tick_units = state.tick_size.units.max(1);
    let dynamic_quantum = match data.grouping_mode {
        FootprintGroupingMode::Automatic { factor } => {
            Some(compute_dynamic_quantum(state, factor, tick_units))
        }
        FootprintGroupingMode::Manual => None,
    };

    // Calculate max cluster qty for visible candles
    let max_cluster_qty = calc_visible_max(data, earliest, latest, &state.basis, dynamic_quantum);

    let max_bars = data.max_bars_to_show;
    let mut rendered_count: usize = 0;

    // Per-candle helper: compute row_height + text/style params
    let compute_render_params = |levels: &[FootprintLevel], quantum: i64| {
        let quantum_ticks = (quantum / tick_units).max(1);
        let fallback_row_height = state.cell_height * quantum_ticks as f32;
        let row_height = compute_row_height(levels, &price_to_y, fallback_row_height);
        let row_height_screen = row_height * state.scaling;

        let text_size = if data.dynamic_text_size {
            let from_h = row_height_screen.round().min(MAX_TEXT_SIZE) - TEXT_SIZE_PADDING;
            from_h.min(from_w)
        } else {
            data.font_size
        };

        let show_text = lod_text_ok && row_height_screen > 8.0 && cell_width_unscaled > min_text_w;

        let cluster_style = ClusterStyle {
            palette,
            text_size,
            show_text,
            text_format: data.text_format,
            show_zero_values: data.show_zero_values,
        };

        (row_height, cluster_style)
    };

    // Iterate visible candles
    match &state.basis {
        ChartBasis::Tick(_) => {
            let candle_count = data.candles.len();
            let earliest_idx = earliest as usize;
            let latest_idx = latest as usize;

            for (rev_idx, fp_candle) in data.candles.iter().rev().enumerate() {
                if rev_idx < earliest_idx || rev_idx > latest_idx {
                    continue;
                }
                let x_position = state.interval_to_x(rev_idx as u64);
                let _candle_idx = candle_count - 1 - rev_idx;

                // Merge levels for automatic mode
                let merged_buf;
                let (levels, poc_index) = match dynamic_quantum {
                    Some(q) if q > fp_candle.quantum => {
                        merged_buf = merge_levels_to_quantum(&fp_candle.levels, q);
                        (&merged_buf.0[..], merged_buf.1)
                    }
                    _ => (&fp_candle.levels[..], fp_candle.poc_index),
                };
                let eff_quantum = dynamic_quantum.unwrap_or(fp_candle.quantum);

                let eff_max =
                    effective_cluster_qty(data.scaling, max_cluster_qty, levels, data.data_type);

                let (row_height, cluster_style) = compute_render_params(levels, eff_quantum);

                let layout = ClusterLayout {
                    x_position,
                    cell_width: state.cell_width,
                    row_height,
                    candle_width,
                    candle_position: data.candle_position,
                    bar_marker_width: data.bar_marker_width,
                    spacing: content_spacing,
                };

                let skip_levels = rendered_count >= max_bars;
                rendered_count += 1;

                draw_footprint_candle_clusters(
                    frame,
                    &layout,
                    &cluster_style,
                    eff_max,
                    fp_candle,
                    levels,
                    poc_index,
                    data,
                    &price_to_y,
                    skip_levels,
                );
            }
        }
        ChartBasis::Time(_) => {
            if latest < earliest {
                return;
            }
            for fp_candle in &data.candles {
                if fp_candle.x < earliest || fp_candle.x > latest {
                    continue;
                }
                let x_position = state.interval_to_x(fp_candle.x);

                // Merge levels for automatic mode
                let merged_buf;
                let (levels, poc_index) = match dynamic_quantum {
                    Some(q) if q > fp_candle.quantum => {
                        merged_buf = merge_levels_to_quantum(&fp_candle.levels, q);
                        (&merged_buf.0[..], merged_buf.1)
                    }
                    _ => (&fp_candle.levels[..], fp_candle.poc_index),
                };
                let eff_quantum = dynamic_quantum.unwrap_or(fp_candle.quantum);

                let eff_max =
                    effective_cluster_qty(data.scaling, max_cluster_qty, levels, data.data_type);

                let (row_height, cluster_style) = compute_render_params(levels, eff_quantum);

                let layout = ClusterLayout {
                    x_position,
                    cell_width: state.cell_width,
                    row_height,
                    candle_width,
                    candle_position: data.candle_position,
                    bar_marker_width: data.bar_marker_width,
                    spacing: content_spacing,
                };

                let skip_levels = rendered_count >= max_bars;
                rendered_count += 1;

                draw_footprint_candle_clusters(
                    frame,
                    &layout,
                    &cluster_style,
                    eff_max,
                    fp_candle,
                    levels,
                    poc_index,
                    data,
                    &price_to_y,
                    skip_levels,
                );
            }
        }
    }
}
