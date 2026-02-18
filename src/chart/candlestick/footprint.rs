use crate::style;
use data::util::abbr_large_numbers;
use data::{Candle, ChartBasis, ClusterKind, ClusterScaling};
use exchange::util::{Price, PriceStep};
use iced::theme::palette::Extended;
use iced::widget::canvas;
use iced::{Alignment, Point, Size};
use std::collections::BTreeMap;

use super::candle::draw_footprint_candle;
use super::{TradeGroup, domain_to_exchange_price};

/// Ratio of candle width used for the thin candle body in footprint mode
const CANDLE_BODY_WIDTH_RATIO: f32 = 0.25;
/// Ratio of cell width occupied by cluster bars (leaves inset on each side)
const BAR_WIDTH_FACTOR: f32 = 0.9;
/// Alpha for cluster bar backgrounds when text labels are visible
const BAR_ALPHA_WITH_TEXT: f32 = 0.25;

#[derive(Clone, Copy, Debug)]
pub struct ContentGaps {
    /// Space between imb. markers candle body
    pub marker_to_candle: f32,
    /// Space between candle body and clusters
    pub candle_to_cluster: f32,
    /// Inner space reserved between imb. markers and clusters (used for BidAsk)
    pub marker_to_bars: f32,
}

impl ContentGaps {
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

pub struct ProfileArea {
    pub imb_marker_left: f32,
    pub imb_marker_width: f32,
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
        has_imbalance: bool,
    ) -> Self {
        let candle_lane_left = if has_imbalance {
            content_left + candle_width + gaps.marker_to_candle
        } else {
            content_left
        };
        let candle_lane_width = candle_width * CANDLE_BODY_WIDTH_RATIO;

        let bars_left = candle_lane_left + candle_lane_width + gaps.candle_to_cluster;
        let bars_width = (content_right - bars_left).max(0.0);

        let candle_center_x = candle_lane_left + (candle_lane_width / 2.0);

        Self {
            imb_marker_left: content_left,
            imb_marker_width: if has_imbalance { candle_width } else { 0.0 },
            bars_left,
            bars_width,
            candle_center_x,
        }
    }
}

pub struct BidAskArea {
    pub bid_area_left: f32,
    pub bid_area_right: f32,
    pub ask_area_left: f32,
    pub ask_area_right: f32,
    pub candle_center_x: f32,
    pub imb_marker_width: f32,
}

impl BidAskArea {
    pub fn new(
        x_position: f32,
        content_left: f32,
        content_right: f32,
        candle_width: f32,
        spacing: ContentGaps,
    ) -> Self {
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
            imb_marker_width: candle_width,
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
    cluster_kind: ClusterKind,
) -> f32 {
    let individual_max = match cluster_kind {
        ClusterKind::BidAsk => footprint
            .values()
            .map(|group| group.buy_qty.max(group.sell_qty))
            .fold(0.0_f32, f32::max),
        ClusterKind::DeltaProfile => footprint
            .values()
            .map(|group| (group.buy_qty - group.sell_qty).abs())
            .fold(0.0_f32, f32::max),
        ClusterKind::VolumeProfile => footprint
            .values()
            .map(|group| group.buy_qty + group.sell_qty)
            .fold(0.0_f32, f32::max),
        ClusterKind::Delta | ClusterKind::Volume | ClusterKind::Trades => footprint
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
        ClusterScaling::Linear | ClusterScaling::Sqrt | ClusterScaling::Log => {
            // These are transformation modes, not max determination modes
            // Use visible_max as default
            safe(visible_max)
        }
    }
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
        font: style::AZERET_MONO,
        ..canvas::Text::default()
    });
}

