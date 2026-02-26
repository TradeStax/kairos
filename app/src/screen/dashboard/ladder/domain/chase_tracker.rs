//! Chase Tracker - Pure Domain Logic
//!
//! Tracks consecutive price movements (chasing) with fade animation.
//! Used for visualizing aggressive market behavior on the ladder.
//!
//! Algorithm:
//! 1. Detect consecutive moves in same direction (bid up or ask down)
//! 2. Increase opacity based on consecutive count
//! 3. Fade when movement stops or reverses
//! 4. Reset when faded below minimum visibility
//!
//! All prices are in Price units (i64) for precision.

const CHASE_MIN_VISIBLE_OPACITY: f32 = 0.15;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Direction {
    Up,
    Down,
}

#[derive(Debug, Clone, Copy, Default)]
enum ChaseState {
    #[default]
    Idle,
    Chasing {
        direction: Direction,
        start_units: i64,
        end_units: i64,
        consecutive: u32,
    },
    Fading {
        direction: Direction,
        start_units: i64,
        end_units: i64, // Frozen at chase extreme
        start_consecutive: u32,
        fade_steps: u32,
    },
}

/// Chase Tracker - Detects and fades consecutive price movements
#[derive(Debug, Default, Clone)]
pub struct ChaseTracker {
    last_best: Option<i64>, // Price units
    state: ChaseState,
    last_update_ms: Option<u64>,
}

