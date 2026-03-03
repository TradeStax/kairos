//! Equity curve and daily snapshot tracking.
//!
//! Records the portfolio's equity over time at two granularities:
//!
//! - **Tick-level** via [`EquityCurve`] -- every sample captured by
//!   the engine (typically per-bar or per-fill).
//! - **Daily** via [`DailyEquityTracker`] -- one snapshot per
//!   calendar day, useful for computing daily returns and Sharpe.

use kairos_data::Timestamp;
use serde::{Deserialize, Serialize};

/// Milliseconds in one UTC calendar day.
const MS_PER_DAY: u64 = 86_400_000;

/// A single sample point on the equity curve.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EquityPoint {
    /// Timestamp of this sample (milliseconds since epoch).
    pub timestamp: Timestamp,
    /// Realized equity from closed trades only (cash balance).
    pub realized_equity_usd: f64,
    /// Unrealized PnL from open positions at mark-to-market.
    pub unrealized_pnl_usd: f64,
    /// Total equity = realized + unrealized.
    pub total_equity_usd: f64,
}

/// Full equity curve for a backtest run.
///
/// Stores an ordered sequence of [`EquityPoint`] samples that
/// represent the portfolio's value over time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EquityCurve {
    /// Time-ordered equity samples.
    pub points: Vec<EquityPoint>,
    /// Starting equity for this backtest run (USD).
    pub initial_equity_usd: f64,
}

impl EquityCurve {
    /// Create a new empty equity curve with the given starting
    /// equity.
    #[must_use]
    pub fn new(initial_equity_usd: f64) -> Self {
        Self {
            points: Vec::new(),
            initial_equity_usd,
        }
    }

    /// Record a new equity sample.
    ///
    /// `realized` is the cash balance (initial equity + closed-trade
    /// PnL - commissions). `unrealized` is the mark-to-market PnL of
    /// open positions.
    pub fn record(&mut self, timestamp: Timestamp, realized: f64, unrealized: f64) {
        self.points.push(EquityPoint {
            timestamp,
            realized_equity_usd: realized,
            unrealized_pnl_usd: unrealized,
            total_equity_usd: realized + unrealized,
        });
    }

    /// Current realized equity (last recorded point, or initial if
    /// no samples yet).
    #[must_use]
    pub fn current_realized(&self) -> f64 {
        self.points
            .last()
            .map(|p| p.realized_equity_usd)
            .unwrap_or(self.initial_equity_usd)
    }
}

/// Daily equity snapshot for end-of-day tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailySnapshot {
    /// Start-of-day timestamp (milliseconds, midnight UTC).
    pub date_ms: u64,
    /// Total equity at the time of the snapshot.
    pub equity: f64,
    /// Cumulative realized PnL at end of day.
    pub realized_pnl: f64,
    /// Number of open positions at end of day.
    pub positions_count: usize,
}

/// Tracks daily equity snapshots for analytics.
///
/// Records at most one snapshot per calendar day (UTC). The day
/// boundary is determined by integer-dividing the timestamp by
/// 86,400,000 ms.
pub struct DailyEquityTracker {
    /// Collected daily snapshots in chronological order.
    snapshots: Vec<DailySnapshot>,
    /// Day index of the most recent snapshot (timestamp_ms / 86.4M).
    last_day: Option<u64>,
}

impl DailyEquityTracker {
    /// Create a new empty tracker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            snapshots: Vec::new(),
            last_day: None,
        }
    }

    /// Record a snapshot if a new calendar day (UTC) has started.
    ///
    /// The day boundary is `timestamp_ms / 86_400_000`. If the
    /// current timestamp falls on the same day as the last recorded
    /// snapshot, this is a no-op.
    pub fn maybe_record(
        &mut self,
        timestamp_ms: u64,
        equity: f64,
        realized_pnl: f64,
        positions_count: usize,
    ) {
        let day = timestamp_ms / MS_PER_DAY;
        if self.last_day != Some(day) {
            self.last_day = Some(day);
            self.snapshots.push(DailySnapshot {
                date_ms: day * MS_PER_DAY,
                equity,
                realized_pnl,
                positions_count,
            });
        }
    }

    /// All recorded daily snapshots in chronological order.
    #[must_use]
    pub fn snapshots(&self) -> &[DailySnapshot] {
        &self.snapshots
    }

    /// Clear all snapshots and reset the day tracker.
    pub fn reset(&mut self) {
        self.snapshots.clear();
        self.last_day = None;
    }
}

