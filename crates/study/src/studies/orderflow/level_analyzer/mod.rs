//! Level Analyzer study — auto-detects key price levels and monitors
//! real-time buyer/seller interaction at each level.
//!
//! Combines volume profile analysis (HVN/LVN/POC/VA), session boundaries,
//! prior-day levels, delta zones, and opening range into a unified set of
//! monitored levels. Each level tracks touches, holds, breaks, delta, and
//! volume absorbed.
//!
//! Supports two detection modes:
//! - **Per Session**: each completed RTH/ETH session gets its own
//!   independent profile and levels. Only completed sessions produce
//!   profile-based levels.
//! - **Aggregate**: single profile from all data (legacy mode).
//!
//! Renders as background horizontal lines (`StudyOutput::Levels`) with
//! color/opacity/style reflecting level source and behavioral status.

mod detection;
mod monitoring;
pub mod params;
pub mod session;
pub mod types;

use std::any::Any;

use data::{Price, SerializableColor, Trade};

use crate::config::{
    LineStyleValue, ParameterDef, ParameterTab, StudyConfig,
};
use crate::core::{Study, StudyCategory, StudyInput, StudyPlacement};
use crate::error::StudyError;
use crate::output::{PriceLevel, StudyOutput};

use self::params::TAB_LABELS;
use self::session::SessionInfo;
use self::types::{
    LevelAnalyzerData, LevelRemoval, LevelSource, LevelStatus,
    MonitoredLevel, SessionKey,
};

/// Level Analyzer study instance.
pub struct LevelAnalyzerStudy {
    config: StudyConfig,
    output: StudyOutput,
    params: Vec<ParameterDef>,
    /// Auto-detected and manual monitored levels.
    levels: Vec<MonitoredLevel>,
    /// Next unique ID for new levels.
    next_id: u64,
    /// Number of trades already processed for incremental monitoring.
    processed_trade_count: usize,
    /// Candle fingerprint: (count, last_time) to detect data changes.
    candle_fingerprint: (usize, u64),
    /// Current ATR value for adaptive tolerance.
    current_atr: Option<f64>,
    /// Cached tolerance in price units.
    tolerance_units: i64,
    /// Cached interactive data for UI (rebuilt on each output change).
    interactive_cache: Option<Box<LevelAnalyzerData>>,
    /// Cached session info from last compute.
    cached_sessions: Vec<SessionInfo>,
    /// Cached trade ranges parallel to cached_sessions.
    cached_trade_ranges: Vec<(usize, usize)>,
}

impl LevelAnalyzerStudy {
    pub fn new() -> Self {
        let params = params::build_params();
        let mut config = StudyConfig::new("level_analyzer");
        for p in &params {
            config.set(p.key.clone(), p.default.clone());
        }

        Self {
            config,
            output: StudyOutput::Empty,
            params,
            levels: Vec::new(),
            next_id: 1,
            processed_trade_count: 0,
            candle_fingerprint: (0, 0),
            current_atr: None,
            tolerance_units: 0,
            interactive_cache: None,
            cached_sessions: Vec::new(),
            cached_trade_ranges: Vec::new(),
        }
    }

    /// Compute tolerance in price units from config + candle data.
    fn compute_tolerance(
        &mut self,
        candles: &[data::Candle],
        tick_size: Price,
    ) {
        let mode =
            self.config.get_choice("tolerance_mode", "Fixed Ticks");

        if mode == "ATR-Based" {
            let period =
                self.config.get_int("atr_period", 14) as usize;
            let multiplier =
                self.config.get_float("atr_multiplier", 0.5);

            if let Some(atr) =
                monitoring::compute_atr(candles, period)
            {
                self.current_atr = Some(atr);
                let atr_units =
                    Price::from_f64(atr * multiplier).units();
                self.tolerance_units =
                    atr_units.max(tick_size.units());
            } else {
                log::debug!(
                    "level_analyzer: ATR computation returned \
                     None (insufficient data for period {}), \
                     falling back to fixed ticks",
                    period
                );
                let ticks =
                    self.config.get_int("tolerance_ticks", 4);
                self.tolerance_units = ticks * tick_size.units();
                self.current_atr = None;
            }
        } else {
            let ticks =
                self.config.get_int("tolerance_ticks", 4);
            self.tolerance_units = ticks * tick_size.units();
            self.current_atr = None;
        }
    }

