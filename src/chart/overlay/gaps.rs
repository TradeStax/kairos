//! Gap Marker Overlay
//!
//! Renders visual indicators for data gaps on charts.
//! - NoData: subtle red-tinted semi-transparent band
//! - MarketClosed: gray band (very subtle)
//! - PartialCoverage: yellow-tinted band

use crate::chart::ViewState;
use data::domain::chart::{DataGap, DataGapKind};
use iced::widget::canvas;
use iced::{Color, Point, Rectangle, Size};

/// Draw gap markers for all gaps visible in the current chart region.
///
/// Called after main chart content rendering, before the crosshair layer.
/// Operates in the chart's translated/scaled coordinate space.
pub fn draw_gap_markers(
    frame: &mut canvas::Frame,
    chart: &ViewState,
    gaps: &[DataGap],
    region: &Rectangle,
) {
    if gaps.is_empty() {
        return;
    }

    let (earliest, latest) = chart.interval_range(region);

    for gap in gaps {
        // Skip gaps outside visible range
        if gap.end.0 < earliest || gap.start.0 > latest {
            continue;
        }

        // Clamp to visible range
        let start_time = gap.start.0.max(earliest);
        let end_time = gap.end.0.min(latest);

        let x_start = chart.interval_to_x(start_time);
        let x_end = chart.interval_to_x(end_time);

        let width = x_end - x_start;
        if width.abs() < 0.5 {
            continue;
        }

        let (left, w) = if width > 0.0 {
            (x_start, width)
        } else {
            (x_end, -width)
        };

        let color = gap_color(&gap.kind);

        // Draw semi-transparent band covering full price height
        frame.fill_rectangle(
            Point::new(left, region.y),
            Size::new(w, region.height),
            color,
        );
    }
}

/// Get the color for a gap kind
fn gap_color(kind: &DataGapKind) -> Color {
    match kind {
        DataGapKind::NoData => Color::from_rgba(0.8, 0.2, 0.2, 0.08),
        DataGapKind::MarketClosed => Color::from_rgba(0.5, 0.5, 0.5, 0.05),
        DataGapKind::PartialCoverage { .. } => Color::from_rgba(0.8, 0.7, 0.2, 0.08),
    }
}
