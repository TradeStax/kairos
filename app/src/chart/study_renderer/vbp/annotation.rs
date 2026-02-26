//! VBP annotation rendering: VA fills/lines, POC, zones, developing
//! lines, VWAP, bounding rect, and price labels.

use super::{draw_horizontal_line, draw_label, draw_polyline, extend_x_range, to_iced_color};
use crate::chart::ViewState;
use crate::chart::study_renderer::coord;
use data::Price;
use iced::widget::canvas::{Frame, Path, Stroke};
use iced::{Color, Point, Size};
use study::config::LineStyleValue;
use study::output::{ProfileLevel, ProfileOutput, ProfileRenderConfig};

// ── VA fill rectangle ───────────────────────────────────────────────

pub(super) fn draw_va_fill(
    frame: &mut Frame,
    levels: &[ProfileLevel],
    value_area: Option<(usize, usize)>,
    config: &ProfileRenderConfig,
    state: &ViewState,
    anchor_x: f32,
    box_right: f32,
    bounds: Size,
) {
    let Some((vah_idx, val_idx)) = value_area else {
        return;
    };
    let Some(vah_level) = levels.get(vah_idx) else {
        return;
    };
    let Some(val_level) = levels.get(val_idx) else {
        return;
    };

    let y_vah = state.price_to_y(Price::from_units(vah_level.price_units));
    let y_val = state.price_to_y(Price::from_units(val_level.price_units));
    let y_top = y_vah.min(y_val);
    let y_height = (y_vah - y_val).abs().max(1.0);

    let (x_left, x_right) = extend_x_range(
        anchor_x,
        box_right,
        &config.va_config.va_extend,
        state,
        bounds,
    );

    let fill_color = to_iced_color(
        config.va_config.va_fill_color,
        config.va_config.va_fill_opacity,
    );
    frame.fill_rectangle(
        Point::new(x_left, y_top),
        Size::new(x_right - x_left, y_height),
        fill_color,
    );
}

// ── VAH/VAL lines ───────────────────────────────────────────────────

pub(super) fn draw_va_lines(
    frame: &mut Frame,
    levels: &[ProfileLevel],
    value_area: Option<(usize, usize)>,
    config: &ProfileRenderConfig,
    state: &ViewState,
    anchor_x: f32,
    box_right: f32,
    bounds: Size,
) {
    let Some((vah_idx, val_idx)) = value_area else {
        return;
    };

    // VAH line
    if let Some(level) = levels.get(vah_idx) {
        let y = state.price_to_y(Price::from_units(level.price_units));
        draw_horizontal_line(
            frame,
            y,
            to_iced_color(config.va_config.vah_color, 1.0),
            &config.va_config.vah_line_style,
            config.va_config.vah_line_width,
            &config.va_config.va_extend,
            anchor_x,
            box_right,
            bounds,
            state,
        );
    }

    // VAL line
    if let Some(level) = levels.get(val_idx) {
        let y = state.price_to_y(Price::from_units(level.price_units));
        draw_horizontal_line(
            frame,
            y,
            to_iced_color(config.va_config.val_color, 1.0),
            &config.va_config.val_line_style,
            config.va_config.val_line_width,
            &config.va_config.va_extend,
            anchor_x,
            box_right,
            bounds,
            state,
        );
    }
}

// ── Zone fills ──────────────────────────────────────────────────────

pub(super) fn draw_zone_fills(
    frame: &mut Frame,
    zones: &[(i64, i64)],
    color: data::SerializableColor,
    opacity: f32,
    state: &ViewState,
    anchor_x: f32,
    box_right: f32,
) {
    let fill_color = to_iced_color(color, opacity);
    for &(lo, hi) in zones {
        let y_lo = state.price_to_y(Price::from_units(lo));
        let y_hi = state.price_to_y(Price::from_units(hi));
        let y_top = y_hi.min(y_lo);
        let y_height = (y_hi - y_lo).abs().max(1.0);
        frame.fill_rectangle(
            Point::new(anchor_x, y_top),
            Size::new(box_right - anchor_x, y_height),
            fill_color,
        );
    }
}

