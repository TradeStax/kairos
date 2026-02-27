//! Volume studies: total volume, delta, cumulative delta, and OBV.
//!
//! Volume indicators analyse the number of contracts traded per period.
//! They confirm price moves (strong volume backing a breakout) or warn
//! of weak participation (rising price on declining volume).
//!
//! ## Indicators
//!
//! - **Volume** — Raw total volume per candle, colored by candle direction.
//!   The simplest measure of market participation; useful for confirming
//!   breakouts and spotting exhaustion (climactic volume at extremes).
//!
//! - **Delta** — Per-candle buy volume minus sell volume. Positive bars
//!   indicate net buying pressure; negative bars indicate net selling.
//!   Helps reveal the aggressor side behind each candle's move.
//!
//! - **CVD** (Cumulative Volume Delta) — Running sum of per-candle delta.
//!   Divergences between CVD and price highlight weakening demand or
//!   supply. Supports optional daily or weekly resets to isolate
//!   intraday order flow patterns.
//!
//! - **OBV** (On Balance Volume) — Classic cumulative indicator that adds
//!   total volume on up-closes and subtracts it on down-closes.
//!   Divergences between OBV and price often precede trend reversals.
//!
//! All four studies render in a separate panel below the price chart.

mod basic;
pub mod cvd;
pub mod delta;
pub mod obv;

pub use basic::VolumeStudy;
pub use cvd::CvdStudy;
pub use delta::DeltaStudy;
pub use obv::ObvStudy;
