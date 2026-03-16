//! VBP (Volume-by-Price) renderer
//!
//! Renders horizontal volume bars anchored to the candle time range.
//! Multi-pass rendering with clear draw order:
//!
//! 1.  VA fill rectangle (behind everything)
//!     1b. HVN zone fills
//!     1c. LVN zone fills
//! 2.  Volume bars (5 modes, with VA dimming)
//! 3.  VAH/VAL lines
//! 4.  Peak line (single dominant HVN)
//!     4b. Valley line (single deepest LVN)
//! 5.  POC line (enhanced with style/extension)
//! 6.  Developing POC polyline
//!     6b. Developing Peak polyline
//!     6c. Developing Valley polyline
//! 7.  VWAP line + bands
//! 8.  Bounding rect outline
//! 9.  Price labels (POC, VA, Peak, Valley, VWAP)

mod annotation;
mod bar;
pub mod side_panel;

use crate::output::render::canvas::Canvas;
use crate::output::render::chart_view::ChartView;
use crate::output::render::constants::{VBP_LABEL_FONT_SIZE, VBP_MIN_ROW_PX};
use crate::output::render::coord;
use crate::output::render::types::{FontHint, LineStyle};
use crate::output::{
    ExtendDirection, ProfileLevel, ProfileOutput, ProfileRenderConfig, VbpGroupingMode,
    VbpResolvedCache, VbpType,
};
use crate::studies::orderflow::vbp::profile_core;
use data::{Price, Rgba};

use annotation::{
    draw_bounding_rect, draw_developing_line, draw_developing_poc, draw_poc_enhanced,
    draw_price_labels, draw_va_fill, draw_va_lines, draw_vwap, draw_zone_fills,
};
use bar::{draw_bid_ask, draw_delta, draw_delta_and_total, draw_delta_pct, draw_volume};

/// Convert a `SerializableColor` to `Rgba`, applying an opacity multiplier.
fn to_color(sc: data::SerializableColor, opacity: f32) -> Rgba {
    sc.scale_alpha(opacity)
}

/// Merge profile levels to a coarser quantum boundary.
pub(crate) fn merge_levels_to_quantum(
    levels: &[ProfileLevel],
    target_quantum: i64,
) -> Vec<ProfileLevel> {
    if levels.is_empty() {
        return Vec::new();
    }

    let mut merged = Vec::with_capacity(levels.len() / 2 + 1);
    let mut cur_bucket = (levels[0].price_units / target_quantum) * target_quantum;
    let mut buy_acc: f64 = 0.0;
    let mut sell_acc: f64 = 0.0;

    for level in levels {
        let bucket = (level.price_units / target_quantum) * target_quantum;
        if bucket != cur_bucket {
            merged.push(ProfileLevel {
                price: Price::from_units(cur_bucket).to_f64(),
                price_units: cur_bucket,
                buy_volume: buy_acc as f32,
                sell_volume: sell_acc as f32,
            });
            cur_bucket = bucket;
            buy_acc = 0.0;
            sell_acc = 0.0;
        }
        buy_acc += level.buy_volume as f64;
        sell_acc += level.sell_volume as f64;
    }
    merged.push(ProfileLevel {
        price: Price::from_units(cur_bucket).to_f64(),
        price_units: cur_bucket,
        buy_volume: buy_acc as f32,
        sell_volume: sell_acc as f32,
    });

    merged
}

/// Ensure the resolved cache is populated and up-to-date.
pub(crate) fn ensure_resolved_cache(
    output: &ProfileOutput,
    config: &ProfileRenderConfig,
    view: &dyn ChartView,
) {
    let tick_units = view.tick_size_units().max(1);

    let target_quantum = match output.grouping_mode {
        VbpGroupingMode::Automatic { factor } => {
            let dq = coord::compute_dynamic_quantum(
                view.cell_height(),
                view.scaling(),
                VBP_MIN_ROW_PX,
                factor,
                tick_units,
            );
            if dq > output.quantum {
                dq
            } else {
                output.quantum
            }
        }
        VbpGroupingMode::Manual => output.quantum,
    };

    {
        let cache = output
            .resolved_cache
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if let Some(ref c) = *cache
            && c.quantum == target_quantum
        {
            return;
        }
    }

    let (levels, poc, value_area) = if target_quantum > output.quantum {
        let merged = merge_levels_to_quantum(&output.levels, target_quantum);
        let poc = profile_core::find_poc_index(&merged);
        let value_area = if config.va_config.show_value_area {
            poc.and_then(|idx| {
                profile_core::calculate_value_area(
                    &merged,
                    idx,
                    config.va_config.value_area_pct as f64,
                )
            })
        } else {
            None
        };
        (merged, poc, value_area)
    } else {
        (output.levels.clone(), output.poc, output.value_area)
    };

    *output
        .resolved_cache
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = Some(VbpResolvedCache {
        quantum: target_quantum,
        levels,
        poc,
        value_area,
    });
}

