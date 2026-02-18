//! Performance Systems
//!
//! This module provides level-of-detail (LOD) rendering and
//! viewport culling for off-screen elements.

#[allow(dead_code)]
pub mod lod;
#[allow(dead_code)]
pub mod viewport;

// Re-export key types for convenience
pub use lod::{LodCalculator, LodIteratorExt, LodLevel};
pub use overlay::{OverlayConfig, OverlayPosition, PerformanceOverlay};
pub use presets::{BasisOptimizer, PerformancePreset, PresetSettings};
pub use progressive::{FrameStats, PhaseItem, ProgressiveRenderer, RenderPhase};
pub use viewport::{SpatialGrid, ViewportBounds, ViewportCuller};

use std::time::Instant;

/// Performance budgets for 60 FPS rendering
#[derive(Debug, Clone, Copy)]
pub struct RenderBudget {
    /// Maximum draw calls per frame
    pub max_draw_calls: usize,
    /// Maximum vertices per frame
    pub max_vertices: usize,
    /// Target frame time in milliseconds (16.67ms = 60 FPS)
    pub target_frame_time_ms: f32,
}

impl Default for RenderBudget {
    fn default() -> Self {
        Self {
            max_draw_calls: 50_000,
            max_vertices: 200_000,
            target_frame_time_ms: 16.67, // 60 FPS
        }
    }
}

impl RenderBudget {
    /// Strict budget for high-performance mode
    pub fn strict() -> Self {
        Self {
            max_draw_calls: 20_000,
            max_vertices: 100_000,
            target_frame_time_ms: 16.67,
        }
    }

    /// Relaxed budget for compatibility mode
    pub fn relaxed() -> Self {
        Self {
            max_draw_calls: 100_000,
            max_vertices: 400_000,
            target_frame_time_ms: 33.33, // 30 FPS acceptable
        }
    }

    /// Check if metrics are within budget
    pub fn is_within_budget(&self, metrics: &FrameMetrics) -> bool {
        metrics.draw_calls <= self.max_draw_calls
            && metrics.vertices <= self.max_vertices
            && metrics.frame_time_ms <= self.target_frame_time_ms
    }

    /// Calculate budget utilization percentage (0.0 to 1.0+)
    pub fn utilization(&self, metrics: &FrameMetrics) -> f32 {
        let draw_call_util = metrics.draw_calls as f32 / self.max_draw_calls as f32;
        let vertex_util = metrics.vertices as f32 / self.max_vertices as f32;
        let time_util = metrics.frame_time_ms / self.target_frame_time_ms;

        // Return the maximum utilization (bottleneck)
        draw_call_util.max(vertex_util).max(time_util)
    }
}

/// Frame rendering metrics
#[derive(Debug, Clone, Copy, Default)]
pub struct FrameMetrics {
    /// Number of draw calls this frame
    pub draw_calls: usize,
    /// Number of vertices rendered
    pub vertices: usize,
    /// Frame render time in milliseconds
    pub frame_time_ms: f32,
    /// Timestamp when frame started
    pub frame_start: Option<Instant>,
}

impl FrameMetrics {
    /// Create new metrics tracker
    pub fn new() -> Self {
        Self {
            draw_calls: 0,
            vertices: 0,
            frame_time_ms: 0.0,
            frame_start: Some(Instant::now()),
        }
    }

    /// Start tracking a new frame
    pub fn start_frame(&mut self) {
        self.draw_calls = 0;
        self.vertices = 0;
        self.frame_time_ms = 0.0;
        self.frame_start = Some(Instant::now());
    }

    /// End frame and calculate final time
    pub fn end_frame(&mut self) {
        if let Some(start) = self.frame_start {
            self.frame_time_ms = start.elapsed().as_secs_f32() * 1000.0;
        }
    }

    /// Record a draw call with vertex count
    pub fn record_draw_call(&mut self, vertex_count: usize) {
        self.draw_calls += 1;
        self.vertices += vertex_count;
    }

