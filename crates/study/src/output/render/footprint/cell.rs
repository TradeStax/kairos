//! Per-cell and per-candle drawing for footprint clusters.

use data::Rgba;
use std::collections::BTreeSet;

use crate::output::{
    FootprintCandle, FootprintCandlePosition, FootprintData, FootprintDataType, FootprintLevel,
    FootprintRenderMode, OutsideBarStyle,
};

use super::super::canvas::Canvas;
use super::super::chart_view::ThemeColors;
use super::super::constants::{
    FP_BAR_ALPHA_WITH_TEXT, FP_BAR_WIDTH_FACTOR, FP_POC_HIGHLIGHT_ALPHA, FP_TEXT_BUDGET,
};
use super::super::types::{FontHint, LineStyle, TextAlign};
use super::box_mode::draw_box_mode;
use super::scale::{format_value, scaled_ratio};
use super::{BidAskArea, ClusterLayout, ClusterStyle, ProfileArea};

pub(super) fn draw_poc_highlight(
    canvas: &mut dyn Canvas,
    x: f32,
    y: f32,
    width: f32,
    cell_height: f32,
    theme: &ThemeColors,
) {
    canvas.fill_rect(
        x,
        y - (cell_height / 2.0),
        width,
        cell_height,
        theme.primary.scale_alpha(FP_POC_HIGHLIGHT_ALPHA),
    );
}

pub(super) fn text_budget_set(levels: &[FootprintLevel], show_text: bool) -> Option<BTreeSet<i64>> {
    if !show_text || levels.len() <= FP_TEXT_BUDGET {
        return None;
    }
    let mut ranked: Vec<(i64, f32)> = levels.iter().map(|l| (l.price, l.total_qty())).collect();
    ranked.select_nth_unstable_by(FP_TEXT_BUDGET - 1, |a, b| {
        b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)
    });
    ranked.truncate(FP_TEXT_BUDGET);
    Some(ranked.into_iter().map(|(p, _)| p).collect())
}

pub(super) fn draw_cluster_text(
    canvas: &mut dyn Canvas,
    text: &str,
    x: f32,
    y: f32,
    text_size: f32,
    color: Rgba,
    align_x: TextAlign,
    align_y: TextAlign,
) {
    canvas.fill_text_aligned(
        x,
        y,
        text,
        text_size,
        color,
        FontHint::Monospace,
        align_x,
        align_y,
    );
}

pub(super) fn draw_thin_candle(
    canvas: &mut dyn Canvas,
    fp_candle: &FootprintCandle,
    candle_center_x: f32,
    candle_width: f32,
    theme: &ThemeColors,
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
        theme.bullish_weak
    } else {
        theme.bearish_weak
    };

    let body_half = candle_width * bar_marker_width / 2.0;
    let body_x = candle_center_x - body_half;
    let body_w = body_half * 2.0;
    let body_top = y_open.min(y_close);
    let body_h = (y_open - y_close).abs();

    canvas.fill_rect(body_x, body_top, body_w, body_h, body_color);

    if show_outside_border {
        canvas.stroke_rect(
            body_x,
            body_top,
            body_w,
            body_h,
            body_color.scale_alpha(0.8),
            1.0,
        );
    }

    // Wicks only in Candle style
    if outside_bar_style == OutsideBarStyle::Candle {
        let wick_color = body_color.scale_alpha(0.6);
        canvas.stroke_line(
            candle_center_x,
            y_high,
            candle_center_x,
            y_low,
            wick_color,
            1.0,
            LineStyle::Solid,
        );
    }
}