/// Compute the x-range from a profile's time_range.
pub fn profile_x_range(output: &ProfileOutput, view: &dyn ChartView) -> (f32, f32) {
    match output.time_range {
        Some((start_ms, end_ms)) => {
            let x0 = view.interval_to_x(start_ms);
            let x1 = view.interval_to_x(end_ms);
            (x0.min(x1), x0.max(x1))
        }
        None => {
            let fw = view.bounds_width() / view.scaling();
            (-fw / 2.0, fw / 2.0)
        }
    }
}

/// Render multiple VBP profile segments onto the chart canvas.
///
/// Skips profiles whose time range falls entirely off-screen.
pub fn render_vbp_multi(
    canvas: &mut dyn Canvas,
    profiles: &[ProfileOutput],
    config: &ProfileRenderConfig,
    view: &dyn ChartView,
) {
    let region = view.visible_region();
    let vis_left = region.x;
    let vis_right = region.x + region.width;

    for profile in profiles {
        let (ax, br) = profile_x_range(profile, view);
        // Skip profiles entirely outside the visible region
        if br < vis_left || ax > vis_right {
            continue;
        }
        render_vbp(canvas, profile, config, view, ax, br);
    }
}

/// Render a single VBP profile output onto the chart canvas.
///
/// `anchor_x` and `box_right` define the horizontal extent of
/// this profile segment in chart-space coordinates. Use
/// [`profile_x_range`] to compute these from the profile's
/// `time_range`.
pub fn render_vbp(
    canvas: &mut dyn Canvas,
    output: &ProfileOutput,
    config: &ProfileRenderConfig,
    view: &dyn ChartView,
    anchor_x: f32,
    box_right: f32,
) {
    if output.levels.is_empty() {
        return;
    }

    ensure_resolved_cache(output, config, view);
    let cache_ref = output
        .resolved_cache
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let Some(resolved) = cache_ref.as_ref() else {
        return;
    };

    if resolved.levels.is_empty() {
        return;
    }

    let segment_width = (box_right - anchor_x).abs();
    let max_bar_length = segment_width * config.width_pct;

    // Estimate bar height from adjacent price levels
    let bar_height = if resolved.levels.len() >= 2 {
        let y0 = view.price_units_to_y(resolved.levels[0].price_units);
        let y1 = view.price_units_to_y(resolved.levels[1].price_units);
        (y1 - y0).abs().max(1.0)
    } else {
        view.cell_height().max(1.0)
    };

    // Compute Y bounds for the profile
    let y_top = price_to_y_top(&resolved.levels, view, bar_height);
    let y_bottom = price_to_y_bottom(&resolved.levels, view, bar_height);

    // Cull off-screen levels
    let region = view.visible_region();
    let vis_top_y = region.y;
    let vis_bottom_y = region.y + region.height;

    // Find visible level range using Y coordinates.
    // Note: higher prices have lower Y values (screen Y increases downward).
    // levels are sorted by price ascending, so Y decreases as index increases.
    let vis_start = resolved
        .levels
        .iter()
        .position(|l| {
            let y = view.price_units_to_y(l.price_units);
            y <= vis_bottom_y
        })
        .unwrap_or(0)
        .saturating_sub(1);
    let vis_end = resolved
        .levels
        .iter()
        .rposition(|l| {
            let y = view.price_units_to_y(l.price_units);
            y >= vis_top_y
        })
        .map(|i| (i + 1).min(resolved.levels.len()))
        .unwrap_or(resolved.levels.len());

    let visible_levels = &resolved.levels[vis_start..vis_end];
    let vis_value_area = resolved
        .value_area
        .map(|(vah, val)| (vah.saturating_sub(vis_start), val.saturating_sub(vis_start)));

    // -- Pass 1: VA fill rectangle --
    if config.va_config.show_value_area && config.va_config.show_va_fill {
        draw_va_fill(
            canvas,
            &resolved.levels,
            resolved.value_area,
            config,
            view,
            anchor_x,
            box_right,
        );
    }

    // -- Pass 1b: HVN zone fills --
    if config.node_config.show_hvn_zones && !output.hvn_zones.is_empty() {
        draw_zone_fills(
            canvas,
            &output.hvn_zones,
            config.node_config.hvn_zone_color,
            config.node_config.hvn_zone_opacity,
            view,
            anchor_x,
            box_right,
        );
    }

    // -- Pass 1c: LVN zone fills --
    if config.node_config.show_lvn_zones && !output.lvn_zones.is_empty() {
        draw_zone_fills(
            canvas,
            &output.lvn_zones,
            config.node_config.lvn_zone_color,
            config.node_config.lvn_zone_opacity,
            view,
            anchor_x,
            box_right,
        );
    }

    // -- Pass 2: Volume bars --
    match config.vbp_type {
        VbpType::Volume => {
            draw_volume(
                canvas,
                visible_levels,
                vis_value_area,
                config,
                view,
                max_bar_length,
                bar_height,
                anchor_x,
                &resolved.levels,
            );
        }
        VbpType::BidAskVolume => {
            draw_bid_ask(
                canvas,
                visible_levels,
                vis_value_area,
                config,
                view,
                max_bar_length,
                bar_height,
                anchor_x,
                &resolved.levels,
            );
        }
        VbpType::Delta => {
            draw_delta(
                canvas,
                visible_levels,
                vis_value_area,
                config,
                view,
                max_bar_length,
                bar_height,
                anchor_x,
                &resolved.levels,
            );
        }
        VbpType::DeltaAndTotalVolume => {
            draw_delta_and_total(
                canvas,
                visible_levels,
                vis_value_area,
                config,
                view,
                max_bar_length,
                bar_height,
                anchor_x,
                &resolved.levels,
            );
        }
        VbpType::DeltaPercentage => {
            draw_delta_pct(
                canvas,
                visible_levels,
                vis_value_area,
                config,
                view,
                max_bar_length,
                bar_height,
                anchor_x,
            );
        }
    }

    // -- Pass 3: VAH/VAL lines --
    if config.va_config.show_value_area {
        draw_va_lines(
            canvas,
            &resolved.levels,
            resolved.value_area,
            config,
            view,
            anchor_x,
            box_right,
        );
    }

    // -- Pass 4: Peak line (single) --
    if config.node_config.show_peak_line
        && let Some(ref node) = output.peak_node
    {
        let y = view.price_units_to_y(node.price_units);
        draw_horizontal_line(
            canvas,
            y,
            to_color(config.node_config.peak_color, 1.0),
            &config.node_config.peak_line_style,
            config.node_config.peak_line_width,
            &config.node_config.peak_extend,
            anchor_x,
            box_right,
            view,
        );
    }

    // -- Pass 4b: Valley line (single) --
    if config.node_config.show_valley_line
        && let Some(ref node) = output.valley_node
    {
        let y = view.price_units_to_y(node.price_units);
        draw_horizontal_line(
            canvas,
            y,
            to_color(config.node_config.valley_color, 1.0),
            &config.node_config.valley_line_style,
            config.node_config.valley_line_width,
            &config.node_config.valley_extend,
            anchor_x,
            box_right,
            view,
        );
    }

    // -- Pass 5: POC line --
    if config.poc_config.show_poc {
        draw_poc_enhanced(
            canvas,
            &resolved.levels,
            resolved.poc,
            config,
            view,
            anchor_x,
            box_right,
        );
    }

    // -- Pass 6: Developing POC --
    if config.poc_config.show_developing_poc && !output.developing_poc_points.is_empty() {
        draw_developing_poc(canvas, output, config, view);
    }

    // -- Pass 6b: Developing Peak --
    if config.node_config.show_developing_peak && !output.developing_peak_points.is_empty() {
        draw_developing_line(
            canvas,
            &output.developing_peak_points,
            config.node_config.developing_peak_color,
            config.node_config.developing_peak_line_width,
            &config.node_config.developing_peak_line_style,
            view,
        );
    }

    // -- Pass 6c: Developing Valley --
    if config.node_config.show_developing_valley && !output.developing_valley_points.is_empty() {
        draw_developing_line(
            canvas,
            &output.developing_valley_points,
            config.node_config.developing_valley_color,
            config.node_config.developing_valley_line_width,
            &config.node_config.developing_valley_line_style,
            view,
        );
    }

    // -- Pass 7: VWAP + bands --
    if config.vwap_config.show_vwap && !output.vwap_points.is_empty() {
        draw_vwap(canvas, output, config, view);
    }

    // -- Pass 8: Bounding rect --
    draw_bounding_rect(canvas, anchor_x, box_right, y_top, y_bottom);

    // -- Pass 9: Price labels --
    draw_price_labels(
        canvas,
        &resolved.levels,
        resolved.poc,
        resolved.value_area,
        output,
        config,
        view,
        anchor_x,
        box_right,
    );
}