    /// Record multiple draw calls at once
    pub fn record_draw_calls(&mut self, count: usize, vertices_per_call: usize) {
        self.draw_calls += count;
        self.vertices += count * vertices_per_call;
    }

    /// Get estimated FPS from frame time
    pub fn fps(&self) -> f32 {
        if self.frame_time_ms > 0.0 {
            1000.0 / self.frame_time_ms
        } else {
            0.0
        }
    }

    /// Check if frame is over budget
    pub fn is_over_budget(&self, budget: &RenderBudget) -> bool {
        !budget.is_within_budget(self)
    }
}

/// Rolling average performance tracker
pub struct PerformanceMonitor {
    /// Recent frame metrics (circular buffer)
    recent_frames: Vec<FrameMetrics>,
    /// Current write position in buffer
    write_index: usize,
    /// Number of frames tracked
    frame_count: usize,
    /// Budget to enforce
    budget: RenderBudget,
}

impl PerformanceMonitor {
    /// Create new performance monitor
    pub fn new(budget: RenderBudget) -> Self {
        Self {
            recent_frames: vec![FrameMetrics::default(); 60], // Track last 60 frames (1 second)
            write_index: 0,
            frame_count: 0,
            budget,
        }
    }

    /// Record a completed frame
    pub fn record_frame(&mut self, metrics: FrameMetrics) {
        self.recent_frames[self.write_index] = metrics;
        self.write_index = (self.write_index + 1) % self.recent_frames.len();
        self.frame_count += 1;
    }

    /// Get average frame time over recent frames
    pub fn avg_frame_time(&self) -> f32 {
        let count = self.frame_count.min(self.recent_frames.len());
        if count == 0 {
            return 0.0;
        }

        let sum: f32 = self.recent_frames.iter().take(count).map(|m| m.frame_time_ms).sum();
        sum / count as f32
    }

    /// Get average FPS
    pub fn avg_fps(&self) -> f32 {
        let avg_time = self.avg_frame_time();
        if avg_time > 0.0 {
            1000.0 / avg_time
        } else {
            0.0
        }
    }

    /// Get average draw calls per frame
    pub fn avg_draw_calls(&self) -> f32 {
        let count = self.frame_count.min(self.recent_frames.len());
        if count == 0 {
            return 0.0;
        }

        let sum: usize = self.recent_frames.iter().take(count).map(|m| m.draw_calls).sum();
        sum as f32 / count as f32
    }

    /// Check if performance is consistently under budget
    pub fn is_stable(&self) -> bool {
        let count = self.frame_count.min(self.recent_frames.len());
        if count < 10 {
            return true; // Not enough data
        }

        // Check last 10 frames
        self.recent_frames
            .iter()
            .take(10)
            .all(|m| self.budget.is_within_budget(m))
    }

    /// Get budget utilization percentage
    pub fn budget_utilization(&self) -> f32 {
        let count = self.frame_count.min(self.recent_frames.len());
        if count == 0 {
            return 0.0;
        }

        let sum: f32 = self.recent_frames
            .iter()
            .take(count)
            .map(|m| self.budget.utilization(m))
            .sum();

        sum / count as f32
    }

    /// Suggest quality adjustment based on performance
    pub fn suggested_quality_adjustment(&self) -> QualityAdjustment {
        let utilization = self.budget_utilization();
        let is_stable = self.is_stable();

        if utilization > 1.5 {
            QualityAdjustment::DecreaseMajor // Severely over budget
        } else if utilization > 1.1 {
            QualityAdjustment::DecreaseMinor // Slightly over budget
        } else if utilization < 0.5 && is_stable {
            QualityAdjustment::IncreaseMajor // Plenty of headroom
        } else if utilization < 0.7 && is_stable {
            QualityAdjustment::IncreaseMinor // Some headroom
        } else {
            QualityAdjustment::Maintain // Just right
        }
    }
}