// ── Developing line (shared helper) ─────────────────────────────────

pub(super) fn draw_developing_line(
    frame: &mut Frame,
    points: &[(u64, i64)],
    color: data::SerializableColor,
    line_width: f32,
    line_style: &LineStyleValue,
    state: &ViewState,
) {
    if points.len() < 2 {
        return;
    }

    let color = to_iced_color(color, 1.0);
    let width = coord::effective_line_width(line_width, state.scaling);
    let dash = coord::line_dash_for_style(line_style);

    let path = Path::new(|builder| {
        let x0 = state.interval_to_x(points[0].0);
        let y0 = state.price_to_y(Price::from_units(points[0].1));
        builder.move_to(Point::new(x0, y0));

        for &(ts, price_units) in &points[1..] {
            let x = state.interval_to_x(ts);
            let y = state.price_to_y(Price::from_units(price_units));
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

// ── Enhanced POC ────────────────────────────────────────────────────

pub(super) fn draw_poc_enhanced(
    frame: &mut Frame,
    levels: &[ProfileLevel],
    poc: Option<usize>,
    config: &ProfileRenderConfig,
    state: &ViewState,
    anchor_x: f32,
    box_right: f32,
    bounds: Size,
) {
    if let Some(poc_idx) = poc
        && let Some(level) = levels.get(poc_idx)
    {
        let y = state.price_to_y(Price::from_units(level.price_units));
        let color = to_iced_color(config.poc_config.poc_color, 1.0);
        draw_horizontal_line(
            frame,
            y,
            color,
            &config.poc_config.poc_line_style,
            config.poc_config.poc_line_width,
            &config.poc_config.poc_extend,
            anchor_x,
            box_right,
            bounds,
            state,
        );
    }
}

// ── Developing POC polyline ─────────────────────────────────────────

pub(super) fn draw_developing_poc(
    frame: &mut Frame,
    output: &ProfileOutput,
    config: &ProfileRenderConfig,
    state: &ViewState,
) {
    let points = &output.developing_poc_points;
    if points.len() < 2 {
        return;
    }

    let color = to_iced_color(config.poc_config.developing_poc_color, 1.0);
    let width =
        coord::effective_line_width(config.poc_config.developing_poc_line_width, state.scaling);
    let dash = coord::line_dash_for_style(&config.poc_config.developing_poc_line_style);

    let path = Path::new(|builder| {
        let x0 = state.interval_to_x(points[0].0);
        let y0 = state.price_to_y(Price::from_units(points[0].1));
        builder.move_to(Point::new(x0, y0));

        for &(ts, price_units) in &points[1..] {
            let x = state.interval_to_x(ts);
            let y = state.price_to_y(Price::from_units(price_units));
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

// ── VWAP line + bands ───────────────────────────────────────────────

pub(super) fn draw_vwap(
    frame: &mut Frame,
    output: &ProfileOutput,
    config: &ProfileRenderConfig,
    state: &ViewState,
) {
    let cfg = &config.vwap_config;

    // Draw bands first (behind VWAP line)
    if cfg.show_bands
        && !output.vwap_upper_points.is_empty()
        && !output.vwap_lower_points.is_empty()
    {
        let band_color = to_iced_color(cfg.band_color, 1.0);
        let band_width = coord::effective_line_width(cfg.band_line_width, state.scaling);
        let band_dash = coord::line_dash_for_style(&cfg.band_line_style);

        draw_polyline(
            frame,
            &output.vwap_upper_points,
            band_color,
            band_width,
            band_dash,
            state,
        );
        draw_polyline(
            frame,
            &output.vwap_lower_points,
            band_color,
            band_width,
            band_dash,
            state,
        );
    }

    // VWAP line
    let vwap_color = to_iced_color(cfg.vwap_color, 1.0);
    let vwap_width = coord::effective_line_width(cfg.vwap_line_width, state.scaling);
    let vwap_dash = coord::line_dash_for_style(&cfg.vwap_line_style);

    draw_polyline(
        frame,
        &output.vwap_points,
        vwap_color,
        vwap_width,
        vwap_dash,
        state,
    );
}

// ── Bounding rect ───────────────────────────────────────────────────

pub(super) fn draw_bounding_rect(
    frame: &mut Frame,
    anchor_x: f32,
    box_right: f32,
    y_top: f32,
    y_bottom: f32,
) {
    let left = anchor_x;
    let width = box_right - anchor_x;
    let top = y_top.min(y_bottom);
    let height = (y_bottom - y_top).abs();

    if height > 0.0 && width > 0.0 {
        let rect_path = Path::rectangle(Point::new(left, top), Size::new(width, height));
        frame.stroke(
            &rect_path,
            Stroke::default()
                .with_color(Color {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 0.15,
                })
                .with_width(1.0),
        );
    }
}

// ── Price labels ────────────────────────────────────────────────────

pub(super) fn draw_price_labels(
    frame: &mut Frame,
    levels: &[ProfileLevel],
    poc: Option<usize>,
    value_area: Option<(usize, usize)>,
    output: &ProfileOutput,
    config: &ProfileRenderConfig,
    state: &ViewState,
    _anchor_x: f32,
    box_right: f32,
    _bounds: Size,
) {
    let label_x = box_right + 4.0;

    // POC label
    if config.poc_config.show_poc
        && config.poc_config.show_poc_label
        && let Some(idx) = poc
        && let Some(level) = levels.get(idx)
    {
        let y = state.price_to_y(Price::from_units(level.price_units));
        let color = to_iced_color(config.poc_config.poc_color, 1.0);
        draw_label(frame, &format!("POC {:.2}", level.price), label_x, y, color);
    }

    // VA labels
    if config.va_config.show_value_area
        && config.va_config.show_va_labels
        && let Some((vah_idx, val_idx)) = value_area
    {
        if let Some(level) = levels.get(vah_idx) {
            let y = state.price_to_y(Price::from_units(level.price_units));
            let color = to_iced_color(config.va_config.vah_color, 1.0);
            draw_label(frame, &format!("VAH {:.2}", level.price), label_x, y, color);
        }
        if let Some(level) = levels.get(val_idx) {
            let y = state.price_to_y(Price::from_units(level.price_units));
            let color = to_iced_color(config.va_config.val_color, 1.0);
            draw_label(frame, &format!("VAL {:.2}", level.price), label_x, y, color);
        }
    }

    // Peak label
    if config.node_config.show_peak_line
        && config.node_config.show_peak_label
        && let Some(ref node) = output.peak_node
    {
        let y = state.price_to_y(Price::from_units(node.price_units));
        let color = to_iced_color(config.node_config.peak_color, 1.0);
        draw_label(frame, &format!("Peak {:.2}", node.price), label_x, y, color);
    }

    // Valley label
    if config.node_config.show_valley_line
        && config.node_config.show_valley_label
        && let Some(ref node) = output.valley_node
    {
        let y = state.price_to_y(Price::from_units(node.price_units));
        let color = to_iced_color(config.node_config.valley_color, 1.0);
        draw_label(
            frame,
            &format!("Valley {:.2}", node.price),
            label_x,
            y,
            color,
        );
    }

    // VWAP label
    if config.vwap_config.show_vwap
        && config.vwap_config.show_vwap_label
        && let Some(last) = output.vwap_points.last()
    {
        let y = state.price_to_y(Price::from_f32(last.1));
        let color = to_iced_color(config.vwap_config.vwap_color, 1.0);
        draw_label(frame, &format!("VWAP {:.2}", last.1), label_x, y, color);
    }
}
