//! VBP annotation rendering: VA fills/lines, POC, zones, developing
//! lines, VWAP, bounding rect, and price labels.

use super::{
    draw_horizontal_line, draw_label, draw_polyline, extend_x_range, to_color,
};
use crate::output::render::canvas::Canvas;
use crate::output::render::chart_view::ChartView;
use crate::output::render::constants::VBP_BOUNDING_RECT_COLOR;
use crate::output::render::coord;
use crate::output::render::types::LineStyle;
use crate::output::{ProfileLevel, ProfileOutput, ProfileRenderConfig};

// -- VA fill rectangle --

pub(super) fn draw_va_fill(
    canvas: &mut dyn Canvas,
    levels: &[ProfileLevel],
    value_area: Option<(usize, usize)>,
    config: &ProfileRenderConfig,
    view: &dyn ChartView,
    anchor_x: f32,
    box_right: f32,
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

    let y_vah = view.price_units_to_y(vah_level.price_units);
    let y_val = view.price_units_to_y(val_level.price_units);
    let y_top = y_vah.min(y_val);
    let y_height = (y_vah - y_val).abs().max(1.0);

    let (x_left, x_right) =
        extend_x_range(anchor_x, box_right, &config.va_config.va_extend, view);

    let fill_color = to_color(
        config.va_config.va_fill_color,
        config.va_config.va_fill_opacity,
    );
    canvas.fill_rect(x_left, y_top, x_right - x_left, y_height, fill_color);
}

// -- VAH/VAL lines --

pub(super) fn draw_va_lines(
    canvas: &mut dyn Canvas,
    levels: &[ProfileLevel],
    value_area: Option<(usize, usize)>,
    config: &ProfileRenderConfig,
    view: &dyn ChartView,
    anchor_x: f32,
    box_right: f32,
) {
    let Some((vah_idx, val_idx)) = value_area else {
        return;
    };

    // VAH line
    if let Some(level) = levels.get(vah_idx) {
        let y = view.price_units_to_y(level.price_units);
        draw_horizontal_line(
            canvas,
            y,
            to_color(config.va_config.vah_color, 1.0),
            &config.va_config.vah_line_style,
            config.va_config.vah_line_width,
            &config.va_config.va_extend,
            anchor_x,
            box_right,
            view,
        );
    }

    // VAL line
    if let Some(level) = levels.get(val_idx) {
        let y = view.price_units_to_y(level.price_units);
        draw_horizontal_line(
            canvas,
            y,
            to_color(config.va_config.val_color, 1.0),
            &config.va_config.val_line_style,
            config.va_config.val_line_width,
            &config.va_config.va_extend,
            anchor_x,
            box_right,
            view,
        );
    }
}

// -- Zone fills --

pub(super) fn draw_zone_fills(
    canvas: &mut dyn Canvas,
    zones: &[(i64, i64)],
    color: data::SerializableColor,
    opacity: f32,
    view: &dyn ChartView,
    anchor_x: f32,
    box_right: f32,
) {
    let fill_color = to_color(color, opacity);
    for &(lo, hi) in zones {
        let y_lo = view.price_units_to_y(lo);
        let y_hi = view.price_units_to_y(hi);
        let y_top = y_hi.min(y_lo);
        let y_height = (y_hi - y_lo).abs().max(1.0);
        canvas.fill_rect(
            anchor_x,
            y_top,
            box_right - anchor_x,
            y_height,
            fill_color,
        );
    }
}

// -- Developing line (shared helper) --

pub(super) fn draw_developing_line(
    canvas: &mut dyn Canvas,
    points: &[(u64, i64)],
    color: data::SerializableColor,
    line_width: f32,
    line_style: &crate::config::LineStyleValue,
    view: &dyn ChartView,
) {
    if points.len() < 2 {
        return;
    }

    let color = to_color(color, 1.0);
    let width = coord::effective_line_width(line_width, view.scaling());
    let style = LineStyle::from(line_style);

    let screen_points: Vec<(f32, f32)> = points
        .iter()
        .map(|&(ts, price_units)| {
            (view.interval_to_x(ts), view.price_units_to_y(price_units))
        })
        .collect();

    canvas.stroke_polyline(&screen_points, color, width, style);
}

// -- Enhanced POC --

pub(super) fn draw_poc_enhanced(
    canvas: &mut dyn Canvas,
    levels: &[ProfileLevel],
    poc: Option<usize>,
    config: &ProfileRenderConfig,
    view: &dyn ChartView,
    anchor_x: f32,
    box_right: f32,
) {
    if let Some(poc_idx) = poc
        && let Some(level) = levels.get(poc_idx)
    {
        let y = view.price_units_to_y(level.price_units);
        let color = to_color(config.poc_config.poc_color, 1.0);
        draw_horizontal_line(
            canvas,
            y,
            color,
            &config.poc_config.poc_line_style,
            config.poc_config.poc_line_width,
            &config.poc_config.poc_extend,
            anchor_x,
            box_right,
            view,
        );
    }
}

