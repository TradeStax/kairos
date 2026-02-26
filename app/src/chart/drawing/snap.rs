//! Drawing snap constraints
//!
//! Provides constraint functions for Shift-key drawing operations:
//! - Angle snapping to 45 degree increments for line-based tools
//! - Square constraint for rectangles
//! - Circle constraint for ellipses
//! - Axis locking for whole-drawing moves

use super::point::DrawingPoint;
use crate::chart::ViewState;
use crate::drawing::DrawingTool;
use iced::{Point, Size};

/// Snap angle to nearest 45 degree increment relative to anchor point.
pub fn constrain_angle(anchor: Point, cursor: Point) -> Point {
    let dx = cursor.x - anchor.x;
    let dy = cursor.y - anchor.y;
    let distance = (dx * dx + dy * dy).sqrt();

    if distance < 0.001 {
        return cursor;
    }

    let angle = dy.atan2(dx);
    let snap_angle = (angle / std::f32::consts::FRAC_PI_4).round() * std::f32::consts::FRAC_PI_4;

    Point::new(
        anchor.x + distance * snap_angle.cos(),
        anchor.y + distance * snap_angle.sin(),
    )
}

/// Constrain to equal width/height (square) relative to anchor corner.
pub fn constrain_square(anchor: Point, cursor: Point) -> Point {
    let dx = cursor.x - anchor.x;
    let dy = cursor.y - anchor.y;
    let max_dim = dx.abs().max(dy.abs());

    Point::new(
        anchor.x + max_dim * dx.signum(),
        anchor.y + max_dim * dy.signum(),
    )
}

/// Constrain to equal radii (circle) relative to center point.
pub fn constrain_circle(center: Point, cursor: Point) -> Point {
    constrain_square(center, cursor)
}

/// Lock movement to horizontal or vertical axis based on dominant direction.
pub fn constrain_axis(start: Point, cursor: Point) -> Point {
    let dx = (cursor.x - start.x).abs();
    let dy = (cursor.y - start.y).abs();

    if dx >= dy {
        Point::new(cursor.x, start.y)
    } else {
        Point::new(start.x, cursor.y)
    }
}

/// Apply constraint for drawing creation based on tool type.
/// `anchor` is the first confirmed point in screen coordinates.
pub fn constrain_creation(tool: DrawingTool, anchor: Point, cursor: Point) -> Point {
    match tool {
        DrawingTool::Line
        | DrawingTool::Ray
        | DrawingTool::ExtendedLine
        | DrawingTool::Arrow
        | DrawingTool::FibRetracement => constrain_angle(anchor, cursor),

        DrawingTool::Rectangle => constrain_square(anchor, cursor),

        DrawingTool::Ellipse => constrain_circle(anchor, cursor),

        // Entry handle: lock to H or V axis
        DrawingTool::BuyCalculator | DrawingTool::SellCalculator => constrain_axis(anchor, cursor),

        _ => cursor,
    }
}

/// Apply constraint for handle drag based on tool type and handle index.
/// Returns the constrained cursor position in screen coordinates.
pub fn constrain_handle(
    tool: DrawingTool,
    points: &[DrawingPoint],
    handle_index: usize,
    state: &ViewState,
    bounds: Size,
    cursor: Point,
) -> Point {
    // VBP handles only change time; no square/angle constraint applies
    if tool.is_vbp() {
        return cursor;
    }

    // Calculator tools: handle 0 = axis lock, handle 1 = free,
    // handle 2 = lock X to point 1
    if matches!(
        tool,
        DrawingTool::BuyCalculator | DrawingTool::SellCalculator
    ) {
        return match handle_index {
            0 => {
                // Entry: lock to H or V axis from point 1
                if points.len() > 1 {
                    let anchor = points[1].as_screen_point(state, bounds);
                    constrain_axis(anchor, cursor)
                } else {
                    cursor
                }
            }
            2 => {
                // Stop: lock X to point 1's X (Y is free)
                if points.len() > 1 {
                    let p1_screen = points[1].as_screen_point(state, bounds);
                    Point::new(p1_screen.x, cursor.y)
                } else {
                    cursor
                }
            }
            _ => cursor, // Target (handle 1): free
        };
    }

    let anchor_index = match tool {
        DrawingTool::Line
        | DrawingTool::Ray
        | DrawingTool::ExtendedLine
        | DrawingTool::Arrow
        | DrawingTool::FibRetracement
        | DrawingTool::Rectangle => {
            if handle_index == 0 {
                1
            } else {
                0
            }
        }
        DrawingTool::Ellipse => {
            if handle_index == 0 {
                // Dragging center point: no angle/shape constraint
                return cursor;
            }
            0
        }
        _ => return cursor,
    };

    if anchor_index >= points.len() {
        return cursor;
    }

    let anchor = points[anchor_index].as_screen_point(state, bounds);

    match tool {
        DrawingTool::Line
        | DrawingTool::Ray
        | DrawingTool::ExtendedLine
        | DrawingTool::Arrow
        | DrawingTool::FibRetracement => constrain_angle(anchor, cursor),

        DrawingTool::Rectangle => constrain_square(anchor, cursor),

        DrawingTool::Ellipse => constrain_circle(anchor, cursor),

        _ => cursor,
    }
}