// -- Shared helpers --

/// Draw a styled horizontal line with optional extension.
fn draw_horizontal_line(
    canvas: &mut dyn Canvas,
    y: f32,
    color: Rgba,
    line_style: &crate::config::LineStyleValue,
    line_width: f32,
    extend: &ExtendDirection,
    anchor_x: f32,
    box_right: f32,
    view: &dyn ChartView,
) {
    let (x_left, x_right) = extend_x_range(anchor_x, box_right, extend, view);
    let width = coord::effective_line_width(line_width, view.scaling());
    let style = LineStyle::from(line_style);
    canvas.stroke_line(x_left, y, x_right, y, color, width, style);
}

/// Compute the X range for a horizontal line with extension.
fn extend_x_range(
    anchor_x: f32,
    box_right: f32,
    extend: &ExtendDirection,
    view: &dyn ChartView,
) -> (f32, f32) {
    let region = view.visible_region();
    let vis_left = region.x;
    let vis_right = region.x + region.width;
    match extend {
        ExtendDirection::None => (anchor_x, box_right),
        ExtendDirection::Left => (vis_left, box_right),
        ExtendDirection::Right => (anchor_x, vis_right),
        ExtendDirection::Both => (vis_left, vis_right),
    }
}

/// Draw a bar growing rightward from anchor_x.
fn draw_bar_right(
    canvas: &mut dyn Canvas,
    anchor_x: f32,
    y: f32,
    bar_h: f32,
    bar_len: f32,
    color: Rgba,
) {
    let top = y - bar_h / 2.0;
    canvas.fill_rect(anchor_x, top, bar_len, bar_h, color);
}

