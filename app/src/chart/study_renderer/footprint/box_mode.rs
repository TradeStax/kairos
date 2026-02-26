//! Box mode rendering and background color helpers for footprint.

use super::cell::{draw_cluster_text, draw_poc_highlight};
use super::scale::{format_value, scaled_ratio};
use iced::widget::canvas::{Frame, Path, Stroke};
use iced::{Alignment, Color, Point, Size};
use study::output::{
    BackgroundColorMode, FootprintDataType, FootprintLevel, FootprintScaling, TextFormat,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn draw_box_mode(
    frame: &mut Frame,
    price_to_y: &impl Fn(i64) -> f32,
    box_left: f32,
    box_width: f32,
    row_height: f32,
    max_cluster_qty: f32,
    palette: &iced::theme::palette::Extended,
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
    let text_color = custom_text_color.unwrap_or(palette.background.weakest.text);
    let box_center = box_left + box_width / 2.0;
    let buy_color = custom_buy_color.unwrap_or(palette.success.base.color);
    let sell_color = custom_sell_color.unwrap_or(palette.danger.base.color);

    let grid_stroke = if show_grid_lines {
        Some(Stroke::with_color(
            Stroke {
                width: 1.0,
                ..Default::default()
            },
            palette.background.weak.color.scale_alpha(0.3),
        ))
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
                    frame.fill_rectangle(
                        Point::new(box_left, bar_y),
                        Size::new(box_width / 2.0, row_height),
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
                        Size::new(box_width / 2.0, row_height),
                        color.scale_alpha(alpha),
                    );
                }

                if let Some(ref stroke) = grid_stroke {
                    frame.stroke(
                        &Path::rectangle(
                            Point::new(box_left, bar_y),
                            Size::new(box_width / 2.0, row_height),
                        ),
                        *stroke,
                    );
                    frame.stroke(
                        &Path::rectangle(
                            Point::new(box_center, bar_y),
                            Size::new(box_width / 2.0, row_height),
                        ),
                        *stroke,
                    );
                }

                if should_label(level.price) {
                    if level.sell_volume > 0.0 || show_zero {
                        draw_cluster_text(
                            frame,
                            &format_value(level.sell_volume, text_format),
                            Point::new(box_left + box_width * 0.25, y),
                            text_size,
                            text_color,
                            Alignment::Center,
                            Alignment::Center,
                        );
                    }
                    if level.buy_volume > 0.0 || show_zero {
                        draw_cluster_text(
                            frame,
                            &format_value(level.buy_volume, text_format),
                            Point::new(box_center + box_width * 0.25, y),
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

                if should_label(level.price) && (total > f32::EPSILON || show_zero) {
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
                    let actual_color = if delta >= 0.0 { buy_color } else { sell_color };
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

                if should_label(level.price) && (delta.abs() > f32::EPSILON || show_zero) {
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
            draw_poc_highlight(frame, box_left, y, box_width, row_height, palette);
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
    sell_color: &Color,
    buy_color: &Color,
) -> Option<(Color, f32)> {
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
