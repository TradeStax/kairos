use crate::components::primitives::AZERET_MONO;
use data::util::abbr_large_numbers;
use data::{Candle, CandlePosition, ClusterScaling, FootprintMode, FootprintType};
use exchange::util::Price;
use iced::theme::palette::Extended;
use iced::widget::canvas;
use iced::{Alignment, Point, Size};
use std::collections::{BTreeMap, BTreeSet};

use super::TradeGroup;
use super::candle::draw_footprint_candle;

/// Ratio of candle width used for the thin candle body in footprint mode
const CANDLE_BODY_WIDTH_RATIO: f32 = 0.25;
/// Ratio of cell width occupied by cluster bars (leaves inset on each side)
const BAR_WIDTH_FACTOR: f32 = 0.9;
/// Alpha for cluster bar backgrounds when text labels are visible
const BAR_ALPHA_WITH_TEXT: f32 = 0.25;
/// Alpha for POC (Point of Control) highlight background
const POC_HIGHLIGHT_ALPHA: f32 = 0.15;
/// Maximum number of price levels that receive text labels per candle
const TEXT_BUDGET: usize = 40;

#[derive(Clone, Copy, Debug)]
pub struct ContentGaps {
    /// Space between candle body and clusters
    pub candle_to_cluster: f32,
}

impl ContentGaps {
    pub fn from_view(candle_width: f32, scaling: f32) -> Self {
        let px = |p: f32| p / scaling;
        let base = (candle_width * 0.2).max(px(2.0));
        Self {
            candle_to_cluster: base,
        }
    }
}

pub struct ProfileArea {
    pub bars_left: f32,
    pub bars_width: f32,
    pub candle_center_x: f32,
}

