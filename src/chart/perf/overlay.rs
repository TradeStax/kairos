//! Performance Overlay
//!
//! Optional debug overlay showing real-time performance metrics.
//! Useful for development and troubleshooting performance issues.

use super::{FrameMetrics, RenderBudget};
use super::lod::LodLevel;
use iced::widget::canvas::{self, Frame, Text};
use iced::{Color, Point};
use crate::style;

/// Performance overlay configuration
#[derive(Debug, Clone, Copy)]
pub struct OverlayConfig {
    /// Show the overlay
    pub enabled: bool,
    /// Position on screen
    pub position: OverlayPosition,
    /// Text size
    pub text_size: f32,
}

impl Default for OverlayConfig {
    fn default() -> Self {
        Self {
            enabled: false, // Disabled by default
            position: OverlayPosition::TopRight,
            text_size: 10.0,
        }
    }
}

/// Overlay position on screen
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayPosition {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// Performance overlay renderer
pub struct PerformanceOverlay {
    config: OverlayConfig,
}

impl PerformanceOverlay {
    /// Create new performance overlay
    pub fn new(config: OverlayConfig) -> Self {
        Self { config }
    }

    /// Draw performance metrics on frame
    pub fn draw(
        &self,
        frame: &mut Frame,
        metrics: &FrameMetrics,
        budget: &RenderBudget,
        lod_level: LodLevel,
        viewport_size: (f32, f32),
    ) {
        if !self.config.enabled {
            return;
        }

        let fps = metrics.fps();
        let utilization = budget.utilization(metrics);

        // Determine color based on performance
        let perf_color = if fps >= 55.0 {
            Color::from_rgb(0.0, 1.0, 0.0) // Green - excellent
        } else if fps >= 40.0 {
            Color::from_rgb(1.0, 1.0, 0.0) // Yellow - acceptable
        } else {
            Color::from_rgb(1.0, 0.0, 0.0) // Red - poor
        };

        // Build metrics text
        let lines = vec![
            format!("FPS: {:.1}", fps),
            format!("Frame: {:.2}ms", metrics.frame_time_ms),
            format!("Draws: {}", metrics.draw_calls),
            format!("Verts: {}", metrics.vertices),
            format!("Budget: {:.0}%", utilization * 100.0),
            format!("LOD: {:?}", lod_level),
        ];

        // Calculate position
        let (x, y) = self.calculate_position(viewport_size);
        let text_size = self.config.text_size;
        let line_height = text_size * 1.4;

        // Draw background
        let bg_width = 120.0;
        let bg_height = lines.len() as f32 * line_height + 8.0;

        frame.fill_rectangle(
            Point::new(x - 4.0, y - 4.0),
            iced::Size::new(bg_width, bg_height),
            Color::from_rgba(0.0, 0.0, 0.0, 0.7),
        );

        // Draw metrics
        for (i, line) in lines.iter().enumerate() {
            let color = if i == 0 { perf_color } else { Color::WHITE };

            frame.fill_text(Text {
                content: line.clone(),
                position: Point::new(x, y + (i as f32 * line_height)),
                size: iced::Pixels(text_size),
                color,
                font: style::AZERET_MONO,
                ..Text::default()
            });
        }
    }

    /// Calculate overlay position based on viewport size
    fn calculate_position(&self, viewport_size: (f32, f32)) -> (f32, f32) {
        let margin = 8.0;

        match self.config.position {
            OverlayPosition::TopLeft => (margin, margin),
            OverlayPosition::TopRight => (viewport_size.0 - 128.0 - margin, margin),
            OverlayPosition::BottomLeft => (margin, viewport_size.1 - 100.0 - margin),
            OverlayPosition::BottomRight => {
                (viewport_size.0 - 128.0 - margin, viewport_size.1 - 100.0 - margin)
            }
        }
    }

    /// Toggle overlay on/off
    pub fn toggle(&mut self) {
        self.config.enabled = !self.config.enabled;
    }

    /// Set overlay enabled state
    pub fn set_enabled(&mut self, enabled: bool) {
        self.config.enabled = enabled;
    }

    /// Check if overlay is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }
}

impl Default for PerformanceOverlay {
    fn default() -> Self {
        Self::new(OverlayConfig::default())
    }
}
