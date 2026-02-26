//! Ruler Measurement Tool
//!
//! Draws a measurement rectangle between two points showing:
//! - Price difference (percentage)
//! - Time/bar difference
//! - Number of bars/ticks

use crate::chart::core::ViewState;
use crate::chart::core::tokens as chart_tokens;
use crate::components::primitives::AZERET_MONO;
use crate::style::tokens;
use data::ChartBasis;
use data::Price;
use iced::theme::palette::Extended;
use iced::widget::canvas::{Frame, Text};
use iced::{Alignment, Point, Size};

const TEXT_SIZE: f32 = tokens::text::BODY;

/// Draw ruler measurement between two points
pub fn draw_ruler(
    state: &ViewState,
    frame: &mut Frame,
    palette: &Extended,
    bounds: Size,
    start: Point,
    end: Point,
) {
    let region = state.visible_region(bounds);

    let highest_p = state.y_to_price(region.y);
    let lowest_p = state.y_to_price(region.y + region.height);
    let highest = highest_p.to_f32();
    let lowest = lowest_p.to_f32();

    let snap_y = |y: f32| -> f32 {
        let ratio = y / bounds.height;
        let price = highest + ratio * (lowest - highest);

        let rounded_price_p = Price::from_f32(price).round_to_tick(state.tick_size.to_price());
        let rounded_price = rounded_price_p.to_f32();
        let price_range = lowest - highest;
        let snap_ratio = if price_range.abs() < f32::EPSILON {
            0.5
        } else {
            (rounded_price - highest) / price_range
        };
        snap_ratio * bounds.height
    };

    let snap_x = |x: f32| -> f32 {
        let (_, snap_ratio) = state.snap_x_to_index(x, bounds, region);
        snap_ratio * bounds.width
    };

    let snapped_p1_x = snap_x(start.x);
    let snapped_p1_y = snap_y(start.y);
    let snapped_p2_x = snap_x(end.x);
    let snapped_p2_y = snap_y(end.y);

    let price1 = state.y_to_price(snapped_p1_y);
    let price2 = state.y_to_price(snapped_p2_y);

    let p1 = price1.to_f32();
    let pct = if p1.abs() < f32::EPSILON {
        0.0
    } else {
        ((price2.to_f32() - p1) / p1) * 100.0
    };
    let pct_text = format!("{:.2}%", pct);

    let interval_diff: String = match state.basis {
        ChartBasis::Time(_) => {
            let (timestamp1, _) = state.snap_x_to_index(start.x, bounds, region);
            let (timestamp2, _) = state.snap_x_to_index(end.x, bounds, region);

            let diff_ms: u64 = timestamp1.abs_diff(timestamp2);
            data::util::format_duration_ms(diff_ms)
        }
        ChartBasis::Tick(_) => {
            let (tick1, _) = state.snap_x_to_index(start.x, bounds, region);
            let (tick2, _) = state.snap_x_to_index(end.x, bounds, region);

            let tick_diff = tick1.abs_diff(tick2);
            format!("{} ticks", tick_diff)
        }
    };

    let rect_x = snapped_p1_x.min(snapped_p2_x);
    let rect_y = snapped_p1_y.min(snapped_p2_y);
    let rect_w = (snapped_p1_x - snapped_p2_x).abs();
    let rect_h = (snapped_p1_y - snapped_p2_y).abs();

    // Draw filled rectangle
    frame.fill_rectangle(
        Point::new(rect_x, rect_y),
        Size::new(rect_w, rect_h),
        palette
            .primary
            .base
            .color
            .scale_alpha(chart_tokens::ruler::FILL_ALPHA),
    );

    // Find corner closest to cursor for text positioning
    let corners = [
        Point::new(rect_x, rect_y),
        Point::new(rect_x + rect_w, rect_y),
        Point::new(rect_x, rect_y + rect_h),
        Point::new(rect_x + rect_w, rect_y + rect_h),
    ];

    let (text_corner, idx) = corners
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            let da = (a.x - end.x).hypot(a.y - end.y);
            let db = (b.x - end.x).hypot(b.y - end.y);
            da.partial_cmp(&db).unwrap()
        })
        .map(|(i, &c)| (c, i))
        .unwrap();

    let text_padding = chart_tokens::ruler::TEXT_PADDING;
    let text_pos = match idx {
        0 => Point::new(text_corner.x + text_padding, text_corner.y + text_padding),
        1 => Point::new(text_corner.x - text_padding, text_corner.y + text_padding),
        2 => Point::new(text_corner.x + text_padding, text_corner.y - text_padding),
        3 => Point::new(text_corner.x - text_padding, text_corner.y - text_padding),
        _ => text_corner,
    };

    let datapoints_text = match state.basis {
        ChartBasis::Time(timeframe) => {
            let interval_ms = timeframe.to_milliseconds();
            let (timestamp1, _) = state.snap_x_to_index(start.x, bounds, region);
            let (timestamp2, _) = state.snap_x_to_index(end.x, bounds, region);

            let diff_ms = timestamp1.abs_diff(timestamp2);
            let datapoints = (diff_ms / interval_ms).max(1);
            format!("{} bars", datapoints)
        }
        ChartBasis::Tick(aggregation) => {
            let (tick1, _) = state.snap_x_to_index(start.x, bounds, region);
            let (tick2, _) = state.snap_x_to_index(end.x, bounds, region);

            let tick_diff = tick1.abs_diff(tick2);
            let datapoints = (tick_diff / u64::from(aggregation)).max(1);
            format!("{} bars", datapoints)
        }
    };

    let label_text = format!("{}, {} | {}", datapoints_text, interval_diff, pct_text);

    let text_width = (label_text.len() as f32) * TEXT_SIZE * 0.6;
    let text_height = TEXT_SIZE * 1.2;
    let rect_padding = chart_tokens::ruler::RECT_PADDING;

    let (bg_x, bg_y) = match idx {
        0 => (text_pos.x - rect_padding, text_pos.y - rect_padding),
        1 => (
            text_pos.x - text_width - rect_padding,
            text_pos.y - rect_padding,
        ),
        2 => (
            text_pos.x - rect_padding,
            text_pos.y - text_height - rect_padding,
        ),
        3 => (
            text_pos.x - text_width - rect_padding,
            text_pos.y - text_height - rect_padding,
        ),
        _ => (
            text_pos.x - text_width / 2.0 - rect_padding,
            text_pos.y - text_height / 2.0 - rect_padding,
        ),
    };

    // Draw background for text
    frame.fill_rectangle(
        Point::new(bg_x, bg_y),
        Size::new(
            text_width + rect_padding * 2.0,
            text_height + rect_padding * 2.0,
        ),
        palette.background.weakest.color.scale_alpha(0.9),
    );

    // Draw text
    frame.fill_text(Text {
        content: label_text,
        position: text_pos,
        color: palette.background.base.text,
        size: iced::Pixels(11.0),
        align_x: match idx {
            0 | 2 => Alignment::Start.into(),
            1 | 3 => Alignment::End.into(),
            _ => Alignment::Center.into(),
        },
        align_y: match idx {
            0 | 1 => Alignment::Start.into(),
            2 | 3 => Alignment::End.into(),
            _ => Alignment::Center.into(),
        },
        font: AZERET_MONO,
        ..Default::default()
    });
}
