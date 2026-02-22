//! Position calculator drawing rendering: BuyCalculator, SellCalculator.

use super::{DrawContext, create_stroke, draw_calc_label};
use super::super::Drawing;
use data::{DrawingTool, LineStyle};
use iced::widget::canvas::{Frame, Path};
use iced::{Point, Size};

pub fn draw(
    frame: &mut Frame,
    ctx: &DrawContext<'_>,
    drawing: &Drawing,
    pts: &[Point],
) {
    if pts.len() < 2 {
        return;
    }

    let config = drawing
        .style
        .position_calc
        .as_ref()
        .cloned()
        .unwrap_or_default();

    let is_buy = matches!(drawing.tool, DrawingTool::BuyCalculator);

    let entry_y = pts[0].y;
    let target_y = pts[1].y;
    let left_x = pts[0].x.min(pts[1].x);
    let right_x = pts[0].x.max(pts[1].x);
    let draw_width = (right_x - left_x).max(1.0);

    let entry_price = drawing.points[0].price.to_f64();
    let target_price = drawing.points[1].price.to_f64();

    // Determine stop price/y — use point 2 if it exists, otherwise mirror
    let (stop_y, stop_price) = if pts.len() >= 3 {
        (pts[2].y, drawing.points[2].price.to_f64())
    } else {
        // Preview mode: mirror target for 1:1 R:R
        let delta = target_y - entry_y;
        let price_delta = target_price - entry_price;
        (entry_y - delta, entry_price - price_delta)
    };

    // Get ticker info for P&L calculations
    let tick_size = ctx.state.ticker_info.tick_size as f64;
    let contract_size = ctx.state.ticker_info.contract_size as f64;
    let tick_value = tick_size * contract_size;
    let quantity = config.quantity.max(1) as f64;

    // Target zone calculations
    let target_delta = (target_price - entry_price).abs();
    let target_ticks = if tick_size > 0.0 {
        target_delta / tick_size
    } else {
        0.0
    };
    let target_pnl = target_ticks * tick_value * quantity;

    // Stop zone calculations
    let stop_delta = (stop_price - entry_price).abs();
    let stop_ticks = if tick_size > 0.0 {
        stop_delta / tick_size
    } else {
        0.0
    };
    let stop_pnl = stop_ticks * tick_value * quantity;

    // R:R ratio
    let rr_str = if stop_ticks > 0.0 {
        format!("{:.1}", target_ticks / stop_ticks)
    } else {
        "--".to_string()
    };

    // Colors
    let target_color =
        crate::style::theme_bridge::rgba_to_iced_color(config.target_color);
    let stop_color =
        crate::style::theme_bridge::rgba_to_iced_color(config.stop_color);

    let is_preview = pts.len() < 3;
    let zone_alpha = if is_preview {
        ctx.alpha * 0.5
    } else {
        ctx.alpha
    };

    // Draw target zone
    let target_min_y = entry_y.min(target_y);
    let target_height = (entry_y - target_y).abs();
    let target_zone = Path::rectangle(
        Point::new(left_x, target_min_y),
        Size::new(draw_width, target_height),
    );
    frame.fill(
        &target_zone,
        target_color.scale_alpha(config.target_opacity * zone_alpha),
    );

    // Draw stop zone
    let stop_min_y = entry_y.min(stop_y);
    let stop_height = (entry_y - stop_y).abs();
    let stop_zone = Path::rectangle(
        Point::new(left_x, stop_min_y),
        Size::new(draw_width, stop_height),
    );
    frame.fill(
        &stop_zone,
        stop_color.scale_alpha(config.stop_opacity * zone_alpha),
    );

    // Draw entry line
    let entry_stroke = create_stroke(
        ctx.stroke_color.scale_alpha(ctx.alpha),
        ctx.stroke_width,
        LineStyle::Solid,
    );
    frame.stroke(
        &Path::line(Point::new(left_x, entry_y), Point::new(right_x, entry_y)),
        entry_stroke,
    );

    // Draw target line (dashed)
    let target_stroke = create_stroke(
        target_color.scale_alpha(ctx.alpha),
        ctx.stroke_width,
        LineStyle::Dashed,
    );
    frame.stroke(
        &Path::line(Point::new(left_x, target_y), Point::new(right_x, target_y)),
        target_stroke,
    );

    // Draw stop line (dashed)
    let stop_stroke = create_stroke(
        stop_color.scale_alpha(ctx.alpha),
        ctx.stroke_width,
        LineStyle::Dashed,
    );
    frame.stroke(
        &Path::line(Point::new(left_x, stop_y), Point::new(right_x, stop_y)),
        stop_stroke,
    );

    // Draw labels
    let font_size = config.label_font_size;

    if config.show_target_label {
        let mut parts = Vec::new();
        if config.show_pnl {
            parts.push(format!("${:.0}", target_pnl));
        }
        if config.show_ticks {
            parts.push(format!("{:.0} ticks", target_ticks));
        }
        if parts.is_empty() {
            parts.push(format!("{:.2}", target_price));
        }
        let target_text = parts.join(" | ");
        let label_y = if target_y < entry_y {
            target_y + 2.0
        } else {
            target_y - font_size - 8.0
        };
        draw_calc_label(
            frame,
            &target_text,
            Point::new(left_x + 4.0, label_y),
            target_color,
            font_size,
            ctx.alpha,
        );
    }

    if config.show_entry_label {
        let side_str = if is_buy { "BUY" } else { "SELL" };
        let entry_text =
            format!("{} {} | R:R {}", side_str, config.quantity, rr_str);
        draw_calc_label(
            frame,
            &entry_text,
            Point::new(left_x + 4.0, entry_y - font_size - 8.0),
            ctx.stroke_color,
            font_size,
            ctx.alpha,
        );
    }

    if config.show_stop_label {
        let mut parts = Vec::new();
        if config.show_pnl {
            parts.push(format!("-${:.0}", stop_pnl));
        }
        if config.show_ticks {
            parts.push(format!("{:.0} ticks", stop_ticks));
        }
        if parts.is_empty() {
            parts.push(format!("{:.2}", stop_price));
        }
        let stop_text = parts.join(" | ");
        let label_y = if stop_y > entry_y {
            stop_y + 2.0
        } else {
            stop_y - font_size - 8.0
        };
        draw_calc_label(
            frame,
            &stop_text,
            Point::new(left_x + 4.0, label_y),
            stop_color,
            font_size,
            ctx.alpha,
        );
    }
}
