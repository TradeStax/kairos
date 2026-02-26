//! VBP volume bar drawing modes (Volume, BidAsk, Delta, etc.).

use super::{draw_bar_left, draw_bar_right, to_iced_color};
use crate::chart::ViewState;
use data::Price;
use iced::widget::canvas::Frame;
use iced::{Point, Size};
use study::output::{ProfileLevel, ProfileRenderConfig};

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

// ── Volume mode ─────────────────────────────────────────────────────

pub(super) fn draw_volume(
    frame: &mut Frame,
    levels: &[ProfileLevel],
    value_area: Option<(usize, usize)>,
    config: &ProfileRenderConfig,
    state: &ViewState,
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
        let y = state.price_to_y(Price::from_units(level.price_units));
        let bar_len = (total / max_vol) * max_bar_length;
        let factor = va_factor(idx, value_area, config.va_config.show_va_highlight);
        let color = to_iced_color(config.volume_color, config.opacity * factor);
        draw_bar_right(frame, anchor_x, y, bar_height, bar_len, color);
    }
}

// ── Bid/Ask Volume mode ─────────────────────────────────────────────

pub(super) fn draw_bid_ask(
    frame: &mut Frame,
    levels: &[ProfileLevel],
    value_area: Option<(usize, usize)>,
    config: &ProfileRenderConfig,
    state: &ViewState,
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
        let y = state.price_to_y(Price::from_units(level.price_units));
        let bar_len = (total / max_vol) * max_bar_length;
        let sell_len = (level.sell_volume / total) * bar_len;
        let buy_len = (level.buy_volume / total) * bar_len;
        let factor = va_factor(idx, value_area, config.va_config.show_va_highlight);
        let sell_color = to_iced_color(config.ask_color, config.opacity * factor);
        let buy_color = to_iced_color(config.bid_color, config.opacity * factor);
        let top = y - bar_height / 2.0;
        if sell_len > 0.0 {
            frame.fill_rectangle(
                Point::new(anchor_x, top),
                Size::new(sell_len, bar_height),
                sell_color,
            );
        }
        if buy_len > 0.0 {
            frame.fill_rectangle(
                Point::new(anchor_x + sell_len, top),
                Size::new(buy_len, bar_height),
                buy_color,
            );
        }
    }
}

// ── Delta mode ──────────────────────────────────────────────────────

pub(super) fn draw_delta(
    frame: &mut Frame,
    levels: &[ProfileLevel],
    value_area: Option<(usize, usize)>,
    config: &ProfileRenderConfig,
    state: &ViewState,
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
        let y = state.price_to_y(Price::from_units(level.price_units));
        let bar_len = (delta.abs() / max_abs_delta) * max_bar_length;
        let factor = va_factor(idx, value_area, config.va_config.show_va_highlight);
        let color = if delta > 0.0 {
            to_iced_color(config.bid_color, config.opacity * factor)
        } else {
            to_iced_color(config.ask_color, config.opacity * factor)
        };
        draw_bar_left(frame, anchor_x, y, bar_height, bar_len, color);
    }
}

// ── Delta & Total Volume (butterfly) mode ───────────────────────────

pub(super) fn draw_delta_and_total(
    frame: &mut Frame,
    levels: &[ProfileLevel],
    value_area: Option<(usize, usize)>,
    config: &ProfileRenderConfig,
    state: &ViewState,
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
        let y = state.price_to_y(Price::from_units(level.price_units));
        let top = y - bar_height / 2.0;
        let total_len = (total / max_vol) * max_bar_length;
        let factor = va_factor(idx, value_area, config.va_config.show_va_highlight);

        let vol_color = to_iced_color(config.volume_color, config.opacity * factor);
        let sell_color = to_iced_color(config.ask_color, config.opacity * factor);
        let buy_color = to_iced_color(config.bid_color, config.opacity * factor);

        frame.fill_rectangle(
            Point::new(anchor_x, top),
            Size::new(total_len, bar_height),
            vol_color,
        );

        let sell_len = (level.sell_volume / total) * total_len;
        let buy_len = (level.buy_volume / total) * total_len;
        if sell_len > 0.0 {
            frame.fill_rectangle(
                Point::new(anchor_x - sell_len - buy_len, top),
                Size::new(sell_len, bar_height),
                sell_color,
            );
        }
        if buy_len > 0.0 {
            frame.fill_rectangle(
                Point::new(anchor_x - buy_len, top),
                Size::new(buy_len, bar_height),
                buy_color,
            );
        }
    }
}

// ── Delta Percentage mode ───────────────────────────────────────────

pub(super) fn draw_delta_pct(
    frame: &mut Frame,
    levels: &[ProfileLevel],
    value_area: Option<(usize, usize)>,
    config: &ProfileRenderConfig,
    state: &ViewState,
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
        let y = state.price_to_y(Price::from_units(level.price_units));
        let bar_len = pct.abs() * max_bar_length;
        let factor = va_factor(idx, value_area, config.va_config.show_va_highlight);
        let color = if pct > 0.0 {
            to_iced_color(config.bid_color, config.opacity * factor)
        } else {
            to_iced_color(config.ask_color, config.opacity * factor)
        };
        draw_bar_left(frame, anchor_x, y, bar_height, bar_len, color);
    }
}