fn draw_imbalance_markers(
    frame: &mut canvas::Frame,
    price_to_y: &impl Fn(Price) -> f32,
    footprint: &BTreeMap<Price, TradeGroup>,
    price: Price,
    sell_qty: f32,
    higher_price: Price,
    threshold: u8,
    color_scale: bool,
    ignore_zeros: bool,
    cell_height: f32,
    palette: &Extended,
    buyside_x: f32,
    sellside_x: f32,
    rect_width: f32,
) {
    if ignore_zeros && sell_qty <= 0.0 {
        return;
    }

    if let Some(group) = footprint.get(&higher_price) {
        let diagonal_buy_qty = group.buy_qty;

        if ignore_zeros && diagonal_buy_qty <= 0.0 {
            return;
        }

        let rect_height = cell_height / 2.0;

        let alpha_from_ratio = |ratio: f32| -> f32 {
            if color_scale {
                // Smooth color scale based on ratio
                (0.2 + 0.8 * (ratio - 1.0).min(1.0)).min(1.0)
            } else {
                1.0
            }
        };

        if diagonal_buy_qty >= sell_qty {
            let required_qty = sell_qty * (100 + threshold) as f32 / 100.0;
            if diagonal_buy_qty > required_qty {
                let ratio = diagonal_buy_qty / required_qty;
                let alpha = alpha_from_ratio(ratio);

                let y = price_to_y(higher_price);
                frame.fill_rectangle(
                    Point::new(buyside_x, y - (rect_height / 2.0)),
                    Size::new(rect_width, rect_height),
                    palette.success.weak.color.scale_alpha(alpha),
                );
            }
        } else {
            let required_qty = diagonal_buy_qty * (100 + threshold) as f32 / 100.0;
            if sell_qty > required_qty {
                let ratio = sell_qty / required_qty;
                let alpha = alpha_from_ratio(ratio);

                let y = price_to_y(price);
                frame.fill_rectangle(
                    Point::new(sellside_x, y - (rect_height / 2.0)),
                    Size::new(rect_width, rect_height),
                    palette.danger.weak.color.scale_alpha(alpha),
                );
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn draw_clusters(
    frame: &mut canvas::Frame,
    price_to_y: impl Fn(Price) -> f32,
    x_position: f32,
    cell_width: f32,
    cell_height: f32,
    candle_width: f32,
    max_cluster_qty: f32,
    palette: &Extended,
    text_size: f32,
    tick_size: f32,
    show_text: bool,
    imbalance: Option<(usize, Option<usize>, bool)>,
    candle: &Candle,
    footprint: &BTreeMap<Price, TradeGroup>,
    cluster_kind: ClusterKind,
    spacing: ContentGaps,
) {
    let text_color = palette.background.weakest.text;

    let inset = (cell_width * (1.0 - BAR_WIDTH_FACTOR)) / 2.0;

    let cell_left = x_position - (cell_width / 2.0);
    let content_left = cell_left + inset;
    let content_right = x_position + (cell_width / 2.0) - inset;

    match cluster_kind {
        ClusterKind::VolumeProfile | ClusterKind::DeltaProfile => {
            let area = ProfileArea::new(
                content_left,
                content_right,
                candle_width,
                spacing,
                imbalance.is_some(),
            );
            let bar_alpha = if show_text { BAR_ALPHA_WITH_TEXT } else { 1.0 };

            for (price, group) in footprint {
                let y = price_to_y(*price);

                match cluster_kind {
                    ClusterKind::VolumeProfile => {
                        crate::chart::draw_volume_bar(
                            frame,
                            area.bars_left,
                            y,
                            group.buy_qty,
                            group.sell_qty,
                            max_cluster_qty,
                            area.bars_width,
                            cell_height,
                            palette.success.base.color,
                            palette.danger.base.color,
                            bar_alpha,
                            true,
                        );

                        if show_text {
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
                    ClusterKind::DeltaProfile => {
                        let delta = group.delta_qty();
                        if show_text {
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

                        let bar_width = (delta.abs() / max_cluster_qty) * area.bars_width;
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
                    }
                    _ => {}
                }

                if let Some((threshold, color_scale, ignore_zeros)) = imbalance {
                    let step = PriceStep::from_f32(tick_size);
                    let higher_price =
                        Price::from_f32(price.to_f32() + tick_size).round_to_step(step);

                    let rect_w = ((area.imb_marker_width - 1.0) / 2.0).max(1.0);
                    let buyside_x = area.imb_marker_left + area.imb_marker_width - rect_w;
                    let sellside_x =
                        area.imb_marker_left + area.imb_marker_width - (2.0 * rect_w) - 1.0;

                    draw_imbalance_markers(
                        frame,
                        &price_to_y,
                        footprint,
                        *price,
                        group.sell_qty,
                        higher_price,
                        threshold as u8,
                        color_scale.is_some(),
                        ignore_zeros,
                        cell_height,
                        palette,
                        buyside_x,
                        sellside_x,
                        rect_w,
                    );
                }
            }

            draw_footprint_candle(
                frame,
                &price_to_y,
                area.candle_center_x,
                candle_width,
                candle,
                palette,
            );
        }
        ClusterKind::BidAsk => {
            let area = BidAskArea::new(
                x_position,
                content_left,
                content_right,
                candle_width,
                spacing,
            );

            let bar_alpha = if show_text { BAR_ALPHA_WITH_TEXT } else { 1.0 };

            let imb_marker_reserve = if imbalance.is_some() {
                ((area.imb_marker_width - 1.0) / 2.0).max(1.0)
            } else {
                0.0
            };

            let right_max_x =
                area.bid_area_right - imb_marker_reserve - (2.0 * spacing.marker_to_bars);
            let right_area_width = (right_max_x - area.bid_area_left).max(0.0);

            let left_min_x =
                area.ask_area_left + imb_marker_reserve + (2.0 * spacing.marker_to_bars);
            let left_area_width = (area.ask_area_right - left_min_x).max(0.0);

            for (price, group) in footprint {
                let y = price_to_y(*price);

                if group.buy_qty > 0.0 && right_area_width > 0.0 {
                    if show_text {
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

                    let bar_width = (group.buy_qty / max_cluster_qty) * right_area_width;
                    if bar_width > 0.0 {
                        frame.fill_rectangle(
                            Point::new(area.bid_area_left, y - (cell_height / 2.0)),
                            Size::new(bar_width, cell_height),
                            palette.success.base.color.scale_alpha(bar_alpha),
                        );
                    }
                }
                if group.sell_qty > 0.0 && left_area_width > 0.0 {
                    if show_text {
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

                    let bar_width = (group.sell_qty / max_cluster_qty) * left_area_width;
                    if bar_width > 0.0 {
                        frame.fill_rectangle(
                            Point::new(area.ask_area_right, y - (cell_height / 2.0)),
                            Size::new(-bar_width, cell_height),
                            palette.danger.base.color.scale_alpha(bar_alpha),
                        );
                    }
                }

                if let Some((threshold, color_scale, ignore_zeros)) = imbalance
                    && area.imb_marker_width > 0.0
                {
                    let step = PriceStep::from_f32(tick_size);
                    let higher_price =
                        Price::from_f32(price.to_f32() + tick_size).round_to_step(step);

                    let rect_width = ((area.imb_marker_width - 1.0) / 2.0).max(1.0);

                    let buyside_x = area.bid_area_right - rect_width - spacing.marker_to_bars;
                    let sellside_x = area.ask_area_left + spacing.marker_to_bars;

                    draw_imbalance_markers(
                        frame,
                        &price_to_y,
                        footprint,
                        *price,
                        group.sell_qty,
                        higher_price,
                        threshold as u8,
                        color_scale.is_some(),
                        ignore_zeros,
                        cell_height,
                        palette,
                        buyside_x,
                        sellside_x,
                        rect_width,
                    );
                }
            }

            draw_footprint_candle(
                frame,
                &price_to_y,
                area.candle_center_x,
                candle_width,
                candle,
                palette,
            );
        }
        ClusterKind::Delta | ClusterKind::Volume | ClusterKind::Trades => {
            // For simple cluster kinds, use BidAsk-style rendering
            let area = BidAskArea::new(
                x_position,
                content_left,
                content_right,
                candle_width,
                spacing,
            );

            let bar_alpha = if show_text { BAR_ALPHA_WITH_TEXT } else { 1.0 };

            let imb_marker_reserve = if imbalance.is_some() {
                ((area.imb_marker_width - 1.0) / 2.0).max(1.0)
            } else {
                0.0
            };

            let right_max_x =
                area.bid_area_right - imb_marker_reserve - (2.0 * spacing.marker_to_bars);
            let right_area_width = (right_max_x - area.bid_area_left).max(0.0);

            let left_min_x =
                area.ask_area_left + imb_marker_reserve + (2.0 * spacing.marker_to_bars);
            let left_area_width = (area.ask_area_right - left_min_x).max(0.0);

            for (price, group) in footprint {
                let y = price_to_y(*price);

                if group.buy_qty > 0.0 && right_area_width > 0.0 {
                    if show_text {
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

                    let bar_width = (group.buy_qty / max_cluster_qty) * right_area_width;
                    if bar_width > 0.0 {
                        frame.fill_rectangle(
                            Point::new(area.bid_area_left, y - (cell_height / 2.0)),
                            Size::new(bar_width, cell_height),
                            palette.success.base.color.scale_alpha(bar_alpha),
                        );
                    }
                }
                if group.sell_qty > 0.0 && left_area_width > 0.0 {
                    if show_text {
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

                    let bar_width = (group.sell_qty / max_cluster_qty) * left_area_width;
                    if bar_width > 0.0 {
                        frame.fill_rectangle(
                            Point::new(area.ask_area_right, y - (cell_height / 2.0)),
                            Size::new(-bar_width, cell_height),
                            palette.danger.base.color.scale_alpha(bar_alpha),
                        );
                    }
                }

                if let Some((threshold, color_scale, ignore_zeros)) = imbalance
                    && area.imb_marker_width > 0.0
                {
                    let step = PriceStep::from_f32(tick_size);
                    let higher_price =
                        Price::from_f32(price.to_f32() + tick_size).round_to_step(step);

                    let rect_width = ((area.imb_marker_width - 1.0) / 2.0).max(1.0);

                    let buyside_x = area.bid_area_right - rect_width - spacing.marker_to_bars;
                    let sellside_x = area.ask_area_left + spacing.marker_to_bars;

                    draw_imbalance_markers(
                        frame,
                        &price_to_y,
                        footprint,
                        *price,
                        group.sell_qty,
                        higher_price,
                        threshold as u8,
                        color_scale.is_some(),
                        ignore_zeros,
                        cell_height,
                        palette,
                        buyside_x,
                        sellside_x,
                        rect_width,
                    );
                }
            }

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

#[allow(clippy::too_many_arguments)]
pub fn draw_all_npocs(
    candles: &[Candle],
    trades: &[data::Trade],
    basis: &ChartBasis,
    frame: &mut canvas::Frame,
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
    let mut pocs: Vec<(usize, Price, f32)> = Vec::new(); // (candle_index, price, volume)

    for (idx, candle) in candles.iter().enumerate() {
        // Get trades for this candle using binary search
        let candle_start = candle.time.0;
        let candle_end = if idx + 1 < candles.len() {
            candles[idx + 1].time.0
        } else {
            candle.time.0 + 60000 // default 1 minute
        };

        // Find start index using binary search
        let start_idx = trades
            .binary_search_by_key(&candle_start, |t| t.time.0)
            .unwrap_or_else(|i| i);

        // Find end index using binary search on the remaining slice
        let end_idx = trades[start_idx..]
            .binary_search_by_key(&candle_end, |t| t.time.0)
            .map(|i| start_idx + i)
            .unwrap_or_else(|i| start_idx + i);

        let candle_trades = &trades[start_idx..end_idx];

        // Build volume profile for this candle
        let mut volume_profile: BTreeMap<Price, f32> = BTreeMap::new();
        for trade in candle_trades {
            let price_rounded = domain_to_exchange_price(trade.price).round_to_step(tick_size);
            *volume_profile.entry(price_rounded).or_insert(0.0) += trade.quantity.0 as f32;
        }

        // Find POC (price with max volume)
        if let Some((poc_price, poc_volume)) = volume_profile
            .iter()
            .max_by(|(_, v1), (_, v2)| v1.partial_cmp(v2).unwrap())
        {
            pocs.push((idx, *poc_price, *poc_volume));
        }
    }

    // Track naked POCs (POCs that haven't been revisited)
    let mut npocs: Vec<(usize, Price)> = Vec::new();

    for (idx, poc_price, _) in &pocs {
        let mut is_naked = true;

        // Check if price was revisited in next `lookback` candles
        for future_candle in candles.iter().take((idx + 1 + lookback).min(candles.len())).skip(idx + 1) {

            // Check if POC price is within future candle's range
            let future_low = domain_to_exchange_price(future_candle.low);
            let future_high = domain_to_exchange_price(future_candle.high);
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
    let (_filled_color, naked_color) = (
        palette.background.strong.color,
        if palette.is_dark {
            palette.warning.weak.color.scale_alpha(0.5)
        } else {
            palette.warning.strong.color
        },
    );

    let line_height = cell_height.min(2.0);
    let inset = (cell_width * (1.0 - BAR_WIDTH_FACTOR)) / 2.0;

    let candle_lane_factor: f32 = match cluster_kind {
        ClusterKind::VolumeProfile | ClusterKind::DeltaProfile => 0.25,
        ClusterKind::BidAsk => 1.0,
        ClusterKind::Delta | ClusterKind::Volume | ClusterKind::Trades => 1.0,
    };

    let start_x_for = |cell_center_x: f32| -> f32 {
        match cluster_kind {
            ClusterKind::BidAsk => cell_center_x + (candle_width / 2.0) + spacing.candle_to_cluster,
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
            ClusterKind::Delta | ClusterKind::Volume | ClusterKind::Trades => {
                cell_center_x + (candle_width / 2.0) + spacing.candle_to_cluster
            }
        }
    };

    let rightmost_x = interval_to_x(visible_latest);

    for (candle_idx, npoc_price) in npocs {
        // Get candle time/position
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
