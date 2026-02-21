//! Footprint study renderer
//!
//! Renders `FootprintData` (from the footprint study) onto a chart canvas.
//! Migrated from `chart::candlestick::footprint` to work with abstract
//! study output types instead of the internal `BTreeMap<Price, TradeGroup>`.

use crate::chart::ViewState;
use crate::chart::perf::LodCalculator;
use crate::components::primitives::AZERET_MONO;
use data::ChartBasis;
use iced::theme::palette::Extended;
use iced::widget::canvas::{self, Frame, Path, Stroke};
use iced::{Alignment, Color, Point, Size};
use std::collections::BTreeSet;
use study::output::{
    BackgroundColorMode, FootprintCandle, FootprintCandlePosition,
    FootprintData, FootprintDataType, FootprintGroupingMode,
    FootprintLevel, FootprintRenderMode, FootprintScaling,
    OutsideBarStyle, TextFormat,
};

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

/// Minimum row height in screen pixels for readable text.
const MIN_ROW_PX: f32 = 16.0;

use exchange::util::Price;

/// Compute the dynamic grouping quantum for automatic mode.
///
/// `factor` is the user's scale factor; larger → coarser grouping.
fn compute_dynamic_quantum(
    state: &ViewState,
    factor: i64,
    tick_units: i64,
) -> i64 {
    let pixel_per_tick = state.cell_height * state.scaling;
    let base_ticks = (MIN_ROW_PX / pixel_per_tick).ceil() as i64;
    (base_ticks * factor).max(1) * tick_units
}

/// Merge footprint levels to a coarser quantum boundary.
///
/// Returns the merged level vec and the new POC index.
fn merge_levels_to_quantum(
    levels: &[FootprintLevel],
    target_quantum: i64,
) -> (Vec<FootprintLevel>, Option<usize>) {
    use std::collections::BTreeMap;

    let mut merged: BTreeMap<i64, (f32, f32)> = BTreeMap::new();
    for level in levels {
        let rounded =
            (level.price / target_quantum) * target_quantum;
        let entry = merged.entry(rounded).or_insert((0.0, 0.0));
        entry.0 += level.buy_volume;
        entry.1 += level.sell_volume;
    }

    let result: Vec<FootprintLevel> = merged
        .into_iter()
        .map(|(price, (buy, sell))| FootprintLevel {
            price,
            buy_volume: buy,
            sell_volume: sell,
        })
        .collect();

    let poc_index = result
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| {
            a.total_qty()
                .partial_cmp(&b.total_qty())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(i, _)| i);

    (result, poc_index)
}