impl ProfileArea {
    pub fn new(
        content_left: f32,
        content_right: f32,
        candle_width: f32,
        gaps: ContentGaps,
        position: CandlePosition,
    ) -> Self {
        let candle_lane_width = candle_width * CANDLE_BODY_WIDTH_RATIO;
        match position {
            CandlePosition::None => Self {
                bars_left: content_left,
                bars_width: (content_right - content_left).max(0.0),
                candle_center_x: 0.0,
            },
            CandlePosition::Left | CandlePosition::Center => {
                let bars_left = content_left + candle_lane_width + gaps.candle_to_cluster;
                Self {
                    bars_left,
                    bars_width: (content_right - bars_left).max(0.0),
                    candle_center_x: content_left + (candle_lane_width / 2.0),
                }
            }
            CandlePosition::Right => {
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

pub struct BidAskArea {
    pub bid_area_left: f32,
    pub bid_area_right: f32,
    pub ask_area_left: f32,
    pub ask_area_right: f32,
    pub candle_center_x: f32,
}

impl BidAskArea {
    pub fn new(
        x_position: f32,
        content_left: f32,
        content_right: f32,
        candle_width: f32,
        spacing: ContentGaps,
        candle_position: CandlePosition,
    ) -> Self {
        if candle_position == CandlePosition::None {
            return Self {
                bid_area_left: x_position,
                bid_area_right: content_right,
                ask_area_left: content_left,
                ask_area_right: x_position,
                candle_center_x: x_position,
            };
        }

        let candle_body_width = candle_width * CANDLE_BODY_WIDTH_RATIO;

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

#[inline]
pub fn should_show_text(cell_height_unscaled: f32, cell_width_unscaled: f32, min_w: f32) -> bool {
    cell_height_unscaled > 8.0 && cell_width_unscaled > min_w
}

pub fn effective_cluster_qty(
    scaling: ClusterScaling,
    visible_max: f32,
    footprint: &BTreeMap<Price, TradeGroup>,
    study_type: FootprintType,
) -> f32 {
    let individual_max = match study_type {
        FootprintType::BidAskSplit | FootprintType::DeltaAndVolume => footprint
            .values()
            .map(|group| group.buy_qty.max(group.sell_qty))
            .fold(0.0_f32, f32::max),
        FootprintType::Delta => footprint
            .values()
            .map(|group| (group.buy_qty - group.sell_qty).abs())
            .fold(0.0_f32, f32::max),
        FootprintType::Volume => footprint
            .values()
            .map(|group| group.buy_qty + group.sell_qty)
            .fold(0.0_f32, f32::max),
    };

    let safe = |v: f32| if v <= f32::EPSILON { 1.0 } else { v };

    match scaling {
        ClusterScaling::VisibleRange => safe(visible_max),
        ClusterScaling::Datapoint => safe(individual_max),
        ClusterScaling::Hybrid { weight } => {
            let w = weight.clamp(0.0, 1.0);
            safe(visible_max * w + individual_max * (1.0 - w))
        }
        ClusterScaling::Linear | ClusterScaling::Sqrt | ClusterScaling::Log => safe(visible_max),
    }
}

/// Apply the cluster scaling transform to compute a bar-width ratio (0..1).
///
/// `Linear` / `VisibleRange` / `Datapoint` / `Hybrid` use a simple linear
/// ratio.  `Sqrt` and `Log` compress large values so that smaller trades
/// remain visible alongside large ones.
#[inline]
fn scaled_ratio(qty: f32, max: f32, scaling: ClusterScaling) -> f32 {
    if max <= f32::EPSILON || qty <= f32::EPSILON {
        return 0.0;
    }
    match scaling {
        ClusterScaling::Sqrt => qty.sqrt() / max.sqrt(),
        ClusterScaling::Log => (1.0 + qty).ln() / (1.0 + max).ln(),
        _ => qty / max,
    }
}

/// Draw a subtle highlight behind the Point of Control price level.
fn draw_poc_highlight(
    frame: &mut canvas::Frame,
    x: f32,
    y: f32,
    width: f32,
    cell_height: f32,
    palette: &Extended,
) {
    frame.fill_rectangle(
        Point::new(x, y - (cell_height / 2.0)),
        Size::new(width, cell_height),
        palette.primary.base.color.scale_alpha(POC_HIGHLIGHT_ALPHA),
    );
}

/// Find the POC (price with highest total volume) in a footprint.
fn find_poc(footprint: &BTreeMap<Price, TradeGroup>) -> Option<Price> {
    footprint
        .iter()
        .max_by(|(_, a), (_, b)| {
            a.total_qty()
                .partial_cmp(&b.total_qty())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(price, _)| *price)
}

/// Build a text-budget set: when there are many price levels only the
/// top `TEXT_BUDGET` levels (by total volume) receive text labels.
///
/// Uses `select_nth_unstable_by` for O(n) partial selection instead
/// of a full O(n log n) sort.
fn text_budget_set(
    footprint: &BTreeMap<Price, TradeGroup>,
    show_text: bool,
) -> Option<BTreeSet<Price>> {
    if !show_text || footprint.len() <= TEXT_BUDGET {
        return None;
    }
    let mut levels: Vec<(Price, f32)> =
        footprint.iter().map(|(p, g)| (*p, g.total_qty())).collect();
    // Partial sort: partition so that the top TEXT_BUDGET items are in levels[..TEXT_BUDGET]
    levels.select_nth_unstable_by(TEXT_BUDGET - 1, |a, b| {
        b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)
    });
    levels.truncate(TEXT_BUDGET);
    Some(levels.into_iter().map(|(p, _)| p).collect())
}

fn draw_cluster_text(
    frame: &mut canvas::Frame,
    text: &str,
    position: Point,
    text_size: f32,
    color: iced::Color,
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

// ── Box mode rendering ────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn draw_box_mode(
    frame: &mut canvas::Frame,
    price_to_y: &impl Fn(Price) -> f32,
    x_position: f32,
    cell_width: f32,
    cell_height: f32,
    max_cluster_qty: f32,
    palette: &Extended,
    text_size: f32,
    footprint: &BTreeMap<Price, TradeGroup>,
    study_type: FootprintType,
    scaling: ClusterScaling,
    poc_price: Option<Price>,
    should_label: &dyn Fn(&Price) -> bool,
) {
    let text_color = palette.background.weakest.text;
    let cell_left = x_position - (cell_width / 2.0);

    for (price, group) in footprint {
        let y = price_to_y(*price);
        let bar_y = y - (cell_height / 2.0);

        match study_type {
            FootprintType::BidAskSplit | FootprintType::DeltaAndVolume => {
                // Left half: sells/asks
                let sell_ratio = scaled_ratio(group.sell_qty, max_cluster_qty, scaling);
                let sell_alpha = (sell_ratio.min(1.0) * 0.6).max(0.03);
                frame.fill_rectangle(
                    Point::new(cell_left, bar_y),
                    Size::new(cell_width / 2.0, cell_height),
                    palette.danger.base.color.scale_alpha(sell_alpha),
                );

                // Right half: buys/bids
                let buy_ratio = scaled_ratio(group.buy_qty, max_cluster_qty, scaling);
                let buy_alpha = (buy_ratio.min(1.0) * 0.6).max(0.03);
                frame.fill_rectangle(
                    Point::new(x_position, bar_y),
                    Size::new(cell_width / 2.0, cell_height),
                    palette.success.base.color.scale_alpha(buy_alpha),
                );

                if should_label(price) {
                    if group.sell_qty > 0.0 {
                        draw_cluster_text(
                            frame,
                            &abbr_large_numbers(group.sell_qty),
                            Point::new(cell_left + cell_width * 0.25, y),
                            text_size,
                            text_color,
                            Alignment::Center,
                            Alignment::Center,
                        );
                    }
                    if group.buy_qty > 0.0 {
                        draw_cluster_text(
                            frame,
                            &abbr_large_numbers(group.buy_qty),
                            Point::new(x_position + cell_width * 0.25, y),
                            text_size,
                            text_color,
                            Alignment::Center,
                            Alignment::Center,
                        );
                    }
                }
            }
            FootprintType::Volume => {
                let total = group.total_qty();
                let ratio = scaled_ratio(total, max_cluster_qty, scaling);
                let buy_frac = if total > 0.0 {
                    group.buy_qty / total
                } else {
                    0.5
                };
                let color = if buy_frac >= 0.5 {
                    palette.success.base.color
                } else {
                    palette.danger.base.color
                };
                let alpha = (ratio.min(1.0) * 0.6).max(0.03);
                frame.fill_rectangle(
                    Point::new(cell_left, bar_y),
                    Size::new(cell_width, cell_height),
                    color.scale_alpha(alpha),
                );
                if should_label(price) {
                    draw_cluster_text(
                        frame,
                        &abbr_large_numbers(total),
                        Point::new(x_position, y),
                        text_size,
                        text_color,
                        Alignment::Center,
                        Alignment::Center,
                    );
                }
            }
            FootprintType::Delta => {
                let delta = group.delta_qty();
                let ratio = scaled_ratio(delta.abs(), max_cluster_qty, scaling);
                let color = if delta >= 0.0 {
                    palette.success.base.color
                } else {
                    palette.danger.base.color
                };
                let alpha = (ratio.min(1.0) * 0.6).max(0.03);
                frame.fill_rectangle(
                    Point::new(cell_left, bar_y),
                    Size::new(cell_width, cell_height),
                    color.scale_alpha(alpha),
                );
                if should_label(price) {
                    draw_cluster_text(
                        frame,
                        &abbr_large_numbers(delta),
                        Point::new(x_position, y),
                        text_size,
                        text_color,
                        Alignment::Center,
                        Alignment::Center,
                    );
                }
            }
        }

        // POC highlight
        if poc_price == Some(*price) {
            draw_poc_highlight(frame, cell_left, y, cell_width, cell_height, palette);
        }
    }
}

// ── Profile mode rendering (bars extending from candle) ───────────────

/// Position and sizing parameters for cluster rendering.
pub struct ClusterLayout {
    pub x_position: f32,
    pub cell_width: f32,
    pub cell_height: f32,
    pub candle_width: f32,
    pub candle_position: CandlePosition,
    pub spacing: ContentGaps,
}

/// Visual style parameters for cluster rendering.
pub struct ClusterStyle<'a> {
    pub palette: &'a Extended,
    pub text_size: f32,
    pub show_text: bool,
}

pub fn draw_clusters(
    frame: &mut canvas::Frame,
    price_to_y: impl Fn(Price) -> f32,
    layout: &ClusterLayout,
    style: &ClusterStyle<'_>,
    max_cluster_qty: f32,
    candle: &Candle,
    footprint: &BTreeMap<Price, TradeGroup>,
    study_type: FootprintType,
    scaling: ClusterScaling,
    mode: FootprintMode,
) {
    let x_position = layout.x_position;
    let cell_width = layout.cell_width;
    let cell_height = layout.cell_height;
    let candle_width = layout.candle_width;
    let candle_position = layout.candle_position;
    let spacing = layout.spacing;
    let palette = style.palette;
    let text_size = style.text_size;
    let show_text = style.show_text;

    let poc_price = find_poc(footprint);
    let text_set = text_budget_set(footprint, show_text);
    let should_label =
        |price: &Price| show_text && text_set.as_ref().map_or(true, |s| s.contains(price));

    // Box mode: colored grid cells with centered text
    if mode == FootprintMode::Box {
        draw_box_mode(
            frame,
            &price_to_y,
            x_position,
            cell_width,
            cell_height,
            max_cluster_qty,
            palette,
            text_size,
            footprint,
            study_type,
            scaling,
            poc_price,
            &should_label,
        );
        return;
    }

    // Profile mode: bars extending from candle
    let text_color = palette.background.weakest.text;

    let inset = (cell_width * (1.0 - BAR_WIDTH_FACTOR)) / 2.0;
    let cell_left = x_position - (cell_width / 2.0);
    let content_left = cell_left + inset;
    let content_right = x_position + (cell_width / 2.0) - inset;

    let draw_candle_body = candle_position != CandlePosition::None;

    match study_type {
        FootprintType::Volume | FootprintType::Delta => {
            let area = ProfileArea::new(
                content_left,
                content_right,
                candle_width,
                spacing,
                candle_position,
            );
            let bar_alpha = if show_text { BAR_ALPHA_WITH_TEXT } else { 1.0 };

            for (price, group) in footprint {
                let y = price_to_y(*price);

                // POC highlight
                if poc_price == Some(*price) {
                    draw_poc_highlight(
                        frame,
                        area.bars_left,
                        y,
                        area.bars_width,
                        cell_height,
                        palette,
                    );
                }

                match study_type {
                    FootprintType::Volume => {
                        let total_qty = group.total_qty();
                        let ratio = scaled_ratio(total_qty, max_cluster_qty, scaling);
                        let total_bar_len = ratio * area.bars_width;

                        if total_bar_len > 0.0 {
                            let buy_frac = group.buy_qty / total_qty;
                            let sell_len = (1.0 - buy_frac) * total_bar_len;
                            let buy_len = buy_frac * total_bar_len;
                            let bar_y = y - (cell_height / 2.0);

                            if group.sell_qty > 0.0 {
                                frame.fill_rectangle(
                                    Point::new(area.bars_left, bar_y),
                                    Size::new(sell_len, cell_height),
                                    palette.danger.base.color.scale_alpha(bar_alpha),
                                );
                            }
                            if group.buy_qty > 0.0 {
                                frame.fill_rectangle(
                                    Point::new(area.bars_left + sell_len, bar_y),
                                    Size::new(buy_len, cell_height),
                                    palette.success.base.color.scale_alpha(bar_alpha),
                                );
                            }
                        }

                        if should_label(price) {
                            draw_cluster_text(
                                frame,
                                &abbr_large_numbers(group.total_qty()),
                                Point::new(area.bars_left, y),
                                text_size,
                                text_color,
                                Alignment::Start,
                                Alignment::Center,
                            );
                        }
                    }
                    FootprintType::Delta => {
                        let delta = group.delta_qty();
                        let ratio = scaled_ratio(delta.abs(), max_cluster_qty, scaling);
                        let bar_width = ratio * area.bars_width;

                        if bar_width > 0.0 {
                            let color = if delta >= 0.0 {
                                palette.success.base.color.scale_alpha(bar_alpha)
                            } else {
                                palette.danger.base.color.scale_alpha(bar_alpha)
                            };
                            frame.fill_rectangle(
                                Point::new(area.bars_left, y - (cell_height / 2.0)),
                                Size::new(bar_width, cell_height),
                                color,
                            );
                        }

                        if should_label(price) {
                            draw_cluster_text(
                                frame,
                                &abbr_large_numbers(delta),
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
                draw_footprint_candle(
                    frame,
                    &price_to_y,
                    area.candle_center_x,
                    candle_width,
                    candle,
                    palette,
                );
            }
        }
        FootprintType::BidAskSplit | FootprintType::DeltaAndVolume => {
            let area = BidAskArea::new(
                x_position,
                content_left,
                content_right,
                candle_width,
                spacing,
                candle_position,
            );

            let bar_alpha = if show_text { BAR_ALPHA_WITH_TEXT } else { 1.0 };

            let right_area_width = (area.bid_area_right - area.bid_area_left).max(0.0);
            let left_area_width = (area.ask_area_right - area.ask_area_left).max(0.0);

            for (price, group) in footprint {
                let y = price_to_y(*price);

                // POC highlight
                if poc_price == Some(*price) {
                    draw_poc_highlight(
                        frame,
                        area.ask_area_left,
                        y,
                        area.bid_area_right - area.ask_area_left,
                        cell_height,
                        palette,
                    );
                }

                if group.buy_qty > 0.0 && right_area_width > 0.0 {
                    if should_label(price) {
                        draw_cluster_text(
                            frame,
                            &abbr_large_numbers(group.buy_qty),
                            Point::new(area.bid_area_left, y),
                            text_size,
                            text_color,
                            Alignment::Start,
                            Alignment::Center,
                        );
                    }

                    let ratio = scaled_ratio(group.buy_qty, max_cluster_qty, scaling);
                    let bar_width = ratio * right_area_width;
                    if bar_width > 0.0 {
                        frame.fill_rectangle(
                            Point::new(area.bid_area_left, y - (cell_height / 2.0)),
                            Size::new(bar_width, cell_height),
                            palette.success.base.color.scale_alpha(bar_alpha),
                        );
                    }
                }
                if group.sell_qty > 0.0 && left_area_width > 0.0 {
                    if should_label(price) {
                        draw_cluster_text(
                            frame,
                            &abbr_large_numbers(group.sell_qty),
                            Point::new(area.ask_area_right, y),
                            text_size,
                            text_color,
                            Alignment::End,
                            Alignment::Center,
                        );
                    }

                    let ratio = scaled_ratio(group.sell_qty, max_cluster_qty, scaling);
                    let bar_width = ratio * left_area_width;
                    if bar_width > 0.0 {
                        frame.fill_rectangle(
                            Point::new(area.ask_area_right, y - (cell_height / 2.0)),
                            Size::new(-bar_width, cell_height),
                            palette.danger.base.color.scale_alpha(bar_alpha),
                        );
                    }
                }
            }

            if draw_candle_body {
                draw_footprint_candle(
                    frame,
                    &price_to_y,
                    area.candle_center_x,
                    candle_width,
                    candle,
                    palette,
                );
            }
        }
    }
}
