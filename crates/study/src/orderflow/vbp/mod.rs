//! Volume-by-Price (VBP) Study
//!
//! Renders horizontal volume distribution bars at each price level
//! on the chart background, supporting 5 visualization types,
//! configurable time periods, POC/Value Area overlays, and full
//! color/style customization.
//!
//! Integrated features: POC line, developing POC, value area
//! lines/fill, HVN/LVN detection, and anchored VWAP with
//! standard deviation bands.
//!
//! Split into focused submodules:
//! - [`params`] -- parameter definitions and default colors
//! - [`compute`] -- core computation and helpers
//! - [`config`] -- config import/export and builder helpers

mod compute;
mod config;
pub(crate) mod params;

use crate::config::{
    ParameterDef, ParameterTab, ParameterValue, StudyConfig,
};
use crate::error::StudyError;
use crate::orderflow::profile_core;
use crate::output::{
    ProfileOutput, ProfileRenderConfig, StudyOutput,
    VbpGroupingMode, VbpPeriod,
};
use crate::traits::{Study, StudyCategory, StudyInput, StudyPlacement};

use params::*;

pub struct VbpStudy {
    pub(super) config: StudyConfig,
    output: StudyOutput,
    pub(super) params: Vec<ParameterDef>,
    /// Fingerprint of the last computed input to skip redundant
    /// recomputation when the underlying data hasn't changed.
    /// (candle_count, first_candle_ts, last_candle_ts,
    /// trade_count)
    pub(super) last_input_fingerprint: (usize, u64, u64, usize),
    /// Cached visible range from the last full computation.
    /// Skips recompute when pan hasn't moved >25% of the span.
    pub(super) last_stable_range: Option<(u64, u64)>,
}

impl VbpStudy {
    pub fn new() -> Self {
        let params = params::build_params();

        let mut config = StudyConfig::new("vbp");
        for p in &params {
            config.set(p.key.clone(), p.default.clone());
        }

        Self {
            config,
            output: StudyOutput::Empty,
            params,
            last_input_fingerprint: (0, 0, 0, 0),
            last_stable_range: None,
        }
    }

    /// Create a VbpStudy pre-configured for a fixed time range.
    pub fn for_range(start_ms: u64, end_ms: u64) -> Self {
        let mut study = Self::new();
        let _ = study.set_parameter(
            "period",
            ParameterValue::Choice("Custom".into()),
        );
        let _ = study.set_parameter(
            "custom_start",
            ParameterValue::Integer(start_ms as i64),
        );
        let _ = study.set_parameter(
            "custom_end",
            ParameterValue::Integer(end_ms as i64),
        );
        study
    }

    /// Update the time range and reset fingerprints for
    /// recomputation.
    ///
    /// Also forces `period` to Custom so that `import_config()`
    /// (which may set a different period) doesn't override the
    /// drawing's anchor points.
    pub fn set_range(&mut self, start_ms: u64, end_ms: u64) {
        let _ = self.set_parameter(
            "period",
            ParameterValue::Choice("Custom".into()),
        );
        let _ = self.set_parameter(
            "custom_start",
            ParameterValue::Integer(start_ms as i64),
        );
        let _ = self.set_parameter(
            "custom_end",
            ParameterValue::Integer(end_ms as i64),
        );
        self.last_input_fingerprint = (0, 0, 0, 0);
        self.last_stable_range = None;
    }
}

impl Default for VbpStudy {
    fn default() -> Self {
        Self::new()
    }
}

impl Study for VbpStudy {
    fn id(&self) -> &str {
        "vbp"
    }