impl Default for DailyEquityTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kairos_data::Timestamp;

    // ── EquityCurve ──────────────────────────────────────────────

    #[test]
    fn test_equity_curve_new() {
        let curve = EquityCurve::new(100_000.0);
        assert!((curve.initial_equity_usd - 100_000.0).abs() < 1e-10);
        assert!(curve.points.is_empty());
        assert!((curve.current_realized() - 100_000.0).abs() < 1e-10);
    }

    #[test]
    fn test_equity_curve_record() {
        let mut curve = EquityCurve::new(100_000.0);
        curve.record(Timestamp(1000), 100_500.0, 200.0);

        assert_eq!(curve.points.len(), 1);
        let p = &curve.points[0];
        assert!((p.realized_equity_usd - 100_500.0).abs() < 1e-10);
        assert!((p.unrealized_pnl_usd - 200.0).abs() < 1e-10);
        assert!((p.total_equity_usd - 100_700.0).abs() < 1e-10);
    }

    #[test]
    fn test_equity_curve_current_realized() {
        let mut curve = EquityCurve::new(100_000.0);
        curve.record(Timestamp(1000), 100_500.0, 0.0);
        curve.record(Timestamp(2000), 101_000.0, 0.0);

        assert!((curve.current_realized() - 101_000.0).abs() < 1e-10);
    }

    #[test]
    fn test_equity_curve_multiple_points() {
        let mut curve = EquityCurve::new(50_000.0);
        curve.record(Timestamp(1000), 51_000.0, 100.0);
        curve.record(Timestamp(2000), 50_500.0, -200.0);
        curve.record(Timestamp(3000), 52_000.0, 500.0);

        assert_eq!(curve.points.len(), 3);
        assert!((curve.points[2].total_equity_usd - 52_500.0).abs() < 1e-10);
    }

    // ── DailyEquityTracker ───────────────────────────────────────

    #[test]
    fn test_daily_tracker_new() {
        let tracker = DailyEquityTracker::new();
        assert!(tracker.snapshots().is_empty());
    }

    #[test]
    fn test_daily_tracker_records_new_day() {
        let mut tracker = DailyEquityTracker::new();
        tracker.maybe_record(86_400_000, 100_000.0, 0.0, 0);

        assert_eq!(tracker.snapshots().len(), 1);
        assert!((tracker.snapshots()[0].equity - 100_000.0).abs() < 1e-10);
    }

    #[test]
    fn test_daily_tracker_skips_same_day() {
        let mut tracker = DailyEquityTracker::new();
        let day1_start = 86_400_000_u64;
        tracker.maybe_record(day1_start, 100_000.0, 0.0, 0);
        // Same day, different timestamp
        tracker.maybe_record(day1_start + 1000, 100_500.0, 500.0, 1);

        // Only one snapshot (first call for the day wins)
        assert_eq!(tracker.snapshots().len(), 1);
        assert!((tracker.snapshots()[0].equity - 100_000.0).abs() < 1e-10);
    }

    #[test]
    fn test_daily_tracker_records_different_days() {
        let mut tracker = DailyEquityTracker::new();
        let day = 86_400_000_u64;
        tracker.maybe_record(day, 100_000.0, 0.0, 0);
        tracker.maybe_record(day * 2, 101_000.0, 1_000.0, 1);
        tracker.maybe_record(day * 3, 100_500.0, 500.0, 0);

        assert_eq!(tracker.snapshots().len(), 3);
    }

    #[test]
    fn test_daily_tracker_reset() {
        let mut tracker = DailyEquityTracker::new();
        tracker.maybe_record(86_400_000, 100_000.0, 0.0, 0);
        tracker.maybe_record(86_400_000 * 2, 101_000.0, 1_000.0, 0);

        tracker.reset();

        assert!(tracker.snapshots().is_empty());
        // Should be able to record the same day again
        tracker.maybe_record(86_400_000, 99_000.0, 0.0, 0);
        assert_eq!(tracker.snapshots().len(), 1);
    }

    #[test]
    fn test_daily_tracker_date_ms_is_midnight() {
        let mut tracker = DailyEquityTracker::new();
        // Timestamp is mid-day on day 1
        let mid_day = 86_400_000 + 43_200_000; // 1.5 days
        tracker.maybe_record(mid_day, 100_000.0, 0.0, 0);

        // date_ms should be midnight of that day
        let snap = &tracker.snapshots()[0];
        assert_eq!(snap.date_ms, 86_400_000);
    }
}