// -- Developing POC polyline --

pub(super) fn draw_developing_poc(
    canvas: &mut dyn Canvas,
    output: &ProfileOutput,
    config: &ProfileRenderConfig,
    view: &dyn ChartView,
) {
    let points = &output.developing_poc_points;
    if points.len() < 2 {
        return;
    }

    let color = to_color(config.poc_config.developing_poc_color, 1.0);
    let width = coord::effective_line_width(
        config.poc_config.developing_poc_line_width,
        view.scaling(),
    );
    let style = LineStyle::from(&config.poc_config.developing_poc_line_style);

    let screen_points: Vec<(f32, f32)> = points
        .iter()
        .map(|&(ts, price_units)| {
            (view.interval_to_x(ts), view.price_units_to_y(price_units))
        })
        .collect();

    canvas.stroke_polyline(&screen_points, color, width, style);
}

// -- VWAP line + bands --

pub(super) fn draw_vwap(
    canvas: &mut dyn Canvas,
    output: &ProfileOutput,
    config: &ProfileRenderConfig,
    view: &dyn ChartView,
) {
    let cfg = &config.vwap_config;

    // Draw bands first (behind VWAP line)
    if cfg.show_bands
        && !output.vwap_upper_points.is_empty()
        && !output.vwap_lower_points.is_empty()
    {
        let band_color = to_color(cfg.band_color, 1.0);
        let band_width =
            coord::effective_line_width(cfg.band_line_width, view.scaling());
        let band_style = LineStyle::from(&cfg.band_line_style);

        draw_polyline(
            canvas,
            &output.vwap_upper_points,
            band_color,
            band_width,
            band_style,
            view,
        );
        draw_polyline(
            canvas,
            &output.vwap_lower_points,
            band_color,
            band_width,
            band_style,
            view,
        );
    }

    // VWAP line
    let vwap_color = to_color(cfg.vwap_color, 1.0);
    let vwap_width =
        coord::effective_line_width(cfg.vwap_line_width, view.scaling());
    let vwap_style = LineStyle::from(&cfg.vwap_line_style);

    draw_polyline(
        canvas,
        &output.vwap_points,
        vwap_color,
        vwap_width,
        vwap_style,
        view,
    );
}

// -- Bounding rect --

pub(super) fn draw_bounding_rect(
    canvas: &mut dyn Canvas,
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
        canvas.stroke_rect(
            left,
            top,
            width,
            height,
            VBP_BOUNDING_RECT_COLOR,
            1.0,
        );
    }
}

// -- Price labels --

pub(super) fn draw_price_labels(
    canvas: &mut dyn Canvas,
    levels: &[ProfileLevel],
    poc: Option<usize>,
    value_area: Option<(usize, usize)>,
    output: &ProfileOutput,
    config: &ProfileRenderConfig,
    view: &dyn ChartView,
    _anchor_x: f32,
    box_right: f32,
) {
    let label_x = box_right + 4.0;

    // POC label
    if config.poc_config.show_poc
        && config.poc_config.show_poc_label
        && let Some(idx) = poc
        && let Some(level) = levels.get(idx)
    {
        let y = view.price_units_to_y(level.price_units);
        let color = to_color(config.poc_config.poc_color, 1.0);
        draw_label(canvas, &format!("POC {:.2}", level.price), label_x, y, color);
    }

    // VA labels
    if config.va_config.show_value_area
        && config.va_config.show_va_labels
        && let Some((vah_idx, val_idx)) = value_area
    {
        if let Some(level) = levels.get(vah_idx) {
            let y = view.price_units_to_y(level.price_units);
            let color = to_color(config.va_config.vah_color, 1.0);
            draw_label(
                canvas,
                &format!("VAH {:.2}", level.price),
                label_x,
                y,
                color,
            );
        }
        if let Some(level) = levels.get(val_idx) {
            let y = view.price_units_to_y(level.price_units);
            let color = to_color(config.va_config.val_color, 1.0);
            draw_label(
                canvas,
                &format!("VAL {:.2}", level.price),
                label_x,
                y,
                color,
            );
        }
    }

    // Peak label
    if config.node_config.show_peak_line
        && config.node_config.show_peak_label
        && let Some(ref node) = output.peak_node
    {
        let y = view.price_units_to_y(node.price_units);
        let color = to_color(config.node_config.peak_color, 1.0);
        draw_label(
            canvas,
            &format!("Peak {:.2}", node.price),
            label_x,
            y,
            color,
        );
    }

    // Valley label
    if config.node_config.show_valley_line
        && config.node_config.show_valley_label
        && let Some(ref node) = output.valley_node
    {
        let y = view.price_units_to_y(node.price_units);
        let color = to_color(config.node_config.valley_color, 1.0);
        draw_label(
            canvas,
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
        let y = view.value_to_y(last.1);
        let color = to_color(config.vwap_config.vwap_color, 1.0);
        draw_label(
            canvas,
            &format!("VWAP {:.2}", last.1),
            label_x,
            y,
            color,
        );
    }
}
