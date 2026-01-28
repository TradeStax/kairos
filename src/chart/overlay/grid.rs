//! Grid Overlay
//!
//! Draws background grid lines for price and time levels.

use iced::theme::palette::Extended;
use iced::widget::canvas::{Frame, LineDash, Path, Stroke};
use iced::{Point, Size};

/// Configuration for grid drawing
pub struct GridConfig {
    /// Number of horizontal grid lines
    pub horizontal_lines: usize,
    /// Number of vertical grid lines
    pub vertical_lines: usize,
    /// Line width
    pub line_width: f32,
    /// Line alpha (0.0-1.0)
    pub alpha: f32,
}

impl Default for GridConfig {
    fn default() -> Self {
        Self {
            horizontal_lines: 5,
            vertical_lines: 6,
            line_width: 1.0,
            alpha: 0.1,
        }
    }
}

/// Draw grid lines on the chart
pub fn draw_grid(frame: &mut Frame, palette: &Extended, bounds: Size, config: &GridConfig) {
    let grid_color = palette.background.strong.color.scale_alpha(config.alpha);

    let grid_stroke = Stroke {
        width: config.line_width,
        line_dash: LineDash {
            segments: &[2.0, 4.0],
            offset: 0,
        },
        ..Default::default()
    };
    let grid_stroke = Stroke::with_color(grid_stroke, grid_color);

    // Horizontal grid lines
    if config.horizontal_lines > 0 {
        let step = bounds.height / (config.horizontal_lines as f32 + 1.0);
        for i in 1..=config.horizontal_lines {
            let y = step * i as f32;
            frame.stroke(
                &Path::line(Point::new(0.0, y), Point::new(bounds.width, y)),
                grid_stroke,
            );
        }
    }

    // Vertical grid lines
    if config.vertical_lines > 0 {
        let step = bounds.width / (config.vertical_lines as f32 + 1.0);
        for i in 1..=config.vertical_lines {
            let x = step * i as f32;
            frame.stroke(
                &Path::line(Point::new(x, 0.0), Point::new(x, bounds.height)),
                grid_stroke,
            );
        }
    }
}

/// Draw grid lines aligned to specific price/time values
pub fn draw_aligned_grid(
    frame: &mut Frame,
    palette: &Extended,
    bounds: Size,
    y_positions: &[f32],
    x_positions: &[f32],
    config: &GridConfig,
) {
    let grid_color = palette.background.strong.color.scale_alpha(config.alpha);

    let grid_stroke = Stroke {
        width: config.line_width,
        line_dash: LineDash {
            segments: &[2.0, 4.0],
            offset: 0,
        },
        ..Default::default()
    };
    let grid_stroke = Stroke::with_color(grid_stroke, grid_color);

    // Horizontal grid lines at specific Y positions
    for &y in y_positions {
        if y >= 0.0 && y <= bounds.height {
            frame.stroke(
                &Path::line(Point::new(0.0, y), Point::new(bounds.width, y)),
                grid_stroke,
            );
        }
    }

    // Vertical grid lines at specific X positions
    for &x in x_positions {
        if x >= 0.0 && x <= bounds.width {
            frame.stroke(
                &Path::line(Point::new(x, 0.0), Point::new(x, bounds.height)),
                grid_stroke,
            );
        }
    }
}