    /// Rebuild StudyOutput::Levels from current monitored levels.
    fn rebuild_output(&mut self) {
        let show_labels =
            self.config.get_bool("show_labels", true);
        let show_broken =
            self.config.get_bool("show_broken_levels", true);
        let show_touch_count =
            self.config.get_bool("show_touch_count", false);
        let show_strength =
            self.config.get_bool("show_strength", false);
        let show_zones =
            self.config.get_bool("show_zones", true);
        let base_width =
            self.config.get_float("line_width", 1.0) as f32;
        let is_per_session = self
            .config
            .get_choice("session_mode", "Per Session")
            == "Per Session";

        let zone_hw =
            if show_zones && self.tolerance_units > 0 {
                Some(
                    Price::from_units(self.tolerance_units)
                        .to_f64(),
                )
            } else {
                None
            };

        let price_levels: Vec<PriceLevel> = self
            .levels
            .iter()
            .filter(|l| {
                show_broken || l.status != LevelStatus::Broken
            })
            .map(|level| {
                let base_color =
                    self.color_for_source(level.source);
                let base_opacity = 0.8_f32;
                let opacity = base_opacity
                    * level.status.opacity_multiplier();

                let style = match level.status {
                    LevelStatus::Broken
                    | LevelStatus::Weakening => {
                        LineStyleValue::Dashed
                    }
                    _ => LineStyleValue::Solid,
                };

                let width = base_width
                    * (0.5 + 1.5 * level.strength);

                let label = if show_labels {
                    self.build_label(
                        level,
                        is_per_session,
                        show_touch_count,
                        show_strength,
                    )
                } else {
                    String::new()
                };

                // Always anchor as ray when detected_at is set
                let start_x = if level.detected_at > 0 {
                    Some(level.detected_at)
                } else {
                    None
                };

                PriceLevel {
                    price: level.price,
                    label,
                    color: base_color,
                    style,
                    opacity,
                    show_label: show_labels,
                    fill_above: None,
                    fill_below: None,
                    width,
                    start_x,
                    zone_half_width: zone_hw,
                }
            })
            .collect();

        self.output = if price_levels.is_empty() {
            StudyOutput::Empty
        } else {
            StudyOutput::Levels(price_levels)
        };

        let block_min_qty =
            self.config.get_int("block_min_qty", 25) as f64;
        let aggregation_window_ms =
            self.config.get_int("aggregation_window_ms", 40)
                as u64;

        let session_keys: Vec<SessionKey> = self
            .cached_sessions
            .iter()
            .map(|s| s.key.clone())
            .collect();

        self.interactive_cache =
            Some(Box::new(LevelAnalyzerData {
                levels: self.levels.clone(),
                tolerance_ticks: self.tolerance_units,
                current_atr: self.current_atr,
                block_threshold: block_min_qty,
                aggregation_window_ms,
                sessions: session_keys,
            }));
    }

    /// Build label string for a level.
    fn build_label(
        &self,
        level: &MonitoredLevel,
        is_per_session: bool,
        show_touch_count: bool,
        show_strength: bool,
    ) -> String {
        let mut s = String::new();

        // Session prefix for per-session mode (skip for
        // cross-session and manual levels)
        if is_per_session
            && !level.session_key.is_cross_session()
            && level.source != LevelSource::Manual
        {
            let tag = level.session_key.short_tag();
            if !tag.is_empty() {
                s.push_str(&tag);
                s.push(' ');
            }
        }

        s.push_str(level.source.label());

        if show_touch_count && level.touch_count > 0 {
            s.push_str(&format!(" ({})", level.touch_count));
        }
        if show_strength && level.strength > 0.0 {
            s.push_str(&format!(
                " {:.0}%",
                level.strength * 100.0
            ));
        }

        s
    }

