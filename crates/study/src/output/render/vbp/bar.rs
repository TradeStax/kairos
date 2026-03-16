//! VBP volume bar drawing modes (Volume, BidAsk, Delta, etc.).

use super::{draw_bar_left, draw_bar_right, to_color};
use crate::output::render::canvas::Canvas;
use crate::output::render::chart_view::ChartView;
use crate::output::{ProfileLevel, ProfileRenderConfig};

/// Determines opacity multiplier for a bar based on VA membership.
pub(super) fn va_factor(
    idx: usize,
    value_area: Option<(usize, usize)>,
    show_va_highlight: bool,
) -> f32 {
    if !show_va_highlight {
        return 1.0;
    }
    if let Some((vah_idx, val_idx)) = value_area {
        if idx >= val_idx && idx <= vah_idx {
            1.0
        } else {
            0.4
        }
    } else {
        1.0
    }
}

// -- Volume mode --

pub(super) fn draw_volume(
    canvas: &mut dyn Canvas,
    levels: &[ProfileLevel],
    value_area: Option<(usize, usize)>,
    config: &ProfileRenderConfig,
    view: &dyn ChartView,
    max_bar_length: f32,
    bar_height: f32,
    anchor_x: f32,
    all_levels: &[ProfileLevel],
) {
    let max_vol = all_levels
        .iter()
        .map(|l| l.buy_volume + l.sell_volume)
        .fold(0.0_f32, f32::max);
    if max_vol <= 0.0 {
        return;
    }

    for (idx, level) in levels.iter().enumerate() {
        let total = level.buy_volume + level.sell_volume;
        if total <= 0.0 {
            continue;
        }
        let y = view.price_units_to_y(level.price_units);
        let bar_len = (total / max_vol) * max_bar_length;
        let factor = va_factor(idx, value_area, config.va_config.show_va_highlight);
        let color = to_color(config.volume_color, config.opacity * factor);
        draw_bar_right(canvas, anchor_x, y, bar_height, bar_len, color);
    }
}

// -- Bid/Ask Volume mode --

pub(super) fn draw_bid_ask(
    canvas: &mut dyn Canvas,
    levels: &[ProfileLevel],
    value_area: Option<(usize, usize)>,
    config: &ProfileRenderConfig,
    view: &dyn ChartView,
    max_bar_length: f32,
    bar_height: f32,
    anchor_x: f32,
    all_levels: &[ProfileLevel],
) {
    let max_vol = all_levels
        .iter()
        .map(|l| l.buy_volume + l.sell_volume)
        .fold(0.0_f32, f32::max);
    if max_vol <= 0.0 {
        return;
    }

    for (idx, level) in levels.iter().enumerate() {
        let total = level.buy_volume + level.sell_volume;
        if total <= 0.0 {
            continue;
        }
        let y = view.price_units_to_y(level.price_units);
        let bar_len = (total / max_vol) * max_bar_length;
        let sell_len = (level.sell_volume / total) * bar_len;
        let buy_len = (level.buy_volume / total) * bar_len;
        let factor = va_factor(idx, value_area, config.va_config.show_va_highlight);
        let sell_color = to_color(config.ask_color, config.opacity * factor);
        let buy_color = to_color(config.bid_color, config.opacity * factor);
        let top = y - bar_height / 2.0;
        if sell_len > 0.0 {
            canvas.fill_rect(anchor_x, top, sell_len, bar_height, sell_color);
        }
        if buy_len > 0.0 {
            canvas.fill_rect(anchor_x + sell_len, top, buy_len, bar_height, buy_color);
        }
    }
}

// -- Delta mode --