// ── Main per-candle rendering ─────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
pub(super) fn draw_footprint_candle_clusters(
    canvas: &mut dyn Canvas,
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
    let theme = style.theme;
    let text_size = style.text_size;
    let show_text = style.show_text;

    let poc_price = poc_index.and_then(|i| levels.get(i)).map(|l| l.price);
    // Box mode: no text budget (cells don't overlap)
    let text_set = if data.mode == FootprintRenderMode::Box {
        None
    } else {
        text_budget_set(levels, show_text)
    };
    let show_zero = style.show_zero_values;
    let text_format = style.text_format;
    let should_label =
        |price: i64| show_text && text_set.as_ref().is_none_or(|s| s.contains(&price));

    // When skip_levels is true, only draw the thin candle (no levels)
    if skip_levels {
        if candle_position != FootprintCandlePosition::None {
            draw_thin_candle(
                canvas,
                fp_candle,
                x_position,
                candle_width,
                theme,
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
        let inset = (cell_width * (1.0 - FP_BAR_WIDTH_FACTOR)) / 2.0;
        let content_left = x_position - (cell_width / 2.0) + inset;
        let content_right = x_position + (cell_width / 2.0) - inset;

        let (box_left, box_width, candle_cx) = if candle_position == FootprintCandlePosition::None {
            (content_left, (content_right - content_left).max(0.0), 0.0)
        } else {
            let lane = candle_width * bar_marker_width;
            match candle_position {
                FootprintCandlePosition::Left => {
                    let bl = content_left + lane + spacing.candle_to_cluster;
                    (
                        bl,
                        (content_right - bl).max(0.0),
                        content_left + (lane / 2.0),
                    )
                }
                FootprintCandlePosition::Center => (
                    content_left,
                    (content_right - content_left).max(0.0),
                    x_position,
                ),
                FootprintCandlePosition::Right => {
                    let br = content_right - lane - spacing.candle_to_cluster;
                    (
                        content_left,
                        (br - content_left).max(0.0),
                        content_right - (lane / 2.0),
                    )
                }
                _ => (content_left, (content_right - content_left).max(0.0), 0.0),
            }
        };

        let to_rgba = |c: &data::SerializableColor| Rgba::new(c.r, c.g, c.b, c.a);

        draw_box_mode(
            canvas,
            price_to_y,
            box_left,
            box_width,
            row_height,
            max_cluster_qty,
            theme,
            text_size,
            levels,
            data.data_type,
            data.scaling,
            poc_price,
            &should_label,
            data.bg_color_mode,
            data.bg_max_alpha,
            data.bg_buy_color.as_ref().map(to_rgba),
            data.bg_sell_color.as_ref().map(to_rgba),
            data.text_color.as_ref().map(to_rgba),
            data.show_grid_lines,
            show_zero,
            text_format,
        );

        if candle_position != FootprintCandlePosition::None {
            draw_thin_candle(
                canvas,
                fp_candle,
                candle_cx,
                candle_width,
                theme,
                price_to_y,
                data.outside_bar_style,
                data.show_outside_border,
                bar_marker_width,
            );
        }
        return;
    }

    // Profile mode
    let to_rgba = |c: &data::SerializableColor| Rgba::new(c.r, c.g, c.b, c.a);
    let text_color = data.text_color.as_ref().map(to_rgba).unwrap_or(theme.text);
    let inset = (cell_width * (1.0 - FP_BAR_WIDTH_FACTOR)) / 2.0;
    let cell_left = x_position - (cell_width / 2.0);
    let content_left = cell_left + inset;
    let content_right = x_position + (cell_width / 2.0) - inset;

    let draw_candle_body = candle_position != FootprintCandlePosition::None;

    let buy_bar_color = data
        .bg_buy_color
        .as_ref()
        .map(to_rgba)
        .unwrap_or(theme.bullish_base);
    let sell_bar_color = data
        .bg_sell_color
        .as_ref()
        .map(to_rgba)
        .unwrap_or(theme.bearish_base);

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
            let bar_alpha = if show_text {
                FP_BAR_ALPHA_WITH_TEXT
            } else {
                1.0
            };

            for level in levels {
                let y = price_to_y(level.price);

                if poc_price == Some(level.price) {
                    draw_poc_highlight(
                        canvas,
                        area.bars_left,
                        y,
                        area.bars_width,
                        row_height,
                        theme,
                    );
                }

                match data.data_type {
                    FootprintDataType::Volume => {
                        let total_qty = level.total_qty();
                        let ratio = scaled_ratio(total_qty, max_cluster_qty, data.scaling);
                        let total_bar_len = ratio * area.bars_width;

                        if total_bar_len > 0.0 {
                            let buy_frac = level.buy_volume / total_qty;
                            let sell_len = (1.0 - buy_frac) * total_bar_len;
                            let buy_len = buy_frac * total_bar_len;
                            let bar_y = y - (row_height / 2.0);

                            if level.sell_volume > 0.0 {
                                canvas.fill_rect(
                                    area.bars_left,
                                    bar_y,
                                    sell_len,
                                    row_height,
                                    sell_bar_color.scale_alpha(bar_alpha),
                                );
                            }
                            if level.buy_volume > 0.0 {
                                canvas.fill_rect(
                                    area.bars_left + sell_len,
                                    bar_y,
                                    buy_len,
                                    row_height,
                                    buy_bar_color.scale_alpha(bar_alpha),
                                );
                            }
                        }

                        if should_label(level.price) && (show_zero || total_qty > f32::EPSILON) {
                            draw_cluster_text(
                                canvas,
                                &format_value(total_qty, text_format),
                                area.bars_left,
                                y,
                                text_size,
                                text_color,
                                TextAlign::Start,
                                TextAlign::Center,
                            );
                        }
                    }
                    FootprintDataType::Delta => {
                        let delta = level.delta_qty();
                        let ratio = scaled_ratio(delta.abs(), max_cluster_qty, data.scaling);
                        let bar_width = ratio * area.bars_width;

                        if bar_width > 0.0 {
                            let color = if delta >= 0.0 {
                                buy_bar_color.scale_alpha(bar_alpha)
                            } else {
                                sell_bar_color.scale_alpha(bar_alpha)
                            };
                            canvas.fill_rect(
                                area.bars_left,
                                y - (row_height / 2.0),
                                bar_width,
                                row_height,
                                color,
                            );
                        }

                        if should_label(level.price) && (show_zero || delta.abs() > f32::EPSILON) {
                            draw_cluster_text(
                                canvas,
                                &format_value(delta, text_format),
                                area.bars_left,
                                y,
                                text_size,
                                text_color,
                                TextAlign::Start,
                                TextAlign::Center,
                            );
                        }
                    }
                    _ => {}
                }
            }

            if draw_candle_body {
                draw_thin_candle(
                    canvas,
                    fp_candle,
                    area.candle_center_x,
                    candle_width,
                    theme,
                    price_to_y,
                    data.outside_bar_style,
                    data.show_outside_border,
                    bar_marker_width,
                );
            }
        }
        FootprintDataType::BidAskSplit | FootprintDataType::DeltaAndVolume => {
            let area = BidAskArea::new(
                x_position,
                content_left,
                content_right,
                candle_width,
                spacing,
                candle_position,
                bar_marker_width,
            );

            let bar_alpha = if show_text {
                FP_BAR_ALPHA_WITH_TEXT
            } else {
                1.0
            };
            let right_area_width = (area.bid_area_right - area.bid_area_left).max(0.0);
            let left_area_width = (area.ask_area_right - area.ask_area_left).max(0.0);

            for level in levels {
                let y = price_to_y(level.price);

                if poc_price == Some(level.price) {
                    draw_poc_highlight(
                        canvas,
                        area.ask_area_left,
                        y,
                        area.bid_area_right - area.ask_area_left,
                        row_height,
                        theme,
                    );
                }

                if level.buy_volume > 0.0 && right_area_width > 0.0 {
                    if should_label(level.price) && (show_zero || level.buy_volume > f32::EPSILON) {
                        draw_cluster_text(
                            canvas,
                            &format_value(level.buy_volume, text_format),
                            area.bid_area_left,
                            y,
                            text_size,
                            text_color,
                            TextAlign::Start,
                            TextAlign::Center,
                        );
                    }

                    let ratio = scaled_ratio(level.buy_volume, max_cluster_qty, data.scaling);
                    let bar_width = ratio * right_area_width;
                    if bar_width > 0.0 {
                        canvas.fill_rect(
                            area.bid_area_left,
                            y - (row_height / 2.0),
                            bar_width,
                            row_height,
                            buy_bar_color.scale_alpha(bar_alpha),
                        );
                    }
                }
                if (level.sell_volume > 0.0 || show_zero) && left_area_width > 0.0 {
                    if should_label(level.price) && (show_zero || level.sell_volume > f32::EPSILON)
                    {
                        draw_cluster_text(
                            canvas,
                            &format_value(level.sell_volume, text_format),
                            area.ask_area_right,
                            y,
                            text_size,
                            text_color,
                            TextAlign::End,
                            TextAlign::Center,
                        );
                    }

                    if level.sell_volume > 0.0 {
                        let ratio = scaled_ratio(level.sell_volume, max_cluster_qty, data.scaling);
                        let bar_width = ratio * left_area_width;
                        if bar_width > 0.0 {
                            canvas.fill_rect(
                                area.ask_area_right,
                                y - (row_height / 2.0),
                                -bar_width,
                                row_height,
                                sell_bar_color.scale_alpha(bar_alpha),
                            );
                        }
                    }
                }
            }

            if draw_candle_body {
                draw_thin_candle(
                    canvas,
                    fp_candle,
                    area.candle_center_x,
                    candle_width,
                    theme,
                    price_to_y,
                    data.outside_bar_style,
                    data.show_outside_border,
                    bar_marker_width,
                );
            }
        }
    }
}
