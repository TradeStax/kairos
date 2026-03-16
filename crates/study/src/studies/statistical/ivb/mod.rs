//! IVB (Initial Value Balance) study.
//!
//! Computes statistical projections from historical opening range
//! breakout data. Renders protection, average, and projection levels
//! on the chart with probability zones.

pub mod conditional;
pub mod distributions;
mod levels;
mod params;
pub mod session_record;

#[cfg(test)]
mod tests;

use std::any::Any;

use crate::config::{ParameterDef, StudyConfig};
use crate::core::{
    Study, StudyCapabilities, StudyCategory, StudyInput, StudyMetadata, StudyPlacement, StudyResult,
};
use crate::error::StudyError;
use crate::output::StudyOutput;

use conditional::ConditionalFilter;
use distributions::{
    EmpiricalDistribution, IvbDistribution, WeightedEmpiricalDistribution, exponential_weights,
};
use levels::{EntryIntel, IvbLevelSet};
use session_record::{IvbSessionRecord, SessionDatabase, build_session_records};

/// IVB study — statistical opening range projections.
pub struct IvbStudy {
    metadata: StudyMetadata,
    params: Vec<ParameterDef>,
    config: StudyConfig,
    output: StudyOutput,
    session_records: Vec<IvbSessionRecord>,
    external_records: Vec<IvbSessionRecord>,
    current_levels: Option<IvbLevelSet>,
    last_candle_count: usize,
}

impl IvbStudy {
    pub fn new() -> Self {
        let params = params::build_parameter_defs();
        let config = StudyConfig::from_params("ivb", &params);

        Self {
            metadata: StudyMetadata {
                name: "IVB (Opening Range)".into(),
                category: StudyCategory::Volume,
                placement: StudyPlacement::Overlay,
                description: "Statistical opening range projections \
                     with conditional filtering"
                    .into(),
                config_version: 1,
                capabilities: StudyCapabilities {
                    needs_visible_range: true,
                    interactive: true,
                    ..StudyCapabilities::default()
                },
            },
            params,
            config,
            output: StudyOutput::Empty,
            session_records: Vec::new(),
            external_records: Vec::new(),
            current_levels: None,
            last_candle_count: 0,
        }
    }

    /// Export session records as a serializable database.
    pub fn export_session_database(&self, instrument: &str) -> SessionDatabase {
        let or_minutes = self
            .config
            .get_choice("or_window_minutes", "30")
            .parse::<u32>()
            .unwrap_or(30);
        let last_date = self
            .session_records
            .last()
            .map(|r| r.date.clone())
            .unwrap_or_default();
        SessionDatabase {
            version: 1,
            instrument: instrument.to_string(),
            or_window_minutes: or_minutes,
            records: self.session_records.clone(),
            last_date,
        }
    }
}

impl Default for IvbStudy {
    fn default() -> Self {
        Self::new()
    }
}

impl Study for IvbStudy {
    fn id(&self) -> &str {
        "ivb"
    }