pub(super) fn draw_delta(
    canvas: &mut dyn Canvas,
    levels: &[ProfileLevel],
    value_area: Option<(usize, usize)>,
    config: &ProfileRenderConfig,
    view: &dyn ChartView,
    max_bar_length: f32,
    bar_height: f32,
    anchor_x: f32,
    all_levels: &[ProfileLevel],
) {
    let max_abs_delta = all_levels
        .iter()
        .map(|l| (l.buy_volume - l.sell_volume).abs())
        .fold(0.0_f32, f32::max);
    if max_abs_delta <= 0.0 {
        return;
    }

    for (idx, level) in levels.iter().enumerate() {
        let delta = level.buy_volume - level.sell_volume;
        if delta.abs() < f32::EPSILON {
            continue;
        }
        let y = view.price_units_to_y(level.price_units);
        let bar_len = (delta.abs() / max_abs_delta) * max_bar_length;
        let factor = va_factor(idx, value_area, config.va_config.show_va_highlight);
        let color = if delta > 0.0 {
            to_color(config.bid_color, config.opacity * factor)
        } else {
            to_color(config.ask_color, config.opacity * factor)
        };
        draw_bar_left(canvas, anchor_x, y, bar_height, bar_len, color);
    }
}

// -- Delta & Total Volume (butterfly) mode --

pub(super) fn draw_delta_and_total(
    canvas: &mut dyn Canvas,
    levels: &[ProfileLevel],
    value_area: Option<(usize, usize)>,
    config: &ProfileRenderConfig,
    view: &dyn ChartView,
    max_bar_length: f32,
    bar_height: f32,
    anchor_x: f32,
    all_levels: &[ProfileLevel],
) {
    let max_vol = all_levels
        .iter()
        .map(|l| l.buy_volume + l.sell_volume)
        .fold(0.0_f32, f32::max);
    if max_vol <= 0.0 {
        return;
    }

    for (idx, level) in levels.iter().enumerate() {
        let total = level.buy_volume + level.sell_volume;
        if total <= 0.0 {
            continue;
        }
        let y = view.price_units_to_y(level.price_units);
        let top = y - bar_height / 2.0;
        let total_len = (total / max_vol) * max_bar_length;
        let factor = va_factor(idx, value_area, config.va_config.show_va_highlight);

        let vol_color = to_color(config.volume_color, config.opacity * factor);
        let sell_color = to_color(config.ask_color, config.opacity * factor);
        let buy_color = to_color(config.bid_color, config.opacity * factor);

        canvas.fill_rect(anchor_x, top, total_len, bar_height, vol_color);

        let sell_len = (level.sell_volume / total) * total_len;
        let buy_len = (level.buy_volume / total) * total_len;
        if sell_len > 0.0 {
            canvas.fill_rect(
                anchor_x - sell_len - buy_len,
                top,
                sell_len,
                bar_height,
                sell_color,
            );
        }
        if buy_len > 0.0 {
            canvas.fill_rect(anchor_x - buy_len, top, buy_len, bar_height, buy_color);
        }
    }
}

// -- Delta Percentage mode --

pub(super) fn draw_delta_pct(
    canvas: &mut dyn Canvas,
    levels: &[ProfileLevel],
    value_area: Option<(usize, usize)>,
    config: &ProfileRenderConfig,
    view: &dyn ChartView,
    max_bar_length: f32,
    bar_height: f32,
    anchor_x: f32,
) {
    for (idx, level) in levels.iter().enumerate() {
        let total = level.buy_volume + level.sell_volume;
        if total <= 0.0 {
            continue;
        }
        let delta = level.buy_volume - level.sell_volume;
        let pct = delta / total;
        if pct.abs() < f32::EPSILON {
            continue;
        }
        let y = view.price_units_to_y(level.price_units);
        let bar_len = pct.abs() * max_bar_length;
        let factor = va_factor(idx, value_area, config.va_config.show_va_highlight);
        let color = if pct > 0.0 {
            to_color(config.bid_color, config.opacity * factor)
        } else {
            to_color(config.ask_color, config.opacity * factor)
        };
        draw_bar_left(canvas, anchor_x, y, bar_height, bar_len, color);
    }
}