/// Format a footprint level value according to the configured format.
/// Footprint values are always whole contract counts, so we avoid
/// trailing `.0` decimals that `abbr_large_numbers` would produce.
fn format_value(value: f32, format: TextFormat) -> String {
    match format {
        TextFormat::Automatic => {
            let abs = value.abs();
            let sign = if value < 0.0 { "-" } else { "" };
            if abs >= 1_000_000.0 {
                format!("{}{:.1}m", sign, abs / 1_000_000.0)
            } else if abs >= 10_000.0 {
                format!("{}{:.1}k", sign, abs / 1_000.0)
            } else if abs >= 1_000.0 {
                format!("{}{:.1}k", sign, abs / 1_000.0)
            } else {
                format!("{}{:.0}", sign, abs)
            }
        }
        TextFormat::Normal => format!("{:.0}", value),
        TextFormat::K => {
            if value.abs() >= 1000.0 {
                format!("{:.1}K", value / 1000.0)
            } else {
                format!("{:.0}", value)
            }
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

    let price_to_y = |price_units: i64| -> f32 {
        state.price_to_y(Price::from_units(price_units))
    };

    // Calculate LOD for text visibility
    let visible_candle_count = match &state.basis {
        ChartBasis::Time(_) => {
            let first =
                data.candles.partition_point(|c| c.x < earliest);
            let last =
                data.candles.partition_point(|c| c.x <= latest);
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

    let content_spacing =
        ContentGaps::from_view(candle_width, state.scaling);

    let min_text_w = match data.data_type {
        FootprintDataType::Volume | FootprintDataType::Delta => 80.0,
        FootprintDataType::BidAskSplit
        | FootprintDataType::DeltaAndVolume => 120.0,
    };

    // Width-based text limit (viewport-level, doesn't change per candle)
    let from_w = (cell_width_unscaled
        * FOOTPRINT_CANDLE_WIDTH_RATIO)
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
    let max_cluster_qty = calc_visible_max(
        data,
        earliest,
        latest,
        &state.basis,
        dynamic_quantum,
    );

    let max_bars = data.max_bars_to_show;
    let mut rendered_count: usize = 0;

    // Per-candle helper: compute row_height + text/style params
    let compute_render_params =
        |levels: &[FootprintLevel], quantum: i64| {
            let quantum_ticks =
                (quantum / tick_units).max(1);
            let fallback_row_height =
                state.cell_height * quantum_ticks as f32;
            let row_height = compute_row_height(
                levels,
                &price_to_y,
                fallback_row_height,
            );
            let row_height_screen = row_height * state.scaling;

            let text_size = if data.dynamic_text_size {
                let from_h = row_height_screen
                    .round()
                    .min(MAX_TEXT_SIZE)
                    - TEXT_SIZE_PADDING;
                from_h.min(from_w)
            } else {
                data.font_size
            };

            let show_text = lod_text_ok
                && row_height_screen > 8.0
                && cell_width_unscaled > min_text_w;

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

            for (rev_idx, fp_candle) in
                data.candles.iter().rev().enumerate()
            {
                if rev_idx < earliest_idx || rev_idx > latest_idx {
                    continue;
                }
                let x_position =
                    state.interval_to_x(rev_idx as u64);
                let _candle_idx = candle_count - 1 - rev_idx;

                // Merge levels for automatic mode
                let merged_buf;
                let (levels, poc_index) = match dynamic_quantum
                {
                    Some(q) if q > fp_candle.quantum => {
                        merged_buf =
                            merge_levels_to_quantum(
                                &fp_candle.levels,
                                q,
                            );
                        (&merged_buf.0[..], merged_buf.1)
                    }
                    _ => (
                        &fp_candle.levels[..],
                        fp_candle.poc_index,
                    ),
                };
                let eff_quantum =
                    dynamic_quantum.unwrap_or(fp_candle.quantum);

                let eff_max = effective_cluster_qty(
                    data.scaling,
                    max_cluster_qty,
                    levels,
                    data.data_type,
                );

                let (row_height, cluster_style) =
                    compute_render_params(levels, eff_quantum);

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
                let x_position =
                    state.interval_to_x(fp_candle.x);

                // Merge levels for automatic mode
                let merged_buf;
                let (levels, poc_index) = match dynamic_quantum
                {
                    Some(q) if q > fp_candle.quantum => {
                        merged_buf =
                            merge_levels_to_quantum(
                                &fp_candle.levels,
                                q,
                            );
                        (&merged_buf.0[..], merged_buf.1)
                    }
                    _ => (
                        &fp_candle.levels[..],
                        fp_candle.poc_index,
                    ),
                };
                let eff_quantum =
                    dynamic_quantum.unwrap_or(fp_candle.quantum);

                let eff_max = effective_cluster_qty(
                    data.scaling,
                    max_cluster_qty,
                    levels,
                    data.data_type,
                );

                let (row_height, cluster_style) =
                    compute_render_params(levels, eff_quantum);

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
            FootprintCandlePosition::Left
            | FootprintCandlePosition::Center => {
                let bars_left = content_left
                    + candle_lane_width
                    + gaps.candle_to_cluster;
                Self {
                    bars_left,
                    bars_width: (content_right - bars_left).max(0.0),
                    candle_center_x: content_left
                        + (candle_lane_width / 2.0),
                }
            }
            FootprintCandlePosition::Right => {
                let bars_right = content_right
                    - candle_lane_width
                    - gaps.candle_to_cluster;
                Self {
                    bars_left: content_left,
                    bars_width: (bars_right - content_left).max(0.0),
                    candle_center_x: content_right
                        - (candle_lane_width / 2.0),
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
        let ask_area_right =
            candle_left - spacing.candle_to_cluster;
        let bid_area_left =
            candle_right + spacing.candle_to_cluster;

        Self {
            bid_area_left,
            bid_area_right: content_right,
            ask_area_left: content_left,
            ask_area_right,
            candle_center_x: x_position,
        }
    }
}

// ── Helpers ──────────────────────────────────────────────────────────

fn effective_cluster_qty(
    scaling: FootprintScaling,
    visible_max: f32,
    levels: &[FootprintLevel],
    data_type: FootprintDataType,
) -> f32 {
    let individual_max = match data_type {
        FootprintDataType::BidAskSplit
        | FootprintDataType::DeltaAndVolume => levels
            .iter()
            .map(|l| l.buy_volume.max(l.sell_volume))
            .fold(0.0_f32, f32::max),
        FootprintDataType::Delta => levels
            .iter()
            .map(|l| l.delta_qty().abs())
            .fold(0.0_f32, f32::max),
        FootprintDataType::Volume => levels
            .iter()
            .map(|l| l.total_qty())
            .fold(0.0_f32, f32::max),
    };

    let safe = |v: f32| if v <= f32::EPSILON { 1.0 } else { v };

    match scaling {
        FootprintScaling::VisibleRange => safe(visible_max),
        FootprintScaling::Datapoint => safe(individual_max),
        FootprintScaling::Hybrid { weight } => {
            let w = weight.clamp(0.0, 1.0);
            safe(visible_max * w + individual_max * (1.0 - w))
        }
        FootprintScaling::Linear
        | FootprintScaling::Sqrt
        | FootprintScaling::Log => safe(visible_max),
    }
}

#[inline]
fn scaled_ratio(
    qty: f32,
    max: f32,
    scaling: FootprintScaling,
) -> f32 {
    if max <= f32::EPSILON || qty <= f32::EPSILON {
        return 0.0;
    }
    match scaling {
        FootprintScaling::Sqrt => qty.sqrt() / max.sqrt(),
        FootprintScaling::Log => {
            (1.0 + qty).ln() / (1.0 + max).ln()
        }
        _ => qty / max,
    }
}

fn calc_visible_max(
    data: &FootprintData,
    earliest: u64,
    latest: u64,
    basis: &ChartBasis,
    dynamic_quantum: Option<i64>,
) -> f32 {
    let candles_iter: Box<dyn Iterator<Item = &FootprintCandle>> =
        match basis {
            ChartBasis::Time(_) => Box::new(
                data.candles
                    .iter()
                    .filter(move |c| {
                        c.x >= earliest && c.x <= latest
                    }),
            ),
            ChartBasis::Tick(_) => {
                let ea = earliest as usize;
                let la = latest as usize;
                Box::new(
                    data.candles
                        .iter()
                        .rev()
                        .enumerate()
                        .filter(move |(i, _)| {
                            *i >= ea && *i <= la
                        })
                        .map(|(_, c)| c),
                )
            }
        };

    let level_max = |levels: &[FootprintLevel]| -> f32 {
        levels
            .iter()
            .map(|l| match data.data_type {
                FootprintDataType::Volume => l.total_qty(),
                FootprintDataType::BidAskSplit
                | FootprintDataType::DeltaAndVolume => {
                    l.buy_volume.max(l.sell_volume)
                }
                FootprintDataType::Delta => l.delta_qty().abs(),
            })
            .fold(0.0_f32, f32::max)
    };

    candles_iter
        .map(|c| {
            match dynamic_quantum {
                Some(q) if q > c.quantum => {
                    let merged =
                        merge_levels_to_quantum(&c.levels, q);
                    level_max(&merged.0)
                }
                _ => level_max(&c.levels),
            }
        })
        .fold(0.0_f32, f32::max)
}

fn draw_poc_highlight(
    frame: &mut Frame,
    x: f32,
    y: f32,
    width: f32,
    cell_height: f32,
    palette: &Extended,
) {
    frame.fill_rectangle(
        Point::new(x, y - (cell_height / 2.0)),
        Size::new(width, cell_height),
        palette
            .primary
            .base
            .color
            .scale_alpha(POC_HIGHLIGHT_ALPHA),
    );
}

fn text_budget_set(
    levels: &[FootprintLevel],
    show_text: bool,
) -> Option<BTreeSet<i64>> {
    if !show_text || levels.len() <= TEXT_BUDGET {
        return None;
    }
    let mut ranked: Vec<(i64, f32)> =
        levels.iter().map(|l| (l.price, l.total_qty())).collect();
    ranked.select_nth_unstable_by(TEXT_BUDGET - 1, |a, b| {
        b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)
    });
    ranked.truncate(TEXT_BUDGET);
    Some(ranked.into_iter().map(|(p, _)| p).collect())
}

fn draw_cluster_text(
    frame: &mut Frame,
    text: &str,
    position: Point,
    text_size: f32,
    color: Color,
    align_x: Alignment,
    align_y: Alignment,
) {
    frame.fill_text(canvas::Text {
        content: text.to_string(),
        position,
        size: iced::Pixels(text_size),
        color,
        align_x: align_x.into(),
        align_y: align_y.into(),
        font: AZERET_MONO,
        ..canvas::Text::default()
    });
}

fn draw_thin_candle(
    frame: &mut Frame,
    fp_candle: &FootprintCandle,
    candle_center_x: f32,
    candle_width: f32,
    palette: &Extended,
    price_to_y: &impl Fn(i64) -> f32,
    outside_bar_style: OutsideBarStyle,
    show_outside_border: bool,
    bar_marker_width: f32,
) {
    if outside_bar_style == OutsideBarStyle::None {
        return;
    }

    let y_open = price_to_y(fp_candle.open);
    let y_high = price_to_y(fp_candle.high);
    let y_low = price_to_y(fp_candle.low);
    let y_close = price_to_y(fp_candle.close);

    let body_color = if fp_candle.close >= fp_candle.open {
        palette.success.weak.color
    } else {
        palette.danger.weak.color
    };

    let body_half = candle_width * bar_marker_width / 2.0;
    let body_x = candle_center_x - body_half;
    let body_w = body_half * 2.0;
    let body_top = y_open.min(y_close);
    let body_h = (y_open - y_close).abs();

    frame.fill_rectangle(
        Point::new(body_x, body_top),
        Size::new(body_w, body_h),
        body_color,
    );

    if show_outside_border {
        let border_stroke = Stroke::with_color(
            Stroke {
                width: 1.0,
                ..Default::default()
            },
            body_color.scale_alpha(0.8),
        );
        frame.stroke(
            &Path::rectangle(
                Point::new(body_x, body_top),
                Size::new(body_w, body_h),
            ),
            border_stroke,
        );
    }

    // Wicks only in Candle style
    if outside_bar_style == OutsideBarStyle::Candle {
        let wick_color = body_color.scale_alpha(0.6);
        let marker_line = Stroke::with_color(
            Stroke {
                width: 1.0,
                ..Default::default()
            },
            wick_color,
        );
        frame.stroke(
            &Path::line(
                Point::new(candle_center_x, y_high),
                Point::new(candle_center_x, y_low),
            ),
            marker_line,
        );
    }
}

// ── Main per-candle rendering ─────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn draw_footprint_candle_clusters(
    frame: &mut Frame,
    layout: &ClusterLayout,
    style: &ClusterStyle<'_>,
    max_cluster_qty: f32,
    fp_candle: &FootprintCandle,
    levels: &[FootprintLevel],
    poc_index: Option<usize>,
    data: &FootprintData,
    price_to_y: &impl Fn(i64) -> f32,
    skip_levels: bool,
) {
    let x_position = layout.x_position;
    let cell_width = layout.cell_width;
    let row_height = layout.row_height;
    let candle_width = layout.candle_width;
    let candle_position = layout.candle_position;
    let bar_marker_width = layout.bar_marker_width;
    let spacing = layout.spacing;
    let palette = style.palette;
    let text_size = style.text_size;
    let show_text = style.show_text;

    let poc_price = poc_index
        .and_then(|i| levels.get(i))
        .map(|l| l.price);
    // Box mode: no text budget (cells don't overlap)
    let text_set = if data.mode == FootprintRenderMode::Box {
        None
    } else {
        text_budget_set(levels, show_text)
    };
    let show_zero = style.show_zero_values;
    let text_format = style.text_format;
    let should_label = |price: i64| {
        show_text
            && text_set
                .as_ref()
                .is_none_or(|s| s.contains(&price))
    };

    // When skip_levels is true, only draw the thin candle (no levels)
    if skip_levels {
        if candle_position != FootprintCandlePosition::None {
            draw_thin_candle(
                frame,
                fp_candle,
                x_position,
                candle_width,
                palette,
                price_to_y,
                data.outside_bar_style,
                data.show_outside_border,
                bar_marker_width,
            );
        }
        return;
    }

    if data.mode == FootprintRenderMode::Box {
        // Compute box grid area, accounting for candle marker
        let inset =
            (cell_width * (1.0 - BAR_WIDTH_FACTOR)) / 2.0;
        let content_left =
            x_position - (cell_width / 2.0) + inset;
        let content_right =
            x_position + (cell_width / 2.0) - inset;

        let (box_left, box_width, candle_cx) =
            if candle_position
                == FootprintCandlePosition::None
            {
                (
                    content_left,
                    (content_right - content_left).max(0.0),
                    0.0,
                )
            } else {
                let lane =
                    candle_width * bar_marker_width;
                match candle_position {
                    FootprintCandlePosition::Left => {
                        let bl = content_left
                            + lane
                            + spacing.candle_to_cluster;
                        (
                            bl,
                            (content_right - bl).max(0.0),
                            content_left + (lane / 2.0),
                        )
                    }
                    FootprintCandlePosition::Center => {
                        // Candle overlays centered on full
                        // grid
                        (
                            content_left,
                            (content_right - content_left)
                                .max(0.0),
                            x_position,
                        )
                    }
                    FootprintCandlePosition::Right => {
                        let br = content_right
                            - lane
                            - spacing.candle_to_cluster;
                        (
                            content_left,
                            (br - content_left).max(0.0),
                            content_right - (lane / 2.0),
                        )
                    }
                    _ => (
                        content_left,
                        (content_right - content_left)
                            .max(0.0),
                        0.0,
                    ),
                }
            };

        draw_box_mode(
            frame,
            price_to_y,
            box_left,
            box_width,
            row_height,
            max_cluster_qty,
            palette,
            text_size,
            levels,
            data.data_type,
            data.scaling,
            poc_price,
            &should_label,
            data.bg_color_mode,
            data.bg_max_alpha,
            data.bg_buy_color.map(|c| {
                Color::from_rgba(c.r, c.g, c.b, c.a)
            }),
            data.bg_sell_color.map(|c| {
                Color::from_rgba(c.r, c.g, c.b, c.a)
            }),
            data.text_color.map(|c| {
                Color::from_rgba(c.r, c.g, c.b, c.a)
            }),
            data.show_grid_lines,
            show_zero,
            text_format,
        );

        if candle_position != FootprintCandlePosition::None
        {
            draw_thin_candle(
                frame,
                fp_candle,
                candle_cx,
                candle_width,
                palette,
                price_to_y,
                data.outside_bar_style,
                data.show_outside_border,
                bar_marker_width,
            );
        }
        return;
    }

    // Profile mode
    let text_color = data
        .text_color
        .map(|c| Color::from_rgba(c.r, c.g, c.b, c.a))
        .unwrap_or(palette.background.weakest.text);
    let inset = (cell_width * (1.0 - BAR_WIDTH_FACTOR)) / 2.0;
    let cell_left = x_position - (cell_width / 2.0);
    let content_left = cell_left + inset;
    let content_right = x_position + (cell_width / 2.0) - inset;

    let draw_candle_body =
        candle_position != FootprintCandlePosition::None;

    let buy_bar_color = data
        .bg_buy_color
        .map(|c| Color::from_rgba(c.r, c.g, c.b, c.a))
        .unwrap_or(palette.success.base.color);
    let sell_bar_color = data
        .bg_sell_color
        .map(|c| Color::from_rgba(c.r, c.g, c.b, c.a))
        .unwrap_or(palette.danger.base.color);

    match data.data_type {
        FootprintDataType::Volume | FootprintDataType::Delta => {
            let area = ProfileArea::new(
                content_left,
                content_right,
                candle_width,
                spacing,
                candle_position,
                bar_marker_width,
            );
            let bar_alpha =
                if show_text { BAR_ALPHA_WITH_TEXT } else { 1.0 };

            for level in levels {
                let y = price_to_y(level.price);

                if poc_price == Some(level.price) {
                    draw_poc_highlight(
                        frame,
                        area.bars_left,
                        y,
                        area.bars_width,
                        row_height,
                        palette,
                    );
                }

                match data.data_type {
                    FootprintDataType::Volume => {
                        let total_qty = level.total_qty();
                        let ratio = scaled_ratio(
                            total_qty,
                            max_cluster_qty,
                            data.scaling,
                        );
                        let total_bar_len =
                            ratio * area.bars_width;

                        if total_bar_len > 0.0 {
                            let buy_frac =
                                level.buy_volume / total_qty;
                            let sell_len =
                                (1.0 - buy_frac) * total_bar_len;
                            let buy_len =
                                buy_frac * total_bar_len;
                            let bar_y = y - (row_height / 2.0);

                            if level.sell_volume > 0.0 {
                                frame.fill_rectangle(
                                    Point::new(
                                        area.bars_left, bar_y,
                                    ),
                                    Size::new(
                                        sell_len, row_height,
                                    ),
                                    sell_bar_color
                                        .scale_alpha(bar_alpha),
                                );
                            }
                            if level.buy_volume > 0.0 {
                                frame.fill_rectangle(
                                    Point::new(
                                        area.bars_left + sell_len,
                                        bar_y,
                                    ),
                                    Size::new(
                                        buy_len, row_height,
                                    ),
                                    buy_bar_color
                                        .scale_alpha(bar_alpha),
                                );
                            }
                        }

                        if should_label(level.price)
                            && (show_zero
                                || total_qty > f32::EPSILON)
                        {
                            draw_cluster_text(
                                frame,
                                &format_value(
                                    total_qty,
                                    text_format,
                                ),
                                Point::new(area.bars_left, y),
                                text_size,
                                text_color,
                                Alignment::Start,
                                Alignment::Center,
                            );
                        }
                    }
                    FootprintDataType::Delta => {
                        let delta = level.delta_qty();
                        let ratio = scaled_ratio(
                            delta.abs(),
                            max_cluster_qty,
                            data.scaling,
                        );
                        let bar_width = ratio * area.bars_width;

                        if bar_width > 0.0 {
                            let color = if delta >= 0.0 {
                                buy_bar_color
                                    .scale_alpha(bar_alpha)
                            } else {
                                sell_bar_color
                                    .scale_alpha(bar_alpha)
                            };
                            frame.fill_rectangle(
                                Point::new(
                                    area.bars_left,
                                    y - (row_height / 2.0),
                                ),
                                Size::new(bar_width, row_height),
                                color,
                            );
                        }

                        if should_label(level.price)
                            && (show_zero
                                || delta.abs() > f32::EPSILON)
                        {
                            draw_cluster_text(
                                frame,
                                &format_value(delta, text_format),
                                Point::new(area.bars_left, y),
                                text_size,
                                text_color,
                                Alignment::Start,
                                Alignment::Center,
                            );
                        }
                    }
                    _ => {}
                }
            }

            if draw_candle_body {
                draw_thin_candle(
                    frame,
                    fp_candle,
                    area.candle_center_x,
                    candle_width,
                    palette,
                    price_to_y,
                    data.outside_bar_style,
                    data.show_outside_border,
                    bar_marker_width,
                );
            }
        }
        FootprintDataType::BidAskSplit
        | FootprintDataType::DeltaAndVolume => {
            let area = BidAskArea::new(
                x_position,
                content_left,
                content_right,
                candle_width,
                spacing,
                candle_position,
                bar_marker_width,
            );

            let bar_alpha =
                if show_text { BAR_ALPHA_WITH_TEXT } else { 1.0 };
            let right_area_width =
                (area.bid_area_right - area.bid_area_left).max(0.0);
            let left_area_width =
                (area.ask_area_right - area.ask_area_left).max(0.0);

            for level in levels {
                let y = price_to_y(level.price);

                if poc_price == Some(level.price) {
                    draw_poc_highlight(
                        frame,
                        area.ask_area_left,
                        y,
                        area.bid_area_right - area.ask_area_left,
                        row_height,
                        palette,
                    );
                }

                if level.buy_volume > 0.0
                    && right_area_width > 0.0
                {
                    if should_label(level.price)
                        && (show_zero
                            || level.buy_volume > f32::EPSILON)
                    {
                        draw_cluster_text(
                            frame,
                            &format_value(
                                level.buy_volume,
                                text_format,
                            ),
                            Point::new(area.bid_area_left, y),
                            text_size,
                            text_color,
                            Alignment::Start,
                            Alignment::Center,
                        );
                    }

                    let ratio = scaled_ratio(
                        level.buy_volume,
                        max_cluster_qty,
                        data.scaling,
                    );
                    let bar_width = ratio * right_area_width;
                    if bar_width > 0.0 {
                        frame.fill_rectangle(
                            Point::new(
                                area.bid_area_left,
                                y - (row_height / 2.0),
                            ),
                            Size::new(bar_width, row_height),
                            buy_bar_color
                                .scale_alpha(bar_alpha),
                        );
                    }
                }
                if (level.sell_volume > 0.0 || show_zero)
                    && left_area_width > 0.0
                {
                    if should_label(level.price)
                        && (show_zero
                            || level.sell_volume > f32::EPSILON)
                    {
                        draw_cluster_text(
                            frame,
                            &format_value(
                                level.sell_volume,
                                text_format,
                            ),
                            Point::new(area.ask_area_right, y),
                            text_size,
                            text_color,
                            Alignment::End,
                            Alignment::Center,
                        );
                    }

                    if level.sell_volume > 0.0 {
                        let ratio = scaled_ratio(
                            level.sell_volume,
                            max_cluster_qty,
                            data.scaling,
                        );
                        let bar_width = ratio * left_area_width;
                        if bar_width > 0.0 {
                            frame.fill_rectangle(
                                Point::new(
                                    area.ask_area_right,
                                    y - (row_height / 2.0),
                                ),
                                Size::new(
                                    -bar_width, row_height,
                                ),
                                sell_bar_color
                                    .scale_alpha(bar_alpha),
                            );
                        }
                    }
                }
            }

            if draw_candle_body {
                draw_thin_candle(
                    frame,
                    fp_candle,
                    area.candle_center_x,
                    candle_width,
                    palette,
                    price_to_y,
                    data.outside_bar_style,
                    data.show_outside_border,
                    bar_marker_width,
                );
            }
        }
    }
}

// ── Box mode ─────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn draw_box_mode(
    frame: &mut Frame,
    price_to_y: &impl Fn(i64) -> f32,
    box_left: f32,
    box_width: f32,
    row_height: f32,
    max_cluster_qty: f32,
    palette: &Extended,
    text_size: f32,
    levels: &[FootprintLevel],
    data_type: FootprintDataType,
    scaling: FootprintScaling,
    poc_price: Option<i64>,
    should_label: &dyn Fn(i64) -> bool,
    bg_color_mode: BackgroundColorMode,
    bg_max_alpha: f32,
    custom_buy_color: Option<Color>,
    custom_sell_color: Option<Color>,
    custom_text_color: Option<Color>,
    show_grid_lines: bool,
    show_zero: bool,
    text_format: TextFormat,
) {
    let text_color = custom_text_color
        .unwrap_or(palette.background.weakest.text);
    let box_center = box_left + box_width / 2.0;
    let buy_color =
        custom_buy_color.unwrap_or(palette.success.base.color);
    let sell_color =
        custom_sell_color.unwrap_or(palette.danger.base.color);

    let grid_stroke = if show_grid_lines {
        Some(Stroke::with_color(
            Stroke {
                width: 1.0,
                ..Default::default()
            },
            palette
                .background
                .weak
                .color
                .scale_alpha(0.3),
        ))
    } else {
        None
    };

    for level in levels {
        let y = price_to_y(level.price);
        let bar_y = y - (row_height / 2.0);

        match data_type {
            FootprintDataType::BidAskSplit
            | FootprintDataType::DeltaAndVolume => {
                // Left half: sell, Right half: buy
                let sell_bg = compute_box_bg(
                    level.sell_volume,
                    max_cluster_qty,
                    scaling,
                    bg_color_mode,
                    level,
                    false,
                    bg_max_alpha,
                    &sell_color,
                    &buy_color,
                );
                if let Some((color, alpha)) = sell_bg {
                    frame.fill_rectangle(
                        Point::new(box_left, bar_y),
                        Size::new(
                            box_width / 2.0,
                            row_height,
                        ),
                        color.scale_alpha(alpha),
                    );
                }

                let buy_bg = compute_box_bg(
                    level.buy_volume,
                    max_cluster_qty,
                    scaling,
                    bg_color_mode,
                    level,
                    true,
                    bg_max_alpha,
                    &sell_color,
                    &buy_color,
                );
                if let Some((color, alpha)) = buy_bg {
                    frame.fill_rectangle(
                        Point::new(box_center, bar_y),
                        Size::new(
                            box_width / 2.0,
                            row_height,
                        ),
                        color.scale_alpha(alpha),
                    );
                }

                if let Some(ref stroke) = grid_stroke {
                    frame.stroke(
                        &Path::rectangle(
                            Point::new(box_left, bar_y),
                            Size::new(
                                box_width / 2.0,
                                row_height,
                            ),
                        ),
                        *stroke,
                    );
                    frame.stroke(
                        &Path::rectangle(
                            Point::new(box_center, bar_y),
                            Size::new(
                                box_width / 2.0,
                                row_height,
                            ),
                        ),
                        *stroke,
                    );
                }

                if should_label(level.price) {
                    if level.sell_volume > 0.0 || show_zero {
                        draw_cluster_text(
                            frame,
                            &format_value(
                                level.sell_volume,
                                text_format,
                            ),
                            Point::new(
                                box_left + box_width * 0.25,
                                y,
                            ),
                            text_size,
                            text_color,
                            Alignment::Center,
                            Alignment::Center,
                        );
                    }
                    if level.buy_volume > 0.0 || show_zero {
                        draw_cluster_text(
                            frame,
                            &format_value(
                                level.buy_volume,
                                text_format,
                            ),
                            Point::new(
                                box_center + box_width * 0.25,
                                y,
                            ),
                            text_size,
                            text_color,
                            Alignment::Center,
                            Alignment::Center,
                        );
                    }
                }
            }
            FootprintDataType::Volume => {
                let total = level.total_qty();
                let bg = compute_box_bg_single(
                    total,
                    max_cluster_qty,
                    scaling,
                    bg_color_mode,
                    level,
                    bg_max_alpha,
                    &sell_color,
                    &buy_color,
                );
                if let Some((color, alpha)) = bg {
                    frame.fill_rectangle(
                        Point::new(box_left, bar_y),
                        Size::new(box_width, row_height),
                        color.scale_alpha(alpha),
                    );
                }

                if let Some(ref stroke) = grid_stroke {
                    frame.stroke(
                        &Path::rectangle(
                            Point::new(box_left, bar_y),
                            Size::new(box_width, row_height),
                        ),
                        *stroke,
                    );
                }

                if should_label(level.price)
                    && (total > f32::EPSILON || show_zero)
                {
                    draw_cluster_text(
                        frame,
                        &format_value(total, text_format),
                        Point::new(box_center, y),
                        text_size,
                        text_color,
                        Alignment::Center,
                        Alignment::Center,
                    );
                }
            }
            FootprintDataType::Delta => {
                let delta = level.delta_qty();
                let bg = compute_box_bg_single(
                    delta.abs(),
                    max_cluster_qty,
                    scaling,
                    bg_color_mode,
                    level,
                    bg_max_alpha,
                    &sell_color,
                    &buy_color,
                );
                if let Some((_color, alpha)) = bg {
                    let actual_color = if delta >= 0.0 {
                        buy_color
                    } else {
                        sell_color
                    };
                    frame.fill_rectangle(
                        Point::new(box_left, bar_y),
                        Size::new(box_width, row_height),
                        actual_color.scale_alpha(alpha),
                    );
                }

                if let Some(ref stroke) = grid_stroke {
                    frame.stroke(
                        &Path::rectangle(
                            Point::new(box_left, bar_y),
                            Size::new(box_width, row_height),
                        ),
                        *stroke,
                    );
                }

                if should_label(level.price)
                    && (delta.abs() > f32::EPSILON
                        || show_zero)
                {
                    draw_cluster_text(
                        frame,
                        &format_value(delta, text_format),
                        Point::new(box_center, y),
                        text_size,
                        text_color,
                        Alignment::Center,
                        Alignment::Center,
                    );
                }
            }
        }

        if poc_price == Some(level.price) {
            draw_poc_highlight(
                frame, box_left, y, box_width, row_height,
                palette,
            );
        }
    }
}

// ── Box mode background helpers ─────────────────────────────────────

/// Compute background color and alpha for a split cell (bid/ask).
#[allow(clippy::too_many_arguments)]
fn compute_box_bg(
    volume: f32,
    max_cluster_qty: f32,
    scaling: FootprintScaling,
    bg_mode: BackgroundColorMode,
    level: &FootprintLevel,
    is_buy: bool,
    bg_max_alpha: f32,
    sell_color: &Color,
    buy_color: &Color,
) -> Option<(Color, f32)> {
    match bg_mode {
        BackgroundColorMode::VolumeIntensity => {
            let ratio =
                scaled_ratio(volume, max_cluster_qty, scaling);
            let alpha =
                (ratio.min(1.0) * bg_max_alpha).max(0.03);
            let color = if is_buy { *buy_color } else { *sell_color };
            Some((color, alpha))
        }
        BackgroundColorMode::DeltaIntensity => {
            let total = level.total_qty();
            let delta_ratio = if total > 0.0 {
                (level.buy_volume - level.sell_volume) / total
            } else {
                0.0
            };
            let color = if delta_ratio >= 0.0 {
                *buy_color
            } else {
                *sell_color
            };
            let alpha =
                (delta_ratio.abs() * bg_max_alpha).max(0.03);
            Some((color, alpha))
        }
        BackgroundColorMode::None => None,
    }
}

/// Compute background color and alpha for a full-width cell
/// (Volume/Delta data types).
#[allow(clippy::too_many_arguments)]
fn compute_box_bg_single(
    qty: f32,
    max_cluster_qty: f32,
    scaling: FootprintScaling,
    bg_mode: BackgroundColorMode,
    level: &FootprintLevel,
    bg_max_alpha: f32,
    sell_color: &Color,
    buy_color: &Color,
) -> Option<(Color, f32)> {
    match bg_mode {
        BackgroundColorMode::VolumeIntensity => {
            let ratio =
                scaled_ratio(qty, max_cluster_qty, scaling);
            let buy_frac = if level.total_qty() > 0.0 {
                level.buy_volume / level.total_qty()
            } else {
                0.5
            };
            let color = if buy_frac >= 0.5 {
                *buy_color
            } else {
                *sell_color
            };
            let alpha =
                (ratio.min(1.0) * bg_max_alpha).max(0.03);
            Some((color, alpha))
        }
        BackgroundColorMode::DeltaIntensity => {
            let total = level.total_qty();
            let delta_ratio = if total > 0.0 {
                (level.buy_volume - level.sell_volume) / total
            } else {
                0.0
            };
            let color = if delta_ratio >= 0.0 {
                *buy_color
            } else {
                *sell_color
            };
            let alpha =
                (delta_ratio.abs() * bg_max_alpha).max(0.03);
            Some((color, alpha))
        }
        BackgroundColorMode::None => None,
    }
}
