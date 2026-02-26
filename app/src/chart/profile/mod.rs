mod construct;
mod invalidate;
mod render;
mod replay;
mod studies;

use crate::chart::{
    Chart, PlotLimits, ViewState,
    drawing::{ChartDrawingAccess, DrawingManager},
};
use crate::screen::dashboard::pane::config::ProfileConfig;
use data::FuturesTickerInfo;
use data::{Candle, ChartBasis, ChartData};
use data::{Price, PriceStep};
use iced::Vector;
use iced::widget::canvas::Cache;
use study::Study as _;
use study::config::ParameterValue;
use study::output::{ProfileOutput, StudyOutput};
use study::studies::orderflow::VbpStudy;

use std::time::Instant;

/// Standalone profile chart — entire pane shows a Volume-by-Price
/// profile. Y-axis = Price, horizontal bars = Volume.
///
/// Internally delegates computation to a [`VbpStudy`] instance
/// configured from the pane's [`ProfileConfig`].
pub struct ProfileChart {
    chart: ViewState,
    chart_data: ChartData,
    basis: ChartBasis,
    ticker_info: FuturesTickerInfo,
    last_tick: Instant,
    drawings: DrawingManager,

    /// VBP study that computes the profile levels, POC, VA, zones,
    /// peak/valley, and developing features.
    profile_study: VbpStudy,
    /// Fingerprint to skip redundant recomputes
    fingerprint: (usize, u64, u64, usize),

    // Display config
    display_config: ProfileConfig,

    // Overlay / panel studies
    studies: Vec<Box<dyn study::Study>>,
    studies_dirty: bool,
    last_visible_range: Option<(u64, u64)>,
    panel_cache: Cache,
    panel_labels_cache: Cache,
    panel_crosshair_cache: Cache,
}

impl Chart for ProfileChart {
    // Boilerplate: state, mut_state, drawings, studies, panel_cache,
    // panel_labels_cache, panel_crosshair_cache.
    crate::chart_impl!(ProfileChart);

    fn invalidate_crosshair(&mut self) {
        self.chart.cache.clear_crosshair();
        self.panel_crosshair_cache.clear();
    }

    fn invalidate_all(&mut self) {
        self.invalidate();
    }

    fn interval_keys(&self) -> Option<Vec<u64>> {
        // Profile chart has no time axis
        None
    }

    fn autoscaled_coords(&self) -> Vector {
        let chart = self.state();
        let x_translation =
            0.5 * (chart.bounds.width / chart.scaling) - (8.0 * chart.cell_width / chart.scaling);
        Vector::new(x_translation, chart.translation.y)
    }

    fn supports_fit_autoscaling(&self) -> bool {
        true
    }

    fn is_empty(&self) -> bool {
        self.chart_data.candles.is_empty() && self.chart_data.trades.is_empty()
    }

    fn plot_limits(&self) -> PlotLimits {
        PlotLimits {
            max_cell_width: 100.0,
            min_cell_width: 0.01,
            max_cell_height: 200.0,
            min_cell_height: 0.1,
            default_cell_width: 4.0,
        }
    }
}

impl ProfileChart {
    /// Apply a new display configuration.
    pub fn set_display_config(&mut self, config: ProfileConfig) {
        self.display_config = config;
        self.fingerprint = (0, 0, 0, 0); // force recompute
        self.recompute_profile();
        self.invalidate();
    }

    pub fn display_config(&self) -> &ProfileConfig {
        &self.display_config
    }

    pub fn last_update(&self) -> Instant {
        self.last_tick
    }

    pub fn chart_layout(&self) -> data::ViewConfig {
        self.chart.layout()
    }

    pub fn drawings(&self) -> &DrawingManager {
        &self.drawings
    }

    pub fn drawings_mut(&mut self) -> &mut DrawingManager {
        self.chart.cache.clear_drawings();
        &mut self.drawings
    }

    // ── Study output accessors ──────────────────────────────────

