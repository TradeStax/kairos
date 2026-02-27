//! Order flow studies for volume profile analysis and trade visualization.
//!
//! Provides studies that analyze market microstructure:
//! - [`VbpStudy`] -- horizontal volume-by-price profiles with POC, value
//!   area, HVN/LVN detection, and anchored VWAP
//! - [`FootprintStudy`] -- per-candle price-level trade data (buy/sell
//!   volume grid) in Box or Profile render modes
//! - [`BigTradesStudy`] -- highlights unusually large trades on the chart,
//!   with optional absorption detection
//! - [`ImbalanceStudy`] -- detects bid/ask imbalances at individual price
//!   levels

pub mod big_trades;
pub mod footprint;
pub mod imbalance;
pub mod level_analyzer;
pub mod speed_of_tape;
pub mod vbp;

pub use big_trades::BigTradesStudy;
pub use footprint::FootprintStudy;
pub use imbalance::ImbalanceStudy;
pub use level_analyzer::LevelAnalyzerStudy;
pub use speed_of_tape::SpeedOfTapeStudy;
pub use vbp::VbpStudy;