impl ChaseTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Update with current best price
    ///
    /// # Arguments
    /// * `current_best` - Current best price in units (None if no orderbook)
    /// * `is_bid` - true for bid side (chase up), false for ask side (chase down)
    /// * `now_ms` - Current timestamp in milliseconds
    /// * `max_interval_ms` - Reset if gap exceeds this (0 = no timeout)
    pub fn update(
        &mut self,
        current_best: Option<i64>,
        is_bid: bool,
        now_ms: u64,
        max_interval_ms: u64,
    ) {
        // Reset if too much time has passed
        if let Some(prev) = self.last_update_ms
            && max_interval_ms > 0
            && now_ms.saturating_sub(prev) > max_interval_ms
        {
            self.reset();
        }

        self.last_update_ms = Some(now_ms);

        let Some(current) = current_best else {
            self.reset();
            return;
        };

        if let Some(last) = self.last_best {
            let direction = if is_bid {
                Direction::Up
            } else {
                Direction::Down
            };

            let is_continue = match direction {
                Direction::Up => current > last,
                Direction::Down => current < last,
            };
            let is_reverse = match direction {
                Direction::Up => current < last,
                Direction::Down => current > last,
            };
            let is_unchanged = current == last;

            self.state = match (&self.state, is_continue, is_reverse, is_unchanged) {
                // Continue chasing - extend the chase
                (
                    ChaseState::Chasing {
                        direction: sdir,
                        start_units,
                        consecutive,
                        ..
                    },
                    true,
                    _,
                    _,
                ) if *sdir == direction => ChaseState::Chasing {
                    direction,
                    start_units: *start_units,
                    end_units: current,
                    consecutive: consecutive.saturating_add(1),
                },

                // Start new chase (from idle or fading)
                (ChaseState::Idle, true, _, _) | (ChaseState::Fading { .. }, true, _, _) => {
                    ChaseState::Chasing {
                        direction,
                        start_units: last,
                        end_units: current,
                        consecutive: 1,
                    }
                }

                // Reversal while chasing -> start fading (freeze end price)
                (
                    ChaseState::Chasing {
                        direction: sdir,
                        start_units,
                        end_units,
                        consecutive,
                    },
                    _,
                    true,
                    _,
                ) if *consecutive > 0 => ChaseState::Fading {
                    direction: *sdir,
                    start_units: *start_units,
                    end_units: *end_units, // Freeze at extreme
                    start_consecutive: *consecutive,
                    fade_steps: 0,
                },

                // Unchanged while chasing -> start fading
                (
                    ChaseState::Chasing {
                        direction: sdir,
                        start_units,
                        end_units,
                        consecutive,
                    },
                    _,
                    _,
                    true,
                ) if *consecutive > 0 => ChaseState::Fading {
                    direction: *sdir,
                    start_units: *start_units,
                    end_units: *end_units,
                    start_consecutive: *consecutive,
                    fade_steps: 0,
                },

                // Continue fading
                (
                    ChaseState::Fading {
                        direction: sdir,
                        start_units,
                        end_units,
                        start_consecutive,
                        fade_steps,
                    },
                    _,
                    _,
                    _,
                ) => ChaseState::Fading {
                    direction: *sdir,
                    start_units: *start_units,
                    end_units: *end_units,
                    start_consecutive: *start_consecutive,
                    fade_steps: fade_steps.saturating_add(1),
                },

                // Default: keep current state
                _ => self.state,
            };

            // Check if faded below minimum visibility
            if let ChaseState::Fading {
                start_consecutive,
                fade_steps,
                ..
            } = self.state
            {
                let alpha = Self::calculate_alpha(start_consecutive, fade_steps);
                if alpha < CHASE_MIN_VISIBLE_OPACITY {
                    self.state = ChaseState::Idle;
                }
            }
        }

        self.last_best = Some(current);
    }

    /// Reset to idle state
    pub fn reset(&mut self) {
        self.last_best = None;
        self.state = ChaseState::Idle;
        self.last_update_ms = None;
    }

    /// Get chase segment for rendering
    ///
    /// Returns (start_units, end_units, alpha) where alpha is 0.0-1.0
    pub fn segment(&self) -> Option<(i64, i64, f32)> {
        match self.state {
            ChaseState::Chasing {
                start_units,
                end_units,
                consecutive,
                ..
            } => {
                let alpha = Self::consecutive_to_alpha(consecutive);
                Some((start_units, end_units, alpha))
            }
            ChaseState::Fading {
                start_units,
                end_units,
                start_consecutive,
                fade_steps,
                ..
            } => {
                let alpha = Self::calculate_alpha(start_consecutive, fade_steps);
                Some((start_units, end_units, alpha))
            }
            ChaseState::Idle => None,
        }
    }

    /// Calculate alpha for fading state
    fn calculate_alpha(start_consecutive: u32, fade_steps: u32) -> f32 {
        let base = Self::consecutive_to_alpha(start_consecutive);
        base / (1.0 + fade_steps as f32)
    }

    /// Map consecutive moves to opacity (0-1)
    ///
    /// Formula: 1 - 1/(1+n) asymptotically approaches 1.0
    /// n=1: 0.5, n=2: 0.67, n=3: 0.75, n=5: 0.83, n=10: 0.91
    fn consecutive_to_alpha(n: u32) -> f32 {
        let nf = n as f32;
        1.0 - 1.0 / (1.0 + nf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_consecutive_to_alpha() {
        assert!((ChaseTracker::consecutive_to_alpha(1) - 0.5).abs() < 0.01);
        assert!((ChaseTracker::consecutive_to_alpha(2) - 0.667).abs() < 0.01);
        assert!((ChaseTracker::consecutive_to_alpha(10) - 0.909).abs() < 0.01);
    }

    #[test]
    fn test_chase_bid_up() {
        let mut tracker = ChaseTracker::new();

        // Start at 100
        tracker.update(Some(100), true, 1000, 0);
        assert!(tracker.segment().is_none()); // No chase yet

        // Move to 101 (chase starts)
        tracker.update(Some(101), true, 1100, 0);
        let seg = tracker.segment().unwrap();
        assert_eq!(seg.0, 100); // Start
        assert_eq!(seg.1, 101); // End
        assert!((seg.2 - 0.5).abs() < 0.01); // Alpha for 1 consecutive

        // Move to 102 (chase continues)
        tracker.update(Some(102), true, 1200, 0);
        let seg = tracker.segment().unwrap();
        assert_eq!(seg.0, 100);
        assert_eq!(seg.1, 102);
        assert!((seg.2 - 0.667).abs() < 0.01); // Alpha for 2 consecutive

        // Stay at 102 (start fading)
        tracker.update(Some(102), true, 1300, 0);
        let seg = tracker.segment().unwrap();
        assert_eq!(seg.1, 102); // End frozen
        assert!(seg.2 < 0.667); // Alpha decreasing
    }

    #[test]
    fn test_chase_reset_on_timeout() {
        let mut tracker = ChaseTracker::new();

        tracker.update(Some(100), true, 1000, 200); // 200ms timeout
        tracker.update(Some(101), true, 1100, 200); // Chase starts

        assert!(tracker.segment().is_some());

        // Timeout exceeded
        tracker.update(Some(102), true, 1500, 200); // 400ms gap > 200ms timeout

        // Should have reset (small chase, not a long one)
        if let Some((start, end, _)) = tracker.segment() {
            // If not reset, it's a new chase from 101->102
            assert!(start >= 101 && end >= 101);
        }
    }

    #[test]
    fn test_no_chase_when_unchanged() {
        let mut tracker = ChaseTracker::new();

        tracker.update(Some(100), true, 1000, 0);
        tracker.update(Some(100), true, 1100, 0);
        tracker.update(Some(100), true, 1200, 0);

        assert!(tracker.segment().is_none()); // No movement, no chase
    }
}