    /// Extract all profiles and the render config.
    fn profiles_and_config(
        &self,
    ) -> Option<(&[ProfileOutput], &study::output::ProfileRenderConfig)> {
        match self.profile_study.output() {
            StudyOutput::Profile(profiles, config) if !profiles.is_empty() => {
                Some((profiles.as_slice(), config))
            }
            _ => None,
        }
    }

}

impl ChartDrawingAccess for ProfileChart {
    fn drawings(&self) -> &DrawingManager {
        &self.drawings
    }

    fn drawings_mut(&mut self) -> &mut DrawingManager {
        &mut self.drawings
    }

    fn view_state(&self) -> &ViewState {
        &self.chart
    }

    fn invalidate_drawings_cache(&mut self) {
        self.chart.cache.clear_drawings();
    }

    fn invalidate_crosshair_cache(&mut self) {
        self.chart.cache.clear_crosshair();
    }
}

/// Map a [`ProfileConfig`] onto a [`VbpStudy`]'s parameters so
/// the study produces the same output the old manual code did.
///
/// Because ProfileChart does its own period slicing via
/// `resolve_data_slice()`, we always set the study to `Auto`
/// period (it will receive the pre-sliced candles/trades).
pub(super) fn apply_profile_config_to_study(
    study: &mut VbpStudy,
    cfg: &ProfileConfig,
    _info: &FuturesTickerInfo,
) {
    use crate::screen::dashboard::pane::config::ProfileDisplayType as DT;

    // Helper — ignore set_parameter errors (param names are known)
    macro_rules! set {
        ($key:expr, $val:expr) => {
            let _ = study.set_parameter($key, $val);
        };
    }

    // ── VBP type ──────────────────────────────────────────────
    let vbp_type = match cfg.display_type {
        DT::Volume => "Volume",
        DT::BidAskVolume => "Bid/Ask Volume",
        DT::Delta => "Delta",
        DT::DeltaAndTotal => "Delta & Total Volume",
        DT::DeltaPercentage => "Delta Percentage",
    };
    set!("vbp_type", ParameterValue::Choice(vbp_type.into()));

    // ── Period — always Split mode ──────────────────────────────
    set!("period", ParameterValue::Choice("Split".into()));

    // Map ProfileSplitUnit + split_value to VBP study params
    use crate::screen::dashboard::pane::config::ProfileSplitUnit;
    let interval_str = match (cfg.split_unit, cfg.split_value) {
        (ProfileSplitUnit::Days, 1) => "1 Day",
        (ProfileSplitUnit::Hours, 4) => "4 Hours",
        (ProfileSplitUnit::Hours, 2) => "2 Hours",
        (ProfileSplitUnit::Hours, 1) => "1 Hour",
        (ProfileSplitUnit::Minutes, 30) => "30 Minutes",
        (ProfileSplitUnit::Minutes, 15) => "15 Minutes",
        _ => "Custom",
    };
    set!(
        "split_interval",
        ParameterValue::Choice(interval_str.into())
    );

    // For custom intervals, set split_unit and split_value
    if interval_str == "Custom" {
        let unit_str = match cfg.split_unit {
            ProfileSplitUnit::Days => "Days",
            ProfileSplitUnit::Hours => "Hours",
            ProfileSplitUnit::Minutes => "Minutes",
        };
        set!("split_unit", ParameterValue::Choice(unit_str.into()));
        set!(
            "split_value",
            ParameterValue::Integer(cfg.split_value.max(1))
        );
    }

    set!(
        "max_profiles",
        ParameterValue::Integer(cfg.max_profiles.max(1))
    );

    // ── Tick grouping ─────────────────────────────────────────
    // ProfileChart bakes auto_group_factor into the quantum.
    // VbpStudy's "Automatic" mode uses plain tick_units and
    // stores the factor only for renderer-side merging. To
    // reproduce the old behaviour we set "Manual" mode with the
    // effective tick count already multiplied.
    if cfg.auto_grouping {
        set!("auto_grouping", ParameterValue::Choice("Manual".into()));
        set!(
            "manual_ticks",
            ParameterValue::Integer(cfg.auto_group_factor.max(1),)
        );
    } else {
        set!("auto_grouping", ParameterValue::Choice("Manual".into()));
        set!(
            "manual_ticks",
            ParameterValue::Integer(cfg.manual_ticks.max(1))
        );
    }

    // ── Opacity / width (profile fills whole pane) ────────────
    set!("opacity", ParameterValue::Float(cfg.opacity as f64));
    // Profile fills most of the pane width.
    set!("width_pct", ParameterValue::Float(0.90));

    // ── Colors ────────────────────────────────────────────────
    if let Some(c) = cfg.volume_color {
        set!("volume_color", ParameterValue::Color(c));
    }
    if let Some(c) = cfg.bid_color {
        set!("bid_color", ParameterValue::Color(c));
    }
    if let Some(c) = cfg.ask_color {
        set!("ask_color", ParameterValue::Color(c));
    }

    // ── POC ───────────────────────────────────────────────────
    set!("poc_show", ParameterValue::Boolean(cfg.show_poc));
    if let Some(c) = cfg.poc_color {
        set!("poc_color", ParameterValue::Color(c));
    }
    set!(
        "poc_line_width",
        ParameterValue::Float(cfg.poc_line_width as f64)
    );
    set!(
        "poc_line_style",
        ParameterValue::LineStyle(to_study_line_style(cfg.poc_line_style),)
    );
    set!(
        "poc_extend",
        ParameterValue::Choice(extend_to_str(cfg.poc_extend).into(),)
    );
    set!(
        "poc_show_label",
        ParameterValue::Boolean(cfg.show_poc_label)
    );

    // ── Value Area ────────────────────────────────────────────
    set!("va_show", ParameterValue::Boolean(cfg.show_va_highlight));
    set!(
        "value_area_pct",
        ParameterValue::Float(cfg.value_area_pct as f64)
    );
    set!(
        "va_show_highlight",
        ParameterValue::Boolean(cfg.show_va_highlight)
    );
    if let Some(c) = cfg.vah_color {
        set!("va_vah_color", ParameterValue::Color(c));
    }
    set!(
        "va_vah_line_width",
        ParameterValue::Float(cfg.vah_line_width as f64)
    );
    set!(
        "va_vah_line_style",
        ParameterValue::LineStyle(to_study_line_style(cfg.vah_line_style),)
    );
    if let Some(c) = cfg.val_color {
        set!("va_val_color", ParameterValue::Color(c));
    }
    set!(
        "va_val_line_width",
        ParameterValue::Float(cfg.val_line_width as f64)
    );
    set!(
        "va_val_line_style",
        ParameterValue::LineStyle(to_study_line_style(cfg.val_line_style),)
    );
    set!(
        "va_extend",
        ParameterValue::Choice(extend_to_str(cfg.va_extend).into(),)
    );
    set!(
        "va_show_labels",
        ParameterValue::Boolean(cfg.show_va_labels)
    );

    // VA fill
    set!("va_show_fill", ParameterValue::Boolean(cfg.show_va_fill));
    if let Some(c) = cfg.va_fill_color {
        set!("va_fill_color", ParameterValue::Color(c));
    }
    set!(
        "va_fill_opacity",
        ParameterValue::Float(cfg.va_fill_opacity as f64)
    );

    // ── Node detection ────────────────────────────────────────
    set!(
        "node_hvn_method",
        ParameterValue::Choice(cfg.hvn_method.to_string())
    );
    set!(
        "node_hvn_threshold",
        ParameterValue::Float(cfg.hvn_threshold as f64)
    );
    set!(
        "node_lvn_method",
        ParameterValue::Choice(cfg.lvn_method.to_string())
    );
    set!(
        "node_lvn_threshold",
        ParameterValue::Float(cfg.lvn_threshold as f64)
    );

    // HVN zones
    set!("hvn_zone_show", ParameterValue::Boolean(cfg.show_hvn_zones));
    if let Some(c) = cfg.hvn_zone_color {
        set!("hvn_zone_color", ParameterValue::Color(c));
    }
    set!(
        "hvn_zone_opacity",
        ParameterValue::Float(cfg.hvn_zone_opacity as f64)
    );

    // LVN zones
    set!("lvn_zone_show", ParameterValue::Boolean(cfg.show_lvn_zones));
    if let Some(c) = cfg.lvn_zone_color {
        set!("lvn_zone_color", ParameterValue::Color(c));
    }
    set!(
        "lvn_zone_opacity",
        ParameterValue::Float(cfg.lvn_zone_opacity as f64)
    );

    // Peak
    set!("peak_show", ParameterValue::Boolean(cfg.show_peak_line));
    if let Some(c) = cfg.peak_color {
        set!("peak_color", ParameterValue::Color(c));
    }
    set!(
        "peak_line_style",
        ParameterValue::LineStyle(to_study_line_style(cfg.peak_line_style),)
    );
    set!(
        "peak_line_width",
        ParameterValue::Float(cfg.peak_line_width as f64)
    );
    set!(
        "peak_show_label",
        ParameterValue::Boolean(cfg.show_peak_label)
    );

    // Valley
    set!("valley_show", ParameterValue::Boolean(cfg.show_valley_line));
    if let Some(c) = cfg.valley_color {
        set!("valley_color", ParameterValue::Color(c));
    }
    set!(
        "valley_line_style",
        ParameterValue::LineStyle(to_study_line_style(cfg.valley_line_style),)
    );
    set!(
        "valley_line_width",
        ParameterValue::Float(cfg.valley_line_width as f64)
    );
    set!(
        "valley_show_label",
        ParameterValue::Boolean(cfg.show_valley_label)
    );
}