    fn metadata(&self) -> &StudyMetadata {
        &self.metadata
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

    fn set_parameter(
        &mut self,
        key: &str,
        value: crate::config::ParameterValue,
    ) -> Result<(), StudyError> {
        let params = self.parameters();
        let def =
            params
                .iter()
                .find(|p| p.key == key)
                .ok_or_else(|| StudyError::InvalidParameter {
                    key: key.to_string(),
                    reason: "unknown parameter".to_string(),
                })?;
        def.validate(&value)
            .map_err(|reason| StudyError::InvalidParameter {
                key: key.to_string(),
                reason,
            })?;
        self.config.set(key, value);
        self.last_candle_count = 0; // force recompute
        Ok(())
    }

    fn compute(&mut self, input: &StudyInput) -> Result<StudyResult, StudyError> {
        if input.candles.is_empty() {
            self.output = StudyOutput::Empty;
            return Ok(StudyResult::unchanged());
        }

        // Skip if candle count hasn't changed
        if input.candles.len() == self.last_candle_count {
            return Ok(StudyResult::unchanged());
        }
        self.last_candle_count = input.candles.len();

        let or_minutes = self
            .config
            .get_choice("or_window_minutes", "30")
            .parse::<u32>()
            .unwrap_or(30);
        let min_samples = self.config.get_int("min_samples", 30) as usize;
        let recent_window = self.config.get_int("recent_window", 50) as usize;
        let use_conditional = self.config.get_bool("use_conditional", true);
        let min_extension = self.config.get_float("min_extension", 0.1);
        let decay_rate = self.config.get_float("decay_rate", 0.03);
        let tick_size_units = input.tick_size.units();

        // Extract sessions and build historical records
        let sessions = crate::util::session::extract_sessions(input.candles, or_minutes);
        let mut live_records = build_session_records(&sessions, input.candles, or_minutes);

        // Merge external records whose dates aren't covered by live data
        let live_dates: std::collections::HashSet<String> =
            live_records.iter().map(|r| r.date.clone()).collect();
        let extras: Vec<_> = self
            .external_records
            .iter()
            .filter(|ext| !live_dates.contains(&ext.date))
            .cloned()
            .collect();
        live_records.extend(extras);
        live_records.sort_by(|a, b| a.date.cmp(&b.date));
        self.session_records = live_records;

        // Find current (incomplete) RTH session
        let current_rth = sessions.iter().rev().find(|s| {
            s.key.session_type == crate::util::session::SessionType::Rth && !s.is_complete
        });

        let max_sessions = self.config.get_int("max_sessions", 1) as usize;

        // Use recent records only
        let records: Vec<&IvbSessionRecord> = self
            .session_records
            .iter()
            .rev()
            .take(recent_window)
            .collect();

        if records.is_empty() {
            self.output = StudyOutput::Empty;
            self.current_levels = None;
            return Ok(StudyResult::ok());
        }

        // ── Conditional filtering ────────────────────────────
        // Requires an active RTH session with OR data. When
        // none exists (market closed), use all records.
        let has_current = current_rth.is_some()
            && current_rth.unwrap().or_high_units.is_some()
            && current_rth.unwrap().or_low_units.is_some();

        let (filtered, filters_applied, or_high, or_low): (
            Vec<&IvbSessionRecord>,
            Vec<String>,
            Option<i64>,
            Option<i64>,
        ) = if has_current {
            let current = current_rth.unwrap();
            let orh = current.or_high_units.unwrap();
            let orl = current.or_low_units.unwrap();
            let or_range = orh - orl;

            if or_range <= 0 {
                self.output = StudyOutput::Empty;
                self.current_levels = None;
                return Ok(StudyResult::ok());
            }

            let (f, fa) = if use_conditional {
                let narrow_pct = self.config.get_int("range_narrow_pct", 25) as f64 / 100.0;
                let wide_pct = self.config.get_int("range_wide_pct", 75) as f64 / 100.0;
                let gap_threshold_pct = self.config.get_float("gap_threshold_pct", 0.5);

                let or_duration_ms = u64::from(or_minutes) * 60 * 1000;
                let or_end_time = current.open_time + or_duration_ms;
                let (start_ci, end_ci) = current.candle_range;
                let range_end = (end_ci + 1).min(input.candles.len());

                let secs = (current.open_time / 1000) as i64;
                let days = secs.div_euclid(86400);
                let current_dow = ((days + 3) % 7) as u8;

                let mut first_high_time = u64::MAX;
                let mut first_low_time = u64::MAX;
                for c in &input.candles[start_ci..range_end] {
                    if c.time.0 >= or_end_time {
                        break;
                    }
                    if c.high.units() == orh && first_high_time == u64::MAX {
                        first_high_time = c.time.0;
                    }
                    if c.low.units() == orl && first_low_time == u64::MAX {
                        first_low_time = c.time.0;
                    }
                }
                let current_high_first =
                    if first_high_time != u64::MAX || first_low_time != u64::MAX {
                        Some(first_high_time <= first_low_time)
                    } else {
                        None
                    };

                let prior_rth_close = sessions
                    .iter()
                    .rev()
                    .find(|s| {
                        s.key.session_type == crate::util::session::SessionType::Rth
                            && s.is_complete
                    })
                    .map(|s| s.close_units);
                let current_overnight_gap = prior_rth_close.map(|pc| current.open_units - pc);

                let current_session_range: Option<i64> = None;

                let filter = ConditionalFilter::from_current_session(
                    or_range,
                    &records,
                    narrow_pct,
                    wide_pct,
                    Some(current_dow),
                    current_high_first,
                    current_overnight_gap,
                    current_session_range,
                    gap_threshold_pct,
                );
                filter.apply(&records, min_samples)
            } else {
                (records.clone(), Vec::new())
            };
            (f, fa, Some(orh), Some(orl))
        } else {
            // No current session — use all records
            (records.clone(), Vec::new(), None, None)
        };

        if filtered.is_empty() {
            self.output = StudyOutput::Empty;
            self.current_levels = None;
            return Ok(StudyResult::ok());
        }

        // ── Build distributions ──────────────────────────────
        let weights = exponential_weights(records.len(), decay_rate, true);
        let weight_map: std::collections::HashMap<&str, f64> = records
            .iter()
            .zip(weights.iter())
            .map(|(r, &w)| (r.date.as_str(), w))
            .collect();

        let use_weighted = decay_rate > 0.0;

        let up_entries: Vec<(f64, f64)> = filtered
            .iter()
            .filter(|r| r.broke_high)
            .map(|r| {
                let w = weight_map.get(r.date.as_str()).copied().unwrap_or(1.0);
                (r.extension_above_ratio, w)
            })
            .collect();
        let down_entries: Vec<(f64, f64)> = filtered
            .iter()
            .filter(|r| r.broke_low)
            .map(|r| {
                let w = weight_map.get(r.date.as_str()).copied().unwrap_or(1.0);
                (r.extension_below_ratio, w)
            })
            .collect();

        type DistBox = Option<Box<dyn IvbDistribution>>;
        let (up_dist, down_dist): (DistBox, DistBox) = if use_weighted {
            let up =
                WeightedEmpiricalDistribution::from_weighted_ratios(&up_entries, min_extension)
                    .map(|d| Box::new(d) as Box<dyn IvbDistribution>);
            let dn =
                WeightedEmpiricalDistribution::from_weighted_ratios(&down_entries, min_extension)
                    .map(|d| Box::new(d) as Box<dyn IvbDistribution>);
            (up, dn)
        } else {
            let up_ratios: Vec<f64> = up_entries.iter().map(|(v, _)| *v).collect();
            let dn_ratios: Vec<f64> = down_entries.iter().map(|(v, _)| *v).collect();
            let up = EmpiricalDistribution::from_ratios(&up_ratios, min_extension)
                .map(|d| Box::new(d) as Box<dyn IvbDistribution>);
            let dn = EmpiricalDistribution::from_ratios(&dn_ratios, min_extension)
                .map(|d| Box::new(d) as Box<dyn IvbDistribution>);
            (up, dn)
        };

        // ── Current session output ───────────────────────────
        let current_output = if let (Some(orh), Some(orl)) = (or_high, or_low) {
            let current = current_rth.unwrap();
            let or_range = orh - orl;
            let or_high_f64 = data::Price::from_units(orh).to_f64();
            let or_low_f64 = data::Price::from_units(orl).to_f64();
            let or_range_f64 = data::Price::from_units(or_range).to_f64();
            let or_mid_f64 = (or_high_f64 + or_low_f64) / 2.0;

            let up_breakout_rate =
                filtered.iter().filter(|r| r.broke_high).count() as f64 / filtered.len() as f64;
            let down_breakout_rate =
                filtered.iter().filter(|r| r.broke_low).count() as f64 / filtered.len() as f64;
            let no_breakout_count = filtered
                .iter()
                .filter(|r| !r.broke_high && !r.broke_low)
                .count();
            let no_breakout_rate = no_breakout_count as f64 / filtered.len() as f64;

            // Compute OR close
            let or_duration_ms = u64::from(or_minutes) * 60 * 1000;
            let or_end_time = current.open_time + or_duration_ms;
            let (start_ci, end_ci) = current.candle_range;
            let range_end = (end_ci + 1).min(input.candles.len());
            let mut or_close_units = current.open_units;
            for c in &input.candles[start_ci..range_end] {
                if c.time.0 >= or_end_time {
                    break;
                }
                or_close_units = c.close.units();
            }
            let or_close_f64 = data::Price::from_units(or_close_units).to_f64();

            // high_formed_first
            let mut fht = u64::MAX;
            let mut flt = u64::MAX;
            for c in &input.candles[start_ci..range_end] {
                if c.time.0 >= or_end_time {
                    break;
                }
                if c.high.units() == orh && fht == u64::MAX {
                    fht = c.time.0;
                }
                if c.low.units() == orl && flt == u64::MAX {
                    flt = c.time.0;
                }
            }
            let high_formed_first = fht <= flt;

            let bias = levels::compute_bias(
                or_close_f64,
                or_mid_f64,
                or_range_f64,
                high_formed_first,
                up_breakout_rate,
                down_breakout_rate,
            );

            let session_high = current.high_units;
            let session_low = current.low_units;
            let broke_high = session_high > orh;
            let broke_low = session_low < orl;
            let breakout_state = match (broke_high, broke_low) {
                (true, true) => levels::BreakoutState::BrokeBoth,
                (true, false) => levels::BreakoutState::BrokeHigh,
                (false, true) => levels::BreakoutState::BrokeLow,
                (false, false) => levels::BreakoutState::Forming,
            };

            let up_breakers: Vec<_> = filtered.iter().filter(|r| r.broke_high).collect();
            let down_breakers: Vec<_> = filtered.iter().filter(|r| r.broke_low).collect();

            let entry_intel = if !up_breakers.is_empty() || !down_breakers.is_empty() {
                Some(EntryIntel {
                    up_retest_rate: safe_rate(&up_breakers, |r| r.retraced_to_mid_after_high_break),
                    down_retest_rate: safe_rate(&down_breakers, |r| {
                        r.retraced_to_mid_after_low_break
                    }),
                    up_close_confirm_rate: safe_rate(&up_breakers, |r| {
                        r.session_close_above_or_high
                    }),
                    down_close_confirm_rate: safe_rate(&down_breakers, |r| {
                        r.session_close_below_or_low
                    }),
                    avg_time_to_max_above_hrs: avg_opt_time(&up_breakers, |r| r.time_to_max_above),
                    avg_time_to_max_below_hrs: avg_opt_time(&down_breakers, |r| {
                        r.time_to_max_below
                    }),
                })
            } else {
                None
            };

            let bias_label = match bias {
                levels::Bias::Bullish if up_breakout_rate > down_breakout_rate + 0.05 => {
                    format!(
                        "Bullish · {:.0}% up vs {:.0}% dn",
                        up_breakout_rate * 100.0,
                        down_breakout_rate * 100.0,
                    )
                }
                levels::Bias::Bullish => "Bullish".into(),
                levels::Bias::Bearish if down_breakout_rate > up_breakout_rate + 0.05 => {
                    format!(
                        "Bearish · {:.0}% dn vs {:.0}% up",
                        down_breakout_rate * 100.0,
                        up_breakout_rate * 100.0,
                    )
                }
                levels::Bias::Bearish => "Bearish".into(),
                levels::Bias::Neutral => "Neutral".into(),
            };

            let down_partial_target = down_dist
                .as_ref()
                .map(|d| or_low_f64 - d.protection() * or_range_f64 * 0.625);

            let level_set = IvbLevelSet {
                or_high: or_high_f64,
                or_low: or_low_f64,
                or_mid: or_mid_f64,
                bias,
                breakout_state,
                up_protection: up_dist
                    .as_ref()
                    .map(|d| or_high_f64 + d.protection() * or_range_f64),
                up_average: up_dist
                    .as_ref()
                    .map(|d| or_high_f64 + d.average() * or_range_f64),
                up_projection: up_dist
                    .as_ref()
                    .map(|d| or_high_f64 + d.projection() * or_range_f64),
                down_protection: down_dist
                    .as_ref()
                    .map(|d| or_low_f64 - d.protection() * or_range_f64),
                down_average: down_dist
                    .as_ref()
                    .map(|d| or_low_f64 - d.average() * or_range_f64),
                down_projection: down_dist
                    .as_ref()
                    .map(|d| or_low_f64 - d.projection() * or_range_f64),
                up_sample_count: up_dist.as_ref().map(|d| d.sample_count()).unwrap_or(0),
                down_sample_count: down_dist.as_ref().map(|d| d.sample_count()).unwrap_or(0),
                up_breakout_rate,
                down_breakout_rate,
                no_breakout_rate,
                filters_applied,
                entry_intel,
                bias_label,
                down_partial_target,
            };

            let out = levels::to_study_output(
                &level_set,
                current.open_time,
                tick_size_units,
                &self.config,
            );
            self.current_levels = Some(level_set);
            Some(out)
        } else {
            self.current_levels = None;
            None
        };

        // ── Multi-session historical rendering ───────────────
        let up_ref = up_dist.as_ref().map(|d| d.as_ref());
        let down_ref = down_dist.as_ref().map(|d| d.as_ref());

        let hist_count = if current_output.is_some() {
            max_sessions.saturating_sub(1)
        } else {
            max_sessions
        };

        let historical: Vec<_> = if hist_count > 0 {
            sessions
                .iter()
                .rev()
                .filter(|s| {
                    s.key.session_type == crate::util::session::SessionType::Rth
                        && s.is_complete
                        && s.or_high_units.is_some()
                        && s.or_low_units.is_some()
                })
                .take(hist_count)
                .collect()
        } else {
            Vec::new()
        };

        let mut parts = Vec::new();
        if let Some(out) = current_output {
            parts.push(out);
        }
        for hist in &historical {
            let h_out =
                levels::to_historical_output(hist, up_ref, down_ref, tick_size_units, &self.config);
            if !matches!(h_out, StudyOutput::Empty) {
                parts.push(h_out);
            }
        }

        self.output = match parts.len() {
            0 => StudyOutput::Empty,
            1 => parts.into_iter().next().unwrap(),
            _ => StudyOutput::Composite(parts),
        };

        Ok(StudyResult::ok())
    }

    fn output(&self) -> &StudyOutput {
        &self.output
    }

    fn reset(&mut self) {
        self.output = StudyOutput::Empty;
        self.session_records.clear();
        self.external_records.clear();
        self.current_levels = None;
        self.last_candle_count = 0;
    }

    fn interactive_data(&self) -> Option<&dyn Any> {
        self.current_levels.as_ref().map(|l| l as &dyn Any)
    }

    fn accept_external_data(&mut self, data: Box<dyn Any + Send>) -> Result<(), StudyError> {
        let db = data
            .downcast::<SessionDatabase>()
            .map_err(|_| StudyError::InvalidParameter {
                key: "external_data".into(),
                reason: "expected SessionDatabase".into(),
            })?;

        // Store as external records (deduplicated)
        let existing_dates: std::collections::HashSet<String> = self
            .external_records
            .iter()
            .map(|r| r.date.clone())
            .collect();

        for record in db.records {
            if !existing_dates.contains(&record.date) {
                self.external_records.push(record);
            }
        }

        self.external_records.sort_by(|a, b| a.date.cmp(&b.date));

        // Force recompute on next call
        self.last_candle_count = 0;

        Ok(())
    }

    fn clone_study(&self) -> Box<dyn Study> {
        let mut cloned = Self::new();
        cloned.config = self.config.clone();
        Box::new(cloned)
    }
}

// ── Bundled session databases ─────────────────────────────

struct BundledEntry {
    symbol: &'static str,
    data: &'static [u8],
}

const BUNDLED: &[BundledEntry] = &[BundledEntry {
    symbol: "NQ-c-0",
    data: include_bytes!("data/nq.bin.zst"),
}];

/// Load a bundled session database for the given symbol, if available.
pub fn load_bundled(symbol: &str) -> Option<SessionDatabase> {
    let sanitized = symbol.replace('.', "-");
    let entry = BUNDLED.iter().find(|b| b.symbol == sanitized)?;
    let decompressed = zstd::decode_all(entry.data).ok()?;
    bincode::deserialize(&decompressed).ok()
}

fn safe_rate(records: &[&&IvbSessionRecord], pred: impl Fn(&IvbSessionRecord) -> bool) -> f64 {
    if records.is_empty() {
        return 0.0;
    }
    records.iter().filter(|r| pred(r)).count() as f64 / records.len() as f64
}

fn avg_opt_time(
    records: &[&&IvbSessionRecord],
    get: impl Fn(&IvbSessionRecord) -> Option<u64>,
) -> f64 {
    let vals: Vec<f64> = records
        .iter()
        .filter_map(|r| get(r))
        .map(|t| t as f64 / 3_600_000.0)
        .collect();
    if vals.is_empty() {
        0.0
    } else {
        vals.iter().sum::<f64>() / vals.len() as f64
    }
}
