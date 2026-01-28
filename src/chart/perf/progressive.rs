//! Progressive Rendering System
//!
//! Renders charts in multiple phases for smoother user experience.
//! Heavy charts render coarse structure first, then progressively add detail.
//!
//! ## Rendering Phases
//!
//! 1. **Core Phase** (0-5ms): Essential structure
//!    - Candles/depth zones
//!    - Price/time axes
//!    - Background grid
//!
//! 2. **Detail Phase** (5-12ms): Mid-level detail
//!    - Indicators
//!    - Volume bars
//!    - Aggregated trades
//!
//! 3. **Refinement Phase** (12-16ms): Fine details
//!    - Individual trade markers
//!    - Text labels
//!    - Studies/overlays
//!
//! ## Usage
//!
//! ```rust
//! let mut renderer = ProgressiveRenderer::new();
//!
//! // Phase 1: Draw core structure immediately
//! renderer.start_phase(RenderPhase::Core);
//! draw_candles(frame);
//! draw_axes(frame);
//! renderer.end_phase();
//!
//! // Phase 2: Add details if time budget allows
//! if renderer.can_render_phase(RenderPhase::Detail) {
//!     renderer.start_phase(RenderPhase::Detail);
//!     draw_indicators(frame);
//!     renderer.end_phase();
//! }
//! ```

use std::time::{Duration, Instant};

/// Rendering phase for progressive rendering
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RenderPhase {
    /// Core structure (must always render)
    Core = 0,
    /// Mid-level details (render if time permits)
    Detail = 1,
    /// Fine details (render if plenty of time)
    Refinement = 2,
}

impl RenderPhase {
    /// Maximum time budget for this phase (milliseconds)
    pub fn time_budget_ms(&self) -> f32 {
        match self {
            RenderPhase::Core => 5.0,        // Must finish in 5ms
            RenderPhase::Detail => 7.0,      // Additional 7ms for details (total 12ms)
            RenderPhase::Refinement => 4.0,  // Additional 4ms for refinement (total 16ms)
        }
    }

    /// Priority level (higher = more important)
    pub fn priority(&self) -> u8 {
        match self {
            RenderPhase::Core => 3,
            RenderPhase::Detail => 2,
            RenderPhase::Refinement => 1,
        }
    }

    /// Can this phase be skipped if over budget?
    pub fn is_skippable(&self) -> bool {
        !matches!(self, RenderPhase::Core)
    }

    /// Get all phases in render order
    pub fn all_in_order() -> &'static [RenderPhase] {
        &[
            RenderPhase::Core,
            RenderPhase::Detail,
            RenderPhase::Refinement,
        ]
    }
}

/// Progressive renderer with time tracking
pub struct ProgressiveRenderer {
    /// Frame start time
    frame_start: Instant,

    /// Current phase being rendered
    current_phase: Option<RenderPhase>,

    /// Time when current phase started
    phase_start: Option<Instant>,

    /// Time spent in each phase (milliseconds)
    phase_times: [f32; 3],

    /// Total frame budget (milliseconds)
    frame_budget_ms: f32,

    /// Whether frame is over budget
    over_budget: bool,
}

impl Default for ProgressiveRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl ProgressiveRenderer {
    /// Create new progressive renderer with 60 FPS target
    pub fn new() -> Self {
        Self::with_budget(16.67) // 60 FPS = 16.67ms per frame
    }

    /// Create with custom frame budget
    pub fn with_budget(budget_ms: f32) -> Self {
        Self {
            frame_start: Instant::now(),
            current_phase: None,
            phase_start: None,
            phase_times: [0.0; 3],
            frame_budget_ms: budget_ms,
            over_budget: false,
        }
    }

    /// Start a new frame
    pub fn start_frame(&mut self) {
        self.frame_start = Instant::now();
        self.current_phase = None;
        self.phase_start = None;
        self.phase_times = [0.0; 3];
        self.over_budget = false;
    }

    /// Start rendering a phase
    pub fn start_phase(&mut self, phase: RenderPhase) {
        // End previous phase if any
        if self.current_phase.is_some() {
            self.end_phase();
        }

        self.current_phase = Some(phase);
        self.phase_start = Some(Instant::now());
    }

    /// End current phase and record time
    pub fn end_phase(&mut self) {
        if let (Some(phase), Some(start)) = (self.current_phase, self.phase_start) {
            let elapsed_ms = start.elapsed().as_secs_f32() * 1000.0;
            self.phase_times[phase as usize] = elapsed_ms;

            // Check if we're over budget
            if self.total_frame_time_ms() > self.frame_budget_ms {
                self.over_budget = true;
            }
        }

        self.current_phase = None;
        self.phase_start = None;
    }

    /// Check if we can render a phase (within time budget)
    pub fn can_render_phase(&self, phase: RenderPhase) -> bool {
        // Core phase always renders
        if matches!(phase, RenderPhase::Core) {
            return true;
        }

        // Check if we have time budget remaining
        let elapsed_ms = self.frame_start.elapsed().as_secs_f32() * 1000.0;
        let remaining_ms = self.frame_budget_ms - elapsed_ms;

        // Need at least the phase's budget available
        remaining_ms >= phase.time_budget_ms()
    }