    fn name(&self) -> &str {
        "Volume by Price"
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

    fn set_parameter(
        &mut self,
        key: &str,
        value: ParameterValue,
    ) -> Result<(), StudyError> {
        let params = self.parameters();
        let def = params
            .iter()
            .find(|p| p.key == key)
            .ok_or_else(|| StudyError::InvalidParameter {
                key: key.to_string(),
                reason: "unknown parameter".to_string(),
            })?;
        def.validate(&value).map_err(|reason| {
            StudyError::InvalidParameter {
                key: key.to_string(),
                reason,
            }
        })?;
        self.config.set(key, value);
        // Invalidate fingerprint so next compute() runs fully
        self.last_input_fingerprint = (0, 0, 0, 0);
        self.last_stable_range = None;
        Ok(())
    }

    fn tab_labels(
        &self,
    ) -> Option<&[(&'static str, ParameterTab)]> {
        static LABELS: &[(&str, ParameterTab)] = &[
            ("Data", ParameterTab::Parameters),
            ("Style", ParameterTab::Style),
            ("POC", ParameterTab::Tab4),
            ("Value Area", ParameterTab::Tab5),
            ("Peak & Valley", ParameterTab::Tab6),
            ("VWAP", ParameterTab::Tab7),
        ];
        Some(LABELS)
    }

    fn compute(
        &mut self,
        input: &StudyInput,
    ) -> Result<(), StudyError> {
        if input.candles.is_empty() {
            self.output = StudyOutput::Empty;
            return Ok(());
        }

        // Stable-range check for Auto period: skip recompute
        // if visible range hasn't moved >25% of previous span.
        let period = Self::parse_period(
            self.config.get_choice("period", "Auto"),
        );
        if matches!(period, VbpPeriod::Auto) {
            if let Some((start, end)) = input.visible_range {
                if let Some((prev_start, prev_end)) =
                    self.last_stable_range
                {
                    let prev_span =
                        prev_end.saturating_sub(prev_start);
                    let threshold = prev_span / 4;
                    let start_shift = (start as i64
                        - prev_start as i64)
                        .unsigned_abs();
                    let end_shift = (end as i64
                        - prev_end as i64)
                        .unsigned_abs();
                    if start_shift < threshold
                        && end_shift < threshold
                        && !matches!(
                            self.output,
                            StudyOutput::Empty
                        )
                    {
                        return Ok(());
                    }
                }
                self.last_stable_range = Some((start, end));
            }
        }

        let candle_slice =
            self.resolve_candle_range(input.candles, input);

        if candle_slice.is_empty() {
            self.output = StudyOutput::Empty;
            return Ok(());
        }

        // Filter trades once for both fingerprint and profile.
        let filtered_trades = input
            .trades
            .map(|t| Self::filter_trades(t, candle_slice));
        let trade_count =
            filtered_trades.map(|t| t.len()).unwrap_or(0);

        // Build a fingerprint of the actual input data.
        let first_ts = candle_slice
            .first()
            .map(|c| c.time.to_millis())
            .unwrap_or(0);
        let last_ts = candle_slice
            .last()
            .map(|c| c.time.to_millis())
            .unwrap_or(0);
        let fingerprint = (
            candle_slice.len(),
            first_ts,
            last_ts,
            trade_count,
        );

        if fingerprint == self.last_input_fingerprint
            && !matches!(self.output, StudyOutput::Empty)
        {
            return Ok(());
        }
        self.last_input_fingerprint = fingerprint;

        // Tick grouping
        let tick_units = input.tick_size.units().max(1);
        let is_automatic = self
            .config
            .get_choice("auto_grouping", "Automatic")
            != "Manual";

        let group_quantum = if is_automatic {
            tick_units
        } else {
            let manual = self
                .config
                .get_int("manual_ticks", 1)
                .max(1);
            tick_units * manual
        };

        // Build profile: prefer trades if available
        let levels = match filtered_trades {
            Some(filtered) if !filtered.is_empty() => {
                profile_core::build_profile_from_trades(
                    filtered,
                    input.tick_size,
                    group_quantum,
                )
            }
            _ => profile_core::build_profile_from_candles(
                candle_slice,
                input.tick_size,
                group_quantum,
            ),
        };

        let grouping_mode = if is_automatic {
            let factor = self
                .config
                .get_int("auto_group_factor", 1)
                .max(1);
            VbpGroupingMode::Automatic { factor }
        } else {
            VbpGroupingMode::Manual
        };

        if levels.is_empty() {
            self.output = StudyOutput::Empty;
            return Ok(());
        }

        // Read config values
        let vbp_type = Self::parse_vbp_type(
            self.config.get_choice("vbp_type", "Volume"),
        );
        let side = Self::parse_side(
            self.config.get_choice("alignment", "Left"),
        );
        let width_pct = self
            .config
            .get_float("width_pct", 0.25)
            as f32;
        let opacity =
            self.config.get_float("opacity", 0.7) as f32;
        let volume_color = self.config.get_color(
            "volume_color",
            DEFAULT_VOLUME_COLOR,
        );
        let bid_color = self
            .config
            .get_color("bid_color", DEFAULT_BID_COLOR);
        let ask_color = self
            .config
            .get_color("ask_color", DEFAULT_ASK_COLOR);

        // Build nested configs
        let poc_config = self.build_poc_config();
        let va_config = self.build_va_config();
        let node_config = self.build_node_config();
        let vwap_config = self.build_vwap_config();

        // Compute POC and Value Area
        let poc = profile_core::find_poc_index(&levels);
        let value_area = if va_config.show_value_area {
            poc.and_then(|idx| {
                profile_core::calculate_value_area(
                    &levels,
                    idx,
                    va_config.value_area_pct as f64,
                )
            })
        } else {
            None
        };

        // Compute time range from candle slice
        let time_range = {
            let start = candle_slice
                .first()
                .map(|c| c.time.to_millis())
                .unwrap_or(0);
            let end = candle_slice
                .last()
                .map(|c| c.time.to_millis())
                .unwrap_or(0);
            Some((start, end))
        };

        // Developing features (POC, Peak, Valley)
        let need_dev_poc = poc_config.show_developing_poc;
        let need_dev_peak = node_config.show_developing_peak;
        let need_dev_valley =
            node_config.show_developing_valley;

        let (
            developing_poc_points,
            developing_peak_points,
            developing_valley_points,
        ) = if need_dev_poc
            || need_dev_peak
            || need_dev_valley
        {
            Self::compute_developing_features(
                candle_slice,
                input.tick_size,
                group_quantum,
                node_config.hvn_method,
                node_config.hvn_threshold,
                node_config.lvn_method,
                node_config.lvn_threshold,
                need_dev_poc,
                need_dev_peak,
                need_dev_valley,
            )
        } else {
            (Vec::new(), Vec::new(), Vec::new())
        };

        // Zone + peak/valley detection
        let any_node_feature = node_config.show_hvn_zones
            || node_config.show_lvn_zones
            || node_config.show_peak_line
            || node_config.show_valley_line;

        let (hvn_zones, lvn_zones, peak_node, valley_node) =
            if any_node_feature {
                profile_core::detect_volume_zones(
                    &levels,
                    node_config.hvn_method,
                    node_config.hvn_threshold,
                    node_config.lvn_method,
                    node_config.lvn_threshold,
                    node_config.min_prominence,
                )
            } else {
                (Vec::new(), Vec::new(), None, None)
            };

        // Anchored VWAP
        let (
            vwap_points,
            vwap_upper_points,
            vwap_lower_points,
        ) = if vwap_config.show_vwap {
            Self::compute_vwap(
                candle_slice,
                vwap_config.show_bands,
                vwap_config.band_multiplier,
            )
        } else {
            (Vec::new(), Vec::new(), Vec::new())
        };

        self.output = StudyOutput::Profile(
            ProfileOutput {
                levels,
                quantum: group_quantum,
                poc: if poc_config.show_poc {
                    poc
                } else {
                    None
                },
                value_area,
                time_range,
                hvn_zones,
                lvn_zones,
                peak_node,
                valley_node,
                developing_poc_points,
                developing_peak_points,
                developing_valley_points,
                vwap_points,
                vwap_upper_points,
                vwap_lower_points,
                grouping_mode,
                resolved_cache: std::sync::Mutex::new(None),
            },
            ProfileRenderConfig {
                vbp_type,
                side,
                width_pct,
                opacity,
                volume_color,
                bid_color,
                ask_color,
                poc_config,
                va_config,
                node_config,
                vwap_config,
            },
        );

        Ok(())
    }

    fn output(&self) -> &StudyOutput {
        &self.output
    }

    fn reset(&mut self) {
        self.output = StudyOutput::Empty;
        self.last_input_fingerprint = (0, 0, 0, 0);
        self.last_stable_range = None;
    }

    fn clone_study(&self) -> Box<dyn Study> {
        Box::new(Self {
            config: self.config.clone(),
            output: self.output.clone(),
            params: self.params.clone(),
            last_input_fingerprint: (0, 0, 0, 0),
            last_stable_range: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::{ProfileSide, VbpType};
    use data::{
        Candle, ChartBasis, Price, Timeframe, Timestamp, Volume,
    };

    fn make_candle(
        time: u64,
        open: f32,
        high: f32,
        low: f32,
        close: f32,
        buy_vol: f64,
        sell_vol: f64,
    ) -> Candle {
        Candle::new(
            Timestamp::from_millis(time),
            Price::from_f32(open),
            Price::from_f32(high),
            Price::from_f32(low),
            Price::from_f32(close),
            Volume(buy_vol),
            Volume(sell_vol),
        )
        .expect("test: valid candle")
    }

    fn make_input(candles: &[Candle]) -> StudyInput<'_> {
        StudyInput {
            candles,
            trades: None,
            basis: ChartBasis::Time(Timeframe::M1),
            tick_size: Price::from_f32(1.0),
            visible_range: None,
        }
    }

    #[test]
    fn test_vbp_compute_default() {
        let mut study = VbpStudy::new();
        let candles = vec![
            make_candle(
                1000, 100.0, 102.0, 99.0, 101.0, 100.0, 50.0,
            ),
            make_candle(
                2000, 101.0, 103.0, 100.0, 102.0, 80.0, 60.0,
            ),
        ];

        let input = make_input(&candles);
        study.compute(&input).unwrap();

        match &study.output {
            StudyOutput::Profile(data, config) => {
                assert!(!data.levels.is_empty());
                assert!(data.poc.is_some());
                assert_eq!(
                    config.vbp_type,
                    VbpType::Volume
                );
                assert_eq!(
                    config.side,
                    ProfileSide::Left
                );
            }
            _ => panic!("Expected Profile output"),
        }
    }

    #[test]
    fn test_vbp_empty_candles() {
        let mut study = VbpStudy::new();
        let candles: Vec<Candle> = vec![];
        let input = make_input(&candles);
        study.compute(&input).unwrap();
        assert!(matches!(
            study.output(),
            StudyOutput::Empty
        ));
    }

    #[test]
    fn test_vbp_type_parsing() {
        assert_eq!(
            VbpStudy::parse_vbp_type("Volume"),
            VbpType::Volume
        );
        assert_eq!(
            VbpStudy::parse_vbp_type("Bid/Ask Volume"),
            VbpType::BidAskVolume
        );
        assert_eq!(
            VbpStudy::parse_vbp_type("Delta"),
            VbpType::Delta
        );
        assert_eq!(
            VbpStudy::parse_vbp_type("Delta & Total Volume"),
            VbpType::DeltaAndTotalVolume
        );
        assert_eq!(
            VbpStudy::parse_vbp_type("Delta Percentage"),
            VbpType::DeltaPercentage
        );
    }

    #[test]
    fn test_vbp_period_length() {
        let mut study = VbpStudy::new();
        study
            .set_parameter(
                "period",
                ParameterValue::Choice("Length".to_string()),
            )
            .unwrap();
        study
            .set_parameter(
                "length_unit",
                ParameterValue::Choice("Days".to_string()),
            )
            .unwrap();
        study
            .set_parameter(
                "length_value",
                ParameterValue::Integer(2),
            )
            .unwrap();

        let day_ms = 86_400_000u64;
        let candles = vec![
            make_candle(
                day_ms, 100.0, 102.0, 99.0, 101.0, 50.0,
                50.0,
            ),
            make_candle(
                day_ms * 2,
                101.0,
                103.0,
                100.0,
                102.0,
                50.0,
                50.0,
            ),
            make_candle(
                day_ms * 3,
                102.0,
                104.0,
                101.0,
                103.0,
                50.0,
                50.0,
            ),
        ];

        let input = StudyInput {
            candles: &candles,
            trades: None,
            basis: ChartBasis::Time(Timeframe::D1),
            tick_size: Price::from_f32(1.0),
            visible_range: None,
        };

        study.compute(&input).unwrap();
        assert!(matches!(
            study.output(),
            StudyOutput::Profile(_, _)
        ));
    }

    #[test]
    fn test_vbp_value_area_disabled() {
        let mut study = VbpStudy::new();
        study
            .set_parameter(
                "va_show",
                ParameterValue::Boolean(false),
            )
            .unwrap();

        let candles = vec![make_candle(
            1000, 100.0, 102.0, 99.0, 101.0, 100.0, 50.0,
        )];
        let input = make_input(&candles);
        study.compute(&input).unwrap();

        match &study.output {
            StudyOutput::Profile(data, _config) => {
                assert!(data.value_area.is_none());
            }
            _ => panic!("Expected Profile output"),
        }
    }

    #[test]
    fn test_vbp_new_tab_params() {
        let study = VbpStudy::new();
        let params = study.parameters();

        // Check we have params on all 4 new tabs
        let has_tab4 = params
            .iter()
            .any(|p| p.tab == ParameterTab::Tab4);
        let has_tab5 = params
            .iter()
            .any(|p| p.tab == ParameterTab::Tab5);
        let has_tab6 = params
            .iter()
            .any(|p| p.tab == ParameterTab::Tab6);
        let has_tab7 = params
            .iter()
            .any(|p| p.tab == ParameterTab::Tab7);

        assert!(has_tab4, "Missing POC tab params");
        assert!(has_tab5, "Missing Value Area tab params");
        assert!(
            has_tab6,
            "Missing Peak & Valley tab params"
        );
        assert!(has_tab7, "Missing VWAP tab params");
    }

    #[test]
    fn test_vbp_developing_poc() {
        let mut study = VbpStudy::new();
        study
            .set_parameter(
                "poc_show_developing",
                ParameterValue::Boolean(true),
            )
            .unwrap();

        let candles = vec![
            make_candle(
                1000, 100.0, 102.0, 99.0, 101.0, 100.0, 50.0,
            ),
            make_candle(
                2000, 101.0, 103.0, 100.0, 102.0, 80.0, 60.0,
            ),
            make_candle(
                3000, 102.0, 104.0, 101.0, 103.0, 120.0, 80.0,
            ),
        ];
        let input = make_input(&candles);
        study.compute(&input).unwrap();

        match &study.output {
            StudyOutput::Profile(data, _config) => {
                assert_eq!(
                    data.developing_poc_points.len(),
                    3,
                    "should have one point per candle"
                );
                for (ts, price) in
                    &data.developing_poc_points
                {
                    assert!(*ts > 0);
                    assert!(*price > 0);
                }
            }
            _ => panic!("Expected Profile output"),
        }
    }

    #[test]
    fn test_vbp_vwap_computation() {
        let mut study = VbpStudy::new();
        study
            .set_parameter(
                "vwap_show",
                ParameterValue::Boolean(true),
            )
            .unwrap();
        study
            .set_parameter(
                "vwap_show_bands",
                ParameterValue::Boolean(true),
            )
            .unwrap();

        let candles = vec![
            make_candle(
                1000, 100.0, 102.0, 98.0, 100.0, 50.0, 50.0,
            ),
            make_candle(
                2000, 100.0, 104.0, 99.0, 103.0, 80.0, 40.0,
            ),
        ];
        let input = make_input(&candles);
        study.compute(&input).unwrap();

        match &study.output {
            StudyOutput::Profile(data, _config) => {
                assert_eq!(data.vwap_points.len(), 2);
                assert_eq!(
                    data.vwap_upper_points.len(),
                    2
                );
                assert_eq!(
                    data.vwap_lower_points.len(),
                    2
                );
                for (_, price) in &data.vwap_points {
                    assert!(
                        *price > 90.0
                            && *price < 110.0
                    );
                }
                for i in 0..2 {
                    assert!(
                        data.vwap_upper_points[i].1
                            >= data.vwap_points[i].1
                    );
                    assert!(
                        data.vwap_lower_points[i].1
                            <= data.vwap_points[i].1
                    );
                }
            }
            _ => panic!("Expected Profile output"),
        }
    }

    #[test]
    fn test_vbp_peak_valley_integration() {
        let mut study = VbpStudy::new();
        study
            .set_parameter(
                "peak_show",
                ParameterValue::Boolean(true),
            )
            .unwrap();
        study
            .set_parameter(
                "valley_show",
                ParameterValue::Boolean(true),
            )
            .unwrap();
        study
            .set_parameter(
                "hvn_zone_show",
                ParameterValue::Boolean(true),
            )
            .unwrap();
        study
            .set_parameter(
                "node_hvn_method",
                ParameterValue::Choice(
                    "Relative".to_string(),
                ),
            )
            .unwrap();
        study
            .set_parameter(
                "node_hvn_threshold",
                ParameterValue::Float(0.5),
            )
            .unwrap();
        study
            .set_parameter(
                "node_lvn_method",
                ParameterValue::Choice(
                    "Relative".to_string(),
                ),
            )
            .unwrap();
        study
            .set_parameter(
                "node_lvn_threshold",
                ParameterValue::Float(0.2),
            )
            .unwrap();

        // Create candles that produce a profile with clear
        // peaks and valleys (need enough levels)
        let candles = vec![
            make_candle(
                1000, 100.0, 110.0, 90.0, 105.0, 200.0,
                100.0,
            ),
            make_candle(
                2000, 105.0, 115.0, 95.0, 110.0, 50.0, 30.0,
            ),
        ];
        let input = make_input(&candles);
        study.compute(&input).unwrap();

        match &study.output {
            StudyOutput::Profile(data, config) => {
                assert!(!data.levels.is_empty());
                assert!(
                    config.node_config.show_peak_line
                );
                assert!(
                    config.node_config.show_valley_line
                );
                assert!(
                    config.node_config.show_hvn_zones
                );
            }
            _ => panic!("Expected Profile output"),
        }
    }

    #[test]
    fn test_vbp_fingerprint_invalidation() {
        let mut study = VbpStudy::new();
        let candles = vec![make_candle(
            1000, 100.0, 102.0, 99.0, 101.0, 100.0, 50.0,
        )];
        let input = make_input(&candles);

        study.compute(&input).unwrap();
        assert!(matches!(
            study.output(),
            StudyOutput::Profile(_, _)
        ));

        // Change a parameter -- should invalidate fingerprint
        study
            .set_parameter(
                "poc_show",
                ParameterValue::Boolean(false),
            )
            .unwrap();
        assert_eq!(
            study.last_input_fingerprint,
            (0, 0, 0, 0)
        );

        // Recompute should work
        study.compute(&input).unwrap();
        assert!(matches!(
            study.output(),
            StudyOutput::Profile(_, _)
        ));
    }

    #[test]
    fn test_vbp_clone_with_new_fields() {
        let mut study = VbpStudy::new();
        study
            .set_parameter(
                "poc_show_developing",
                ParameterValue::Boolean(true),
            )
            .unwrap();
        study
            .set_parameter(
                "vwap_show",
                ParameterValue::Boolean(true),
            )
            .unwrap();
        study
            .set_parameter(
                "peak_show",
                ParameterValue::Boolean(true),
            )
            .unwrap();

        let candles = vec![
            make_candle(
                1000, 100.0, 102.0, 99.0, 101.0, 100.0, 50.0,
            ),
            make_candle(
                2000, 101.0, 103.0, 100.0, 102.0, 80.0, 60.0,
            ),
        ];
        let input = make_input(&candles);
        study.compute(&input).unwrap();

        let cloned = study.clone_study();
        match cloned.output() {
            StudyOutput::Profile(data, _config) => {
                assert!(!data.levels.is_empty());
                assert!(
                    !data.developing_poc_points.is_empty()
                );
                assert!(!data.vwap_points.is_empty());
            }
            _ => panic!("Expected Profile output"),
        }
    }
}
