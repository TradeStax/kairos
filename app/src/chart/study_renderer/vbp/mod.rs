//! VBP (Volume-by-Price) renderer
//!
//! Renders horizontal volume bars anchored to the candle time range.
//! Multi-pass rendering with clear draw order:
//!
//! 1.  VA fill rectangle (behind everything)
//! 1b. HVN zone fills
//! 1c. LVN zone fills
//! 2.  Volume bars (5 modes, with VA dimming)
//! 3.  VAH/VAL lines
//! 4.  Peak line (single dominant HVN)
//! 4b. Valley line (single deepest LVN)
//! 5.  POC line (enhanced with style/extension)
//! 6.  Developing POC polyline
//! 6b. Developing Peak polyline
//! 6c. Developing Valley polyline
//! 7.  VWAP line + bands
//! 8.  Bounding rect outline
//! 9.  Price labels (POC, VA, Peak, Valley, VWAP)

mod annotation;
mod bar;

use crate::chart::study_renderer::coord;
use crate::chart::ViewState;
use exchange::util::Price;
use iced::widget::canvas::{Frame, LineDash, Path, Stroke, Text};
use iced::{Color, Point, Size};
use study::config::LineStyleValue;
use study::orderflow::vbp::profile_core;
use study::output::{
    ExtendDirection, ProfileLevel, ProfileOutput,
    ProfileRenderConfig, VbpGroupingMode, VbpResolvedCache,
    VbpType,
};

use annotation::{
    draw_bounding_rect, draw_developing_line, draw_developing_poc,
    draw_poc_enhanced, draw_price_labels, draw_va_fill,
    draw_va_lines, draw_vwap, draw_zone_fills,
};
use bar::{
    draw_bid_ask, draw_delta, draw_delta_and_total, draw_delta_pct,
    draw_volume,
};

/// Minimum row height in screen pixels for readable bars.
const MIN_ROW_PX: f32 = 4.0;

/// Font size for price labels.
const LABEL_FONT_SIZE: f32 = 10.0;

/// Compute the dynamic grouping quantum for automatic mode.
fn compute_dynamic_quantum(
    state: &ViewState,
    factor: i64,
    tick_units: i64,
) -> i64 {
    coord::compute_dynamic_quantum(
        state, MIN_ROW_PX, factor, tick_units,
    )
}

