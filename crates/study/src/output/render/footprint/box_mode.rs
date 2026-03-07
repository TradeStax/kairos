//! Box mode rendering and background color helpers for footprint.

use data::Rgba;

use crate::output::{
    BackgroundColorMode, FootprintDataType, FootprintLevel, FootprintScaling, TextFormat,
};

use super::super::canvas::Canvas;
use super::super::chart_view::ThemeColors;
use super::super::types::TextAlign;
use super::cell::{draw_cluster_text, draw_poc_highlight};
use super::scale::{format_value, scaled_ratio};

#[allow(clippy::too_many_arguments)]
pub(super) fn draw_box_mode(
    canvas: &mut dyn Canvas,
    price_to_y: &impl Fn(i64) -> f32,
    box_left: f32,
    box_width: f32,
    row_height: f32,
    max_cluster_qty: f32,
    theme: &ThemeColors,
    text_size: f32,
    levels: &[FootprintLevel],
    data_type: FootprintDataType,
    scaling: FootprintScaling,
    poc_price: Option<i64>,
    should_label: &dyn Fn(i64) -> bool,
    bg_color_mode: BackgroundColorMode,
    bg_max_alpha: f32,
    custom_buy_color: Option<Rgba>,
    custom_sell_color: Option<Rgba>,
    custom_text_color: Option<Rgba>,
    show_grid_lines: bool,
    show_zero: bool,
    text_format: TextFormat,
) {
    let text_color = custom_text_color.unwrap_or(theme.text);
    let box_center = box_left + box_width / 2.0;
    let buy_color = custom_buy_color.unwrap_or(theme.bullish_base);
    let sell_color = custom_sell_color.unwrap_or(theme.bearish_base);

    let grid_color = if show_grid_lines {
        Some(theme.background_weak.scale_alpha(0.3))
    } else {
        None
    };

    for level in levels {
        let y = price_to_y(level.price);
        let bar_y = y - (row_height / 2.0);

        match data_type {
            FootprintDataType::BidAskSplit | FootprintDataType::DeltaAndVolume => {
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
                    canvas.fill_rect(
                        box_left,
                        bar_y,
                        box_width / 2.0,
                        row_height,
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
                    canvas.fill_rect(
                        box_center,
                        bar_y,
                        box_width / 2.0,
                        row_height,
                        color.scale_alpha(alpha),
                    );
                }

                if let Some(gc) = grid_color {
                    canvas.stroke_rect(
                        box_left,
                        bar_y,
                        box_width / 2.0,
                        row_height,
                        gc,
                        1.0,
                    );
                    canvas.stroke_rect(
                        box_center,
                        bar_y,
                        box_width / 2.0,
                        row_height,
                        gc,
                        1.0,
                    );
                }

                if should_label(level.price) {
                    if level.sell_volume > 0.0 || show_zero {
                        draw_cluster_text(
                            canvas,
                            &format_value(level.sell_volume, text_format),
                            box_left + box_width * 0.25,
                            y,
                            text_size,
                            text_color,
                            TextAlign::Center,
                            TextAlign::Center,
                        );
                    }
                    if level.buy_volume > 0.0 || show_zero {
                        draw_cluster_text(
                            canvas,
                            &format_value(level.buy_volume, text_format),
                            box_center + box_width * 0.25,
                            y,
                            text_size,
                            text_color,
                            TextAlign::Center,
                            TextAlign::Center,
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
                    canvas.fill_rect(
                        box_left,
                        bar_y,
                        box_width,
                        row_height,
                        color.scale_alpha(alpha),
                    );
                }

                if let Some(gc) = grid_color {
                    canvas.stroke_rect(box_left, bar_y, box_width, row_height, gc, 1.0);
                }

                if should_label(level.price) && (total > f32::EPSILON || show_zero) {
                    draw_cluster_text(
                        canvas,
                        &format_value(total, text_format),
                        box_center,
                        y,
                        text_size,
                        text_color,
                        TextAlign::Center,
                        TextAlign::Center,
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
                    let actual_color = if delta >= 0.0 { buy_color } else { sell_color };
                    canvas.fill_rect(
                        box_left,
                        bar_y,
                        box_width,
                        row_height,
                        actual_color.scale_alpha(alpha),
                    );
                }

                if let Some(gc) = grid_color {
                    canvas.stroke_rect(box_left, bar_y, box_width, row_height, gc, 1.0);
                }

                if should_label(level.price)
                    && (delta.abs() > f32::EPSILON || show_zero)
                {
                    draw_cluster_text(
                        canvas,
                        &format_value(delta, text_format),
                        box_center,
                        y,
                        text_size,
                        text_color,
                        TextAlign::Center,
                        TextAlign::Center,
                    );
                }
            }
        }

        if poc_price == Some(level.price) {
            draw_poc_highlight(canvas, box_left, y, box_width, row_height, theme);
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
    sell_color: &Rgba,
    buy_color: &Rgba,
) -> Option<(Rgba, f32)> {
    match bg_mode {
        BackgroundColorMode::VolumeIntensity => {
            let ratio = scaled_ratio(volume, max_cluster_qty, scaling);
            let alpha = (ratio.min(1.0) * bg_max_alpha).max(0.03);
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
            let alpha = (delta_ratio.abs() * bg_max_alpha).max(0.03);
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
    sell_color: &Rgba,
    buy_color: &Rgba,
) -> Option<(Rgba, f32)> {
    match bg_mode {
        BackgroundColorMode::VolumeIntensity => {
            let ratio = scaled_ratio(qty, max_cluster_qty, scaling);
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
            let alpha = (ratio.min(1.0) * bg_max_alpha).max(0.03);
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
            let alpha = (delta_ratio.abs() * bg_max_alpha).max(0.03);
            Some((color, alpha))
        }
        BackgroundColorMode::None => None,
    }
}