/// Convert a `ProfileLineStyle` to `study::config::LineStyleValue`.
pub(super) fn to_study_line_style(
    s: crate::screen::dashboard::pane::config::ProfileLineStyle,
) -> study::config::LineStyleValue {
    use crate::screen::dashboard::pane::config::ProfileLineStyle as P;
    match s {
        P::Solid => study::config::LineStyleValue::Solid,
        P::Dashed => study::config::LineStyleValue::Dashed,
        P::Dotted => study::config::LineStyleValue::Dotted,
    }
}

/// Convert a `ProfileExtendDirection` to the string the VBP study
/// understands.
pub(super) fn extend_to_str(
    e: crate::screen::dashboard::pane::config::ProfileExtendDirection,
) -> &'static str {
    use crate::screen::dashboard::pane::config::ProfileExtendDirection as E;
    match e {
        E::None => "None",
        E::Left => "Left",
        E::Right => "Right",
        E::Both => "Both",
    }
}

/// Compute initial price scale from profile data.
pub(super) fn compute_initial_price_scale(
    candles: &[Candle],
    tick_size: f32,
) -> (Price, Price, f32) {
    let step = PriceStep::from_f32(tick_size);

    let (scale_high, scale_low) = if !candles.is_empty() {
        let high = candles
            .iter()
            .map(|c| Price::from_units(c.high.units()))
            .max()
            .unwrap_or(Price::from_f32(0.0));
        let low = candles
            .iter()
            .map(|c| Price::from_units(c.low.units()))
            .min()
            .unwrap_or(Price::from_f32(0.0));
        (high, low)
    } else {
        (Price::from_f32(100.0), Price::from_f32(0.0))
    };

    let low_rounded = scale_low.round_to_side_step(true, step.into());
    let high_rounded = scale_high.round_to_side_step(false, step.into());

    let y_ticks = Price::steps_between_inclusive(low_rounded, high_rounded, step.into())
        .map(|n| n.saturating_sub(1))
        .unwrap_or(1)
        .max(1) as f32;

    let cell_height = 200.0 / y_ticks;

    (scale_high, scale_low, cell_height)
}