/// Merge profile levels to a coarser quantum boundary.
fn merge_levels_to_quantum(
    levels: &[ProfileLevel],
    target_quantum: i64,
) -> Vec<ProfileLevel> {
    if levels.is_empty() {
        return Vec::new();
    }

    let mut merged = Vec::with_capacity(levels.len() / 2 + 1);
    let mut cur_bucket =
        (levels[0].price_units / target_quantum) * target_quantum;
    let mut buy_acc: f64 = 0.0;
    let mut sell_acc: f64 = 0.0;

    for level in levels {
        let bucket =
            (level.price_units / target_quantum) * target_quantum;
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
fn ensure_resolved_cache(
    output: &ProfileOutput,
    config: &ProfileRenderConfig,
    state: &ViewState,
) {
    let tick_units = state.tick_size.units.max(1);

    let target_quantum = match output.grouping_mode {
        VbpGroupingMode::Automatic { factor } => {
            let dq = compute_dynamic_quantum(
                state, factor, tick_units,
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

    let (levels, poc, value_area) =
        if target_quantum > output.quantum {
            let merged = merge_levels_to_quantum(
                &output.levels,
                target_quantum,
            );
            let poc = profile_core::find_poc_index(&merged);
            let value_area =
                if config.va_config.show_value_area {
                    poc.and_then(|idx| {
                        profile_core::calculate_value_area(
                            &merged,
                            idx,
                            config.va_config.value_area_pct
                                as f64,
                        )
                    })
                } else {
                    None
                };
            (merged, poc, value_area)
        } else {
            (
                output.levels.clone(),
                output.poc,
                output.value_area,
            )
        };

    *output
        .resolved_cache
        .lock()
        .unwrap_or_else(|e| e.into_inner()) =
        Some(VbpResolvedCache {
            quantum: target_quantum,
            levels,
            poc,
            value_area,
        });
}

/// Compute the x-range from a profile's time_range.
pub fn profile_x_range(
    output: &ProfileOutput,
    state: &ViewState,
    bounds: Size,
) -> (f32, f32) {
    match output.time_range {
        Some((start_ms, end_ms)) => {
            let x0 = state.interval_to_x(start_ms);
            let x1 = state.interval_to_x(end_ms);
            (x0.min(x1), x0.max(x1))
        }
        None => {
            let fw = bounds.width / state.scaling;
            (-fw / 2.0, fw / 2.0)
        }
    }
}

/// Render multiple VBP profile segments onto the chart canvas.
///
/// Skips profiles whose time range falls entirely off-screen.
pub fn render_vbp_multi(
    frame: &mut Frame,
    profiles: &[ProfileOutput],
    config: &ProfileRenderConfig,
    state: &ViewState,
    bounds: Size,
) {
    let region = state.visible_region(bounds);
    let vis_left = region.x;
    let vis_right = region.x + region.width;

    for profile in profiles {
        let (ax, br) = profile_x_range(profile, state, bounds);
        // Skip profiles entirely outside the visible region
        if br < vis_left || ax > vis_right {
            continue;
        }
        render_vbp(
            frame, profile, config, state, bounds, ax, br,
        );
    }
}

/// Render a single VBP profile output onto the chart canvas.
///
/// `anchor_x` and `box_right` define the horizontal extent of
/// this profile segment in chart-space coordinates. Use
/// [`profile_x_range`] to compute these from the profile's
/// `time_range`.
pub fn render_vbp(
    frame: &mut Frame,
    output: &ProfileOutput,
    config: &ProfileRenderConfig,
    state: &ViewState,
    bounds: Size,
    anchor_x: f32,
    box_right: f32,
) {
    if output.levels.is_empty() {
        return;
    }

    ensure_resolved_cache(output, config, state);
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
        let y0 = state.price_to_y(Price::from_units(
            resolved.levels[0].price_units,
        ));
        let y1 = state.price_to_y(Price::from_units(
            resolved.levels[1].price_units,
        ));
        (y1 - y0).abs().max(1.0)
    } else {
        state.cell_height.max(1.0)
    };

    // Compute Y bounds for the profile
    let y_top =
        price_to_y_top(&resolved.levels, state, bar_height);
    let y_bottom =
        price_to_y_bottom(&resolved.levels, state, bar_height);

    // Cull off-screen levels
    let region = state.visible_region(bounds);
    let (vis_high, vis_low) = state.price_range(&region);
    let vis_high_units = vis_high.units();
    let vis_low_units = vis_low.units();

    let vis_start = resolved
        .levels
        .partition_point(|l| l.price_units < vis_low_units)
        .saturating_sub(1);
    let vis_end = resolved
        .levels
        .partition_point(|l| l.price_units <= vis_high_units)
        .min(resolved.levels.len());

    let visible_levels = &resolved.levels[vis_start..vis_end];
    let vis_value_area = resolved.value_area.map(|(vah, val)| {
        (vah.saturating_sub(vis_start), val.saturating_sub(vis_start))
    });

    // ── Pass 1: VA fill rectangle ────────────────────────────────
    if config.va_config.show_value_area
        && config.va_config.show_va_fill
    {
        draw_va_fill(
            frame,
            &resolved.levels,
            resolved.value_area,
            config,
            state,
            anchor_x,
            box_right,
            bounds,
        );
    }

    // ── Pass 1b: HVN zone fills ─────────────────────────────────
    if config.node_config.show_hvn_zones
        && !output.hvn_zones.is_empty()
    {
        draw_zone_fills(
            frame,
            &output.hvn_zones,
            config.node_config.hvn_zone_color,
            config.node_config.hvn_zone_opacity,
            state,
            anchor_x,
            box_right,
        );
    }

    // ── Pass 1c: LVN zone fills ─────────────────────────────────
    if config.node_config.show_lvn_zones
        && !output.lvn_zones.is_empty()
    {
        draw_zone_fills(
            frame,
            &output.lvn_zones,
            config.node_config.lvn_zone_color,
            config.node_config.lvn_zone_opacity,
            state,
            anchor_x,
            box_right,
        );
    }

    // ── Pass 2: Volume bars ──────────────────────────────────────
    match config.vbp_type {
        VbpType::Volume => {
            draw_volume(
                frame,
                visible_levels,
                vis_value_area,
                config,
                state,
                max_bar_length,
                bar_height,
                anchor_x,
                &resolved.levels,
            );
        }
        VbpType::BidAskVolume => {
            draw_bid_ask(
                frame,
                visible_levels,
                vis_value_area,
                config,
                state,
                max_bar_length,
                bar_height,
                anchor_x,
                &resolved.levels,
            );
        }
        VbpType::Delta => {
            draw_delta(
                frame,
                visible_levels,
                vis_value_area,
                config,
                state,
                max_bar_length,
                bar_height,
                anchor_x,
                &resolved.levels,
            );
        }
        VbpType::DeltaAndTotalVolume => {
            draw_delta_and_total(
                frame,
                visible_levels,
                vis_value_area,
                config,
                state,
                max_bar_length,
                bar_height,
                anchor_x,
                &resolved.levels,
            );
        }
        VbpType::DeltaPercentage => {
            draw_delta_pct(
                frame,
                visible_levels,
                vis_value_area,
                config,
                state,
                max_bar_length,
                bar_height,
                anchor_x,
            );
        }
    }

    // ── Pass 3: VAH/VAL lines ────────────────────────────────────
    if config.va_config.show_value_area {
        draw_va_lines(
            frame,
            &resolved.levels,
            resolved.value_area,
            config,
            state,
            anchor_x,
            box_right,
            bounds,
        );
    }

    // ── Pass 4: Peak line (single) ─────────────────────────────
    if config.node_config.show_peak_line {
        if let Some(ref node) = output.peak_node {
            let y = state.price_to_y(
                Price::from_units(node.price_units),
            );
            draw_horizontal_line(
                frame,
                y,
                to_iced_color(
                    config.node_config.peak_color,
                    1.0,
                ),
                &config.node_config.peak_line_style,
                config.node_config.peak_line_width,
                &config.node_config.peak_extend,
                anchor_x,
                box_right,
                bounds,
                state,
            );
        }
    }

    // ── Pass 4b: Valley line (single) ────────────────────────────
    if config.node_config.show_valley_line {
        if let Some(ref node) = output.valley_node {
            let y = state.price_to_y(
                Price::from_units(node.price_units),
            );
            draw_horizontal_line(
                frame,
                y,
                to_iced_color(
                    config.node_config.valley_color,
                    1.0,
                ),
                &config.node_config.valley_line_style,
                config.node_config.valley_line_width,
                &config.node_config.valley_extend,
                anchor_x,
                box_right,
                bounds,
                state,
            );
        }
    }

    // ── Pass 5: POC line ─────────────────────────────────────────
    if config.poc_config.show_poc {
        draw_poc_enhanced(
            frame,
            &resolved.levels,
            resolved.poc,
            config,
            state,
            anchor_x,
            box_right,
            bounds,
        );
    }

    // ── Pass 6: Developing POC ───────────────────────────────────
    if config.poc_config.show_developing_poc
        && !output.developing_poc_points.is_empty()
    {
        draw_developing_poc(
            frame, output, config, state,
        );
    }

    // ── Pass 6b: Developing Peak ─────────────────────────────────
    if config.node_config.show_developing_peak
        && !output.developing_peak_points.is_empty()
    {
        draw_developing_line(
            frame,
            &output.developing_peak_points,
            config.node_config.developing_peak_color,
            config.node_config.developing_peak_line_width,
            &config
                .node_config
                .developing_peak_line_style,
            state,
        );
    }

    // ── Pass 6c: Developing Valley ───────────────────────────────
    if config.node_config.show_developing_valley
        && !output.developing_valley_points.is_empty()
    {
        draw_developing_line(
            frame,
            &output.developing_valley_points,
            config
                .node_config
                .developing_valley_color,
            config
                .node_config
                .developing_valley_line_width,
            &config
                .node_config
                .developing_valley_line_style,
            state,
        );
    }

    // ── Pass 7: VWAP + bands ─────────────────────────────────────
    if config.vwap_config.show_vwap
        && !output.vwap_points.is_empty()
    {
        draw_vwap(frame, output, config, state);
    }

    // ── Pass 8: Bounding rect ────────────────────────────────────
    draw_bounding_rect(
        frame, anchor_x, box_right, y_top, y_bottom,
    );

    // ── Pass 9: Price labels ─────────────────────────────────────
    draw_price_labels(
        frame,
        &resolved.levels,
        resolved.poc,
        resolved.value_area,
        output,
        config,
        state,
        anchor_x,
        box_right,
        bounds,
    );
}

// ── Shared helpers ──────────────────────────────────────────────────

/// Draw a styled horizontal line with optional extension.
fn draw_horizontal_line(
    frame: &mut Frame,
    y: f32,
    color: Color,
    line_style: &LineStyleValue,
    line_width: f32,
    extend: &ExtendDirection,
    anchor_x: f32,
    box_right: f32,
    bounds: Size,
    state: &ViewState,
) {
    let (x_left, x_right) =
        extend_x_range(anchor_x, box_right, extend, state, bounds);
    let width =
        coord::effective_line_width(line_width, state.scaling);
    let dash = coord::line_dash_for_style(line_style);

    let line = Path::line(
        Point::new(x_left, y),
        Point::new(x_right, y),
    );
    frame.stroke(
        &line,
        Stroke::with_color(
            Stroke {
                width,
                line_dash: dash,
                ..Stroke::default()
            },
            color,
        ),
    );
}

/// Compute the X range for a horizontal line with extension.
///
/// Uses chart-space visible region for extension boundaries.
fn extend_x_range(
    anchor_x: f32,
    box_right: f32,
    extend: &ExtendDirection,
    state: &ViewState,
    bounds: Size,
) -> (f32, f32) {
    let region = state.visible_region(bounds);
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
    frame: &mut Frame,
    anchor_x: f32,
    y: f32,
    bar_h: f32,
    bar_len: f32,
    color: Color,
) {
    let top = y - bar_h / 2.0;
    frame.fill_rectangle(
        Point::new(anchor_x, top),
        Size::new(bar_len, bar_h),
        color,
    );
}

/// Draw a bar growing leftward from anchor_x.
fn draw_bar_left(
    frame: &mut Frame,
    anchor_x: f32,
    y: f32,
    bar_h: f32,
    bar_len: f32,
    color: Color,
) {
    let top = y - bar_h / 2.0;
    frame.fill_rectangle(
        Point::new(anchor_x - bar_len, top),
        Size::new(bar_len, bar_h),
        color,
    );
}

/// Draw a text label at a given position.
fn draw_label(
    frame: &mut Frame,
    text_content: &str,
    x: f32,
    y: f32,
    color: Color,
) {
    frame.fill_text(Text {
        content: text_content.to_string(),
        position: Point::new(x, y - LABEL_FONT_SIZE / 2.0),
        color,
        size: iced::Pixels(LABEL_FONT_SIZE),
        ..Text::default()
    });
}

/// Draw a polyline from (timestamp_ms, price_f32) points.
fn draw_polyline(
    frame: &mut Frame,
    points: &[(u64, f32)],
    color: Color,
    width: f32,
    dash: LineDash<'_>,
    state: &ViewState,
) {
    if points.len() < 2 {
        return;
    }

    let path = Path::new(|builder| {
        let x0 = state.interval_to_x(points[0].0);
        let y0 = state.price_to_y(Price::from_f32(points[0].1));
        builder.move_to(Point::new(x0, y0));

        for &(ts, price) in &points[1..] {
            let x = state.interval_to_x(ts);
            let y = state.price_to_y(Price::from_f32(price));
            builder.line_to(Point::new(x, y));
        }
    });

    frame.stroke(
        &path,
        Stroke::with_color(
            Stroke {
                width,
                line_dash: dash,
                ..Stroke::default()
            },
            color,
        ),
    );
}

/// Get the Y coordinate of the top of the profile.
fn price_to_y_top(
    levels: &[ProfileLevel],
    state: &ViewState,
    bar_height: f32,
) -> f32 {
    let Some(last) = levels.last() else {
        return 0.0;
    };
    state.price_to_y(Price::from_units(last.price_units))
        - bar_height / 2.0
}

/// Get the Y coordinate of the bottom of the profile.
fn price_to_y_bottom(
    levels: &[ProfileLevel],
    state: &ViewState,
    bar_height: f32,
) -> f32 {
    let Some(first) = levels.first() else {
        return 0.0;
    };
    state.price_to_y(Price::from_units(first.price_units))
        + bar_height / 2.0
}

fn to_iced_color(
    sc: data::SerializableColor,
    opacity: f32,
) -> Color {
    coord::to_iced_color(sc, opacity)
}
