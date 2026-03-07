//! Side-panel VBP rendering.
//!
//! Renders horizontal volume bars for a cumulative profile in a
//! separate vertical canvas sharing the main chart's Y axis.

use super::{ensure_resolved_cache, to_color};
use crate::output::render::canvas::Canvas;
use crate::output::render::chart_view::ChartView;
use crate::output::render::coord;
use crate::output::render::types::LineStyle;
use crate::output::{ProfileLevel, ProfileOutput, ProfileRenderConfig, VbpType};
use data::Rgba;

/// Render horizontal VBP bars for one profile in the side panel.
///
/// Y coordinates come from `view.price_units_to_y()` which the app
/// side maps through its `SidePanelChartView` implementation.
///
/// Max volume is computed from the **resolved** (quantum-merged) levels
/// so bars never overflow the canvas bounds.
pub fn render_side_panel_bars(
    canvas: &mut dyn Canvas,
    profile: &ProfileOutput,
    config: &ProfileRenderConfig,
    view: &dyn ChartView,
) {
    if profile.levels.is_empty() {
        return;
    }

    ensure_resolved_cache(profile, config, view);
    let cache_guard = profile
        .resolved_cache
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let Some(resolved) = cache_guard.as_ref() else {
        return;
    };

    if resolved.levels.is_empty() {
        return;
    }

    let max_volume = resolved
        .levels
        .iter()
        .map(|l| l.buy_volume + l.sell_volume)
        .fold(0.0_f32, f32::max);

    if max_volume <= 0.0 {
        return;
    }

    let bounds_width = view.bounds_width();
    let bounds_height = view.bounds_height();

    for (idx, level) in resolved.levels.iter().enumerate() {
        let vol = level.buy_volume + level.sell_volume;
        if vol <= 0.0 {
            continue;
        }

        let y = view.price_units_to_y(level.price_units);

        // Row height: the ChartView implementation handles the
        // scaling transform, so we estimate from adjacent levels.
        let row_h = if resolved.levels.len() >= 2 {
            let idx_next = (idx + 1).min(resolved.levels.len() - 1);
            let idx_prev = idx.saturating_sub(1);
            let y_prev = view.price_units_to_y(resolved.levels[idx_prev].price_units);
            let y_next = view.price_units_to_y(resolved.levels[idx_next].price_units);
            if idx_prev != idx_next {
                ((y_prev - y_next).abs()
                    / (idx_next - idx_prev) as f32)
                    .max(1.0)
            } else {
                1.0
            }
        } else {
            view.cell_height().max(1.0)
        };

        let top = y - row_h / 2.0;

        // Cull off-screen levels
        if top + row_h <= 0.0 || top >= bounds_height {
            continue;
        }

        // Bars grow left to right. Reserve a 2px gap.
        let usable = (bounds_width - 2.0).max(0.0);
        let bar_width = (vol / max_volume) * usable;
        if bar_width < 0.5 {
            continue;
        }

        let color = bar_color(level, idx, resolved.value_area, config);
        canvas.fill_rect(0.0, top, bar_width, row_h, color);
    }

    // POC line
    if config.poc_config.show_poc
        && let Some(poc_idx) = resolved.poc
        && let Some(level) = resolved.levels.get(poc_idx)
    {
        let y = view.price_units_to_y(level.price_units);
        let color = to_color(config.poc_config.poc_color, 1.0);
        let width = coord::effective_line_width(
            config.poc_config.poc_line_width,
            view.scaling(),
        );
        canvas.stroke_line(0.0, y, bounds_width, y, color, width, LineStyle::Solid);
    }

    // VAH / VAL lines
    if config.va_config.show_value_area
        && let Some((vah_idx, val_idx)) = resolved.value_area
    {
        for (idx, color_field, width_field) in [
            (
                vah_idx,
                config.va_config.vah_color,
                config.va_config.vah_line_width,
            ),
            (
                val_idx,
                config.va_config.val_color,
                config.va_config.val_line_width,
            ),
        ] {
            if let Some(level) = resolved.levels.get(idx) {
                let y = view.price_units_to_y(level.price_units);
                let color = to_color(color_field, 1.0);
                let width =
                    coord::effective_line_width(width_field, view.scaling());
                canvas.stroke_line(
                    0.0,
                    y,
                    bounds_width,
                    y,
                    color,
                    width,
                    LineStyle::Solid,
                );
            }
        }
    }
}

/// Choose the bar color based on VBP type and value area membership.
fn bar_color(
    level: &ProfileLevel,
    idx: usize,
    value_area: Option<(usize, usize)>,
    config: &ProfileRenderConfig,
) -> Rgba {
    let va_factor = if config.va_config.show_va_highlight {
        if let Some((vah, val)) = value_area {
            if idx >= val && idx <= vah { 1.0 } else { 0.4 }
        } else {
            1.0
        }
    } else {
        1.0
    };

    match config.vbp_type {
        VbpType::Volume | VbpType::DeltaAndTotalVolume => {
            to_color(config.volume_color, config.opacity * va_factor)
        }
        VbpType::BidAskVolume => {
            to_color(config.bid_color, config.opacity * va_factor)
        }
        VbpType::Delta | VbpType::DeltaPercentage => {
            let delta = level.buy_volume - level.sell_volume;
            if delta >= 0.0 {
                to_color(config.bid_color, config.opacity * va_factor)
            } else {
                to_color(config.ask_color, config.opacity * va_factor)
            }
        }
    }
}