    /// Get total frame time so far
    pub fn total_frame_time_ms(&self) -> f32 {
        self.frame_start.elapsed().as_secs_f32() * 1000.0
    }

    /// Get time spent in a specific phase
    pub fn phase_time_ms(&self, phase: RenderPhase) -> f32 {
        self.phase_times[phase as usize]
    }

    /// Get remaining budget
    pub fn remaining_budget_ms(&self) -> f32 {
        let elapsed = self.total_frame_time_ms();
        (self.frame_budget_ms - elapsed).max(0.0)
    }

    /// Check if frame is over budget
    pub fn is_over_budget(&self) -> bool {
        self.over_budget || self.total_frame_time_ms() > self.frame_budget_ms
    }

    /// Get suggested phases to render based on current budget
    pub fn suggested_phases(&self) -> Vec<RenderPhase> {
        let mut phases = vec![RenderPhase::Core]; // Always render core

        if self.can_render_phase(RenderPhase::Detail) {
            phases.push(RenderPhase::Detail);
        }

        if self.can_render_phase(RenderPhase::Refinement) {
            phases.push(RenderPhase::Refinement);
        }

        phases
    }

    /// Get frame statistics
    pub fn stats(&self) -> FrameStats {
        FrameStats {
            total_time_ms: self.total_frame_time_ms(),
            core_time_ms: self.phase_times[0],
            detail_time_ms: self.phase_times[1],
            refinement_time_ms: self.phase_times[2],
            budget_ms: self.frame_budget_ms,
            over_budget: self.is_over_budget(),
            phases_rendered: self.phase_times.iter().filter(|&&t| t > 0.0).count(),
        }
    }
}

/// Frame rendering statistics
#[derive(Debug, Clone, Copy)]
pub struct FrameStats {
    pub total_time_ms: f32,
    pub core_time_ms: f32,
    pub detail_time_ms: f32,
    pub refinement_time_ms: f32,
    pub budget_ms: f32,
    pub over_budget: bool,
    pub phases_rendered: usize,
}

impl FrameStats {
    /// Get FPS from frame time
    pub fn fps(&self) -> f32 {
        if self.total_time_ms > 0.0 {
            1000.0 / self.total_time_ms
        } else {
            0.0
        }
    }

    /// Get budget utilization percentage (0.0 to 1.0+)
    pub fn utilization(&self) -> f32 {
        if self.budget_ms > 0.0 {
            self.total_time_ms / self.budget_ms
        } else {
            0.0
        }
    }
}

/// Render item with phase annotation
///
/// Allows marking individual render items with their appropriate phase
#[derive(Debug, Clone, Copy)]
pub struct PhaseItem<T> {
    pub item: T,
    pub phase: RenderPhase,
}

impl<T> PhaseItem<T> {
    pub fn new(item: T, phase: RenderPhase) -> Self {
        Self { item, phase }
    }

    /// Filter items by phase
    pub fn filter_by_phase<'a>(
        items: &'a [PhaseItem<T>],
        max_phase: RenderPhase,
    ) -> impl Iterator<Item = &'a T> + 'a {
        items
            .iter()
            .filter(move |item| item.phase <= max_phase)
            .map(|item| &item.item)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_phases() {
        let mut renderer = ProgressiveRenderer::new();

        renderer.start_frame();

        // Core phase should always be allowed
        assert!(renderer.can_render_phase(RenderPhase::Core));

        renderer.start_phase(RenderPhase::Core);
        std::thread::sleep(Duration::from_millis(2));
        renderer.end_phase();

        assert!(renderer.phase_time_ms(RenderPhase::Core) >= 2.0);
    }

    #[test]
    fn test_budget_tracking() {
        let mut renderer = ProgressiveRenderer::with_budget(10.0);

        renderer.start_frame();
        renderer.start_phase(RenderPhase::Core);
        std::thread::sleep(Duration::from_millis(8));
        renderer.end_phase();

        // Should have ~2ms remaining
        assert!(renderer.remaining_budget_ms() < 3.0);
        assert!(renderer.remaining_budget_ms() > 0.0);
    }

    #[test]
    fn test_phase_items() {
        let items = vec![
            PhaseItem::new("candle", RenderPhase::Core),
            PhaseItem::new("indicator", RenderPhase::Detail),
            PhaseItem::new("trade", RenderPhase::Refinement),
        ];

        let core_only: Vec<_> = PhaseItem::filter_by_phase(&items, RenderPhase::Core).collect();
        assert_eq!(core_only.len(), 1);

        let up_to_detail: Vec<_> = PhaseItem::filter_by_phase(&items, RenderPhase::Detail).collect();
        assert_eq!(up_to_detail.len(), 2);
    }
}