    /// Get color for a level source from config.
    fn color_for_source(
        &self,
        source: LevelSource,
    ) -> SerializableColor {
        match source {
            LevelSource::Poc => self
                .config
                .get_color("poc_color", params::POC_COLOR),
            LevelSource::SessionHigh
            | LevelSource::SessionLow => self
                .config
                .get_color("session_color", params::SESSION_COLOR),
            LevelSource::PriorDayHigh
            | LevelSource::PriorDayLow
            | LevelSource::PriorDayClose => self
                .config
                .get_color(
                    "prior_day_color",
                    params::PRIOR_DAY_COLOR,
                ),
            LevelSource::Hvn => self
                .config
                .get_color("hvn_color", params::HVN_COLOR),
            LevelSource::Lvn => self
                .config
                .get_color("lvn_color", params::LVN_COLOR),
            LevelSource::Vah | LevelSource::Val => self
                .config
                .get_color(
                    "vah_val_color",
                    params::VAH_VAL_COLOR,
                ),
            LevelSource::HighDeltaZone
            | LevelSource::LowDeltaZone => self
                .config
                .get_color("delta_color", params::DELTA_COLOR),
            LevelSource::OpeningRangeHigh
            | LevelSource::OpeningRangeLow => self
                .config
                .get_color("or_color", params::OR_COLOR),
            LevelSource::Manual => self
                .config
                .get_color("manual_color", params::MANUAL_COLOR),
        }
    }
}

impl Study for LevelAnalyzerStudy {
    fn id(&self) -> &str {
        "level_analyzer"
    }

    fn name(&self) -> &str {
        "Level Analyzer"
    }

    fn category(&self) -> StudyCategory {
        StudyCategory::OrderFlow
    }

    fn placement(&self) -> StudyPlacement {
        StudyPlacement::Background
    }

    fn parameters(&self) -> &[ParameterDef] {
        &self.params
    }

    fn config(&self) -> &StudyConfig {
        &self.config
    }

    fn config_mut(&mut self) -> &mut StudyConfig {
        &mut self.config
    }

    fn compute(
        &mut self,
        input: &StudyInput,
    ) -> Result<(), StudyError> {
        let candles = input.candles;
        if candles.is_empty() {
            self.output = StudyOutput::Empty;
            return Ok(());
        }

        // Compute tolerance
        self.compute_tolerance(candles, input.tick_size);

        let session_mode = self
            .config
            .get_choice("session_mode", "Per Session")
            .to_string();

        if session_mode == "Per Session" {
            // Per-session detection
            let or_minutes = self
                .config
                .get_int("opening_range_minutes", 30)
                as u32;
            let session_types = self
                .config
                .get_choice("session_types", "RTH + ETH")
                .to_string();
            let visible_sessions = self
                .config
                .get_int("visible_sessions", 3)
                as usize;

            // Extract sessions
            let all_sessions =
                session::extract_sessions(candles, or_minutes);

            // Filter by session type preference
            let filtered_indices =
                session::filter_sessions_by_type(
                    &all_sessions,
                    &session_types,
                );
            let sessions: Vec<SessionInfo> = filtered_indices
                .iter()
                .map(|&i| all_sessions[i].clone())
                .collect();

            // Compute trade ranges
            let trade_ranges =
                if let Some(trades) = input.trades {
                    session::trade_ranges_for_sessions(
                        trades, &sessions,
                    )
                } else {
                    vec![(0, 0); sessions.len()]
                };

            // Detect levels per session
            self.levels =
                detection::detect_levels_per_session(
                    candles,
                    input.trades,
                    input.tick_size,
                    &self.config,
                    &self.levels,
                    &mut self.next_id,
                    &sessions,
                    &trade_ranges,
                    visible_sessions,
                );

            // Cache sessions
            self.cached_sessions = sessions;
            self.cached_trade_ranges = trade_ranges;
        } else {
            // Aggregate mode (legacy)
            self.levels = detection::detect_levels(
                candles,
                input.trades,
                input.tick_size,
                &self.config,
                &self.levels,
                &mut self.next_id,
            );
            self.cached_sessions.clear();
            self.cached_trade_ranges.clear();
        }

        // Process all trades for monitoring
        if let Some(trades) = input.trades {
            let break_threshold =
                self.config.get_float("break_threshold", 1.5);
            let block_window = self
                .config
                .get_int("aggregation_window_ms", 40)
                as u64;
            let block_min =
                self.config.get_int("block_min_qty", 25) as f64;
            monitoring::process_trades(
                &mut self.levels,
                trades,
                self.tolerance_units,
                break_threshold,
                block_window,
                block_min,
            );
            self.processed_trade_count = trades.len();
        } else {
            self.processed_trade_count = 0;
        }

        // Update fingerprint
        let last_time =
            candles.last().map_or(0, |c| c.time.0);
        self.candle_fingerprint = (candles.len(), last_time);

        self.rebuild_output();
        Ok(())
    }