/// Draw a bar growing leftward from anchor_x.
fn draw_bar_left(
    canvas: &mut dyn Canvas,
    anchor_x: f32,
    y: f32,
    bar_h: f32,
    bar_len: f32,
    color: Rgba,
) {
    let top = y - bar_h / 2.0;
    canvas.fill_rect(anchor_x - bar_len, top, bar_len, bar_h, color);
}

/// Draw a text label at a given position.
fn draw_label(canvas: &mut dyn Canvas, text_content: &str, x: f32, y: f32, color: Rgba) {
    canvas.fill_text(
        x,
        y - VBP_LABEL_FONT_SIZE / 2.0,
        text_content,
        VBP_LABEL_FONT_SIZE,
        color,
        FontHint::Default,
    );
}

/// Draw a polyline from (timestamp_ms, price_f32) points.
fn draw_polyline(
    canvas: &mut dyn Canvas,
    points: &[(u64, f32)],
    color: Rgba,
    width: f32,
    style: LineStyle,
    view: &dyn ChartView,
) {
    if points.len() < 2 {
        return;
    }

    let screen_points: Vec<(f32, f32)> = points
        .iter()
        .map(|&(ts, price)| (view.interval_to_x(ts), view.value_to_y(price)))
        .collect();

    canvas.stroke_polyline(&screen_points, color, width, style);
}

/// Get the Y coordinate of the top of the profile.
fn price_to_y_top(levels: &[ProfileLevel], view: &dyn ChartView, bar_height: f32) -> f32 {
    let Some(last) = levels.last() else {
        return 0.0;
    };
    view.price_units_to_y(last.price_units) - bar_height / 2.0
}

/// Get the Y coordinate of the bottom of the profile.
fn price_to_y_bottom(levels: &[ProfileLevel], view: &dyn ChartView, bar_height: f32) -> f32 {
    let Some(first) = levels.first() else {
        return 0.0;
    };
    view.price_units_to_y(first.price_units) + bar_height / 2.0
}