/// Quality adjustment recommendation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QualityAdjustment {
    /// Significantly reduce quality (switch to lower LOD, increase decimation)
    DecreaseMajor,
    /// Slightly reduce quality
    DecreaseMinor,
    /// Maintain current quality
    Maintain,
    /// Slightly increase quality
    IncreaseMinor,
    /// Significantly increase quality (switch to higher LOD, reduce decimation)
    IncreaseMajor,
}

impl QualityAdjustment {
    /// Apply adjustment to LOD level
    pub fn adjust_lod(&self, current: LodLevel) -> LodLevel {
        match self {
            QualityAdjustment::DecreaseMajor => LodLevel::Low,
            QualityAdjustment::DecreaseMinor => match current {
                LodLevel::High => LodLevel::Medium,
                _ => LodLevel::Low,
            },
            QualityAdjustment::Maintain => current,
            QualityAdjustment::IncreaseMinor => match current {
                LodLevel::Low => LodLevel::Medium,
                _ => LodLevel::High,
            },
            QualityAdjustment::IncreaseMajor => LodLevel::High,
        }
    }

    /// Apply adjustment to decimation factor
    pub fn adjust_decimation(&self, current: usize) -> usize {
        match self {
            QualityAdjustment::DecreaseMajor => current * 2,
            QualityAdjustment::DecreaseMinor => (current * 3 / 2).max(current + 1),
            QualityAdjustment::Maintain => current,
            QualityAdjustment::IncreaseMinor => (current * 2 / 3).max(1),
            QualityAdjustment::IncreaseMajor => (current / 2).max(1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_budget_within() {
        let budget = RenderBudget::default();
        let metrics = FrameMetrics {
            draw_calls: 10_000,
            vertices: 50_000,
            frame_time_ms: 12.0,
            frame_start: None,
        };
        assert!(budget.is_within_budget(&metrics));
    }

    #[test]
    fn test_budget_exceeded() {
        let budget = RenderBudget::default();
        let metrics = FrameMetrics {
            draw_calls: 60_000, // Over budget
            vertices: 50_000,
            frame_time_ms: 12.0,
            frame_start: None,
        };
        assert!(!budget.is_within_budget(&metrics));
    }

    #[test]
    fn test_quality_adjustment_lod() {
        let adj = QualityAdjustment::DecreaseMajor;
        assert_eq!(adj.adjust_lod(LodLevel::High), LodLevel::Low);

        let adj = QualityAdjustment::IncreaseMajor;
        assert_eq!(adj.adjust_lod(LodLevel::Low), LodLevel::High);
    }

    #[test]
    fn test_performance_monitor() {
        let mut monitor = PerformanceMonitor::new(RenderBudget::default());

        let metrics = FrameMetrics {
            draw_calls: 10_000,
            vertices: 50_000,
            frame_time_ms: 14.0,
            frame_start: None,
        };

        monitor.record_frame(metrics);

        // After recording one frame with 14.0ms, the average should be exactly 14.0ms
        let avg_time = monitor.avg_frame_time();
        assert!(
            (avg_time - 14.0).abs() < 0.001,
            "Expected avg_frame_time ~14.0ms, got {}",
            avg_time
        );

        // FPS = 1000.0 / 14.0 = ~71.43
        let avg_fps = monitor.avg_fps();
        let expected_fps = 1000.0 / 14.0;
        assert!(
            (avg_fps - expected_fps).abs() < 0.01,
            "Expected avg_fps ~{:.2}, got {:.2}",
            expected_fps,
            avg_fps
        );

        // avg_draw_calls should be exactly 10_000
        let avg_draws = monitor.avg_draw_calls();
        assert!(
            (avg_draws - 10_000.0).abs() < 0.001,
            "Expected avg_draw_calls 10000.0, got {}",
            avg_draws
        );
    }
}