    fn append_trades(
        &mut self,
        new_trades: &[Trade],
        input: &StudyInput,
    ) -> Result<(), StudyError> {
        let candles = input.candles;

        // Check if candle data changed (new candle arrived)
        let last_time =
            candles.last().map_or(0, |c| c.time.0);
        let new_fingerprint = (candles.len(), last_time);

        if new_fingerprint != self.candle_fingerprint {
            // Candle data changed — full recompute
            return self.compute(input);
        }

        // Only process new trades incrementally
        if new_trades.is_empty() {
            return Ok(());
        }

        self.compute_tolerance(candles, input.tick_size);

        let break_threshold =
            self.config.get_float("break_threshold", 1.5);
        let block_window = self
            .config
            .get_int("aggregation_window_ms", 40)
            as u64;
        let block_min =
            self.config.get_int("block_min_qty", 25) as f64;
        monitoring::process_trades(
            &mut self.levels,
            new_trades,
            self.tolerance_units,
            break_threshold,
            block_window,
            block_min,
        );

        self.processed_trade_count += new_trades.len();
        self.rebuild_output();
        Ok(())
    }

    fn output(&self) -> &StudyOutput {
        &self.output
    }

    fn reset(&mut self) {
        self.levels.clear();
        self.output = StudyOutput::Empty;
        self.processed_trade_count = 0;
        self.candle_fingerprint = (0, 0);
        self.current_atr = None;
        self.tolerance_units = 0;
        self.interactive_cache = None;
        self.cached_sessions.clear();
        self.cached_trade_ranges.clear();
    }

    fn tab_labels(
        &self,
    ) -> Option<&[(&'static str, ParameterTab)]> {
        Some(TAB_LABELS)
    }

    fn interactive_data(&self) -> Option<&dyn Any> {
        self.interactive_cache
            .as_ref()
            .map(|b| b.as_ref() as &dyn Any)
    }

    fn has_detail_modal(&self) -> bool {
        true
    }

    fn accept_external_data(
        &mut self,
        data: Box<dyn Any + Send>,
    ) -> Result<(), StudyError> {
        let data = match data.downcast::<MonitoredLevel>() {
            Ok(level) => {
                let already_exists = self.levels.iter().any(|l| {
                    l.source == LevelSource::Manual
                        && l.price_units == level.price_units
                });

                if already_exists {
                    return Err(StudyError::InvalidParameter {
                        key: "manual_level".into(),
                        reason: "level already exists at this \
                            price"
                            .into(),
                    });
                }

                self.levels.push(*level);
                self.rebuild_output();
                return Ok(());
            }
            Err(original) => original,
        };

        match data.downcast::<LevelRemoval>() {
            Ok(removal) => {
                self.levels.retain(|l| {
                    !(l.price_units == removal.price_units
                        && l.source == removal.source)
                });
                self.rebuild_output();
                Ok(())
            }
            Err(_) => Err(StudyError::InvalidParameter {
                key: "external_data".into(),
                reason:
                    "expected MonitoredLevel or LevelRemoval"
                        .into(),
            }),
        }
    }

    fn clone_study(&self) -> Box<dyn Study> {
        Box::new(LevelAnalyzerStudy {
            config: self.config.clone(),
            output: self.output.clone(),
            params: self.params.clone(),
            levels: self.levels.clone(),
            next_id: self.next_id,
            processed_trade_count: self.processed_trade_count,
            candle_fingerprint: self.candle_fingerprint,
            current_atr: self.current_atr,
            tolerance_units: self.tolerance_units,
            interactive_cache: None,
            cached_sessions: self.cached_sessions.clone(),
            cached_trade_ranges: self
                .cached_trade_ranges
                .clone(),
        })
    }
}
