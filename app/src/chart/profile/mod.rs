mod render;
mod studies;

use crate::chart::{
    Chart, PlotLimits, ViewState,
    drawing::{ChartDrawingAccess, DrawingManager},
};
use data::state::pane::ProfileConfig;
use data::util::count_decimals;
use data::{
    Autoscale, Candle, ChartBasis, ChartData,
    Price as DomainPrice, Side, Timeframe, Trade, ViewConfig,
};
use exchange::FuturesTickerInfo;
use exchange::util::{Price, PriceStep};
use iced::Vector;
use iced::widget::canvas::Cache;
use study::Study;
use study::config::ParameterValue;
use study::orderflow::VbpStudy;
use study::output::{ProfileOutput, StudyOutput};
use study::traits::StudyInput;

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
    fn state(&self) -> &ViewState {
        &self.chart
    }

    fn mut_state(&mut self) -> &mut ViewState {
        &mut self.chart
    }

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
        let x_translation = 0.5
            * (chart.bounds.width / chart.scaling)
            - (8.0 * chart.cell_width / chart.scaling);
        Vector::new(x_translation, chart.translation.y)
    }

    fn supports_fit_autoscaling(&self) -> bool {
        true
    }

    fn is_empty(&self) -> bool {
        self.chart_data.candles.is_empty()
            && self.chart_data.trades.is_empty()
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

    fn drawings(&self) -> Option<&DrawingManager> {
        Some(&self.drawings)
    }

    fn studies(&self) -> &[Box<dyn study::Study>] {
        &self.studies
    }

    fn panel_cache(&self) -> Option<&Cache> {
        Some(&self.panel_cache)
    }

    fn panel_labels_cache(&self) -> Option<&Cache> {
        Some(&self.panel_labels_cache)
    }

    fn panel_crosshair_cache(&self) -> Option<&Cache> {
        Some(&self.panel_crosshair_cache)
    }
}

impl ProfileChart {
    /// Create a new ProfileChart from loaded chart data.
    pub fn from_chart_data(
        chart_data: ChartData,
        ticker_info: FuturesTickerInfo,
        layout: ViewConfig,
        config: ProfileConfig,
    ) -> Self {
        let step = PriceStep::from_f32(ticker_info.tick_size);
        let basis = ChartBasis::Time(Timeframe::M5);

        // Compute initial cell_height from price range
        let (_, _, cell_height) = compute_initial_price_scale(
            &chart_data.candles,
            ticker_info.tick_size,
        );

        let base_price_y = chart_data
            .candles
            .iter()
            .map(|c| c.high)
            .max()
            .map(|p| Price::from_units(p.units()))
            .unwrap_or(Price::from_f32(0.0));

        let default_cell_width = 4.0;
        let latest_x = chart_data.candles.last().map(|c| c.time.0).unwrap_or(0);

        let mut chart = ViewState::new(
            basis,
            step,
            count_decimals(ticker_info.tick_size),
            ticker_info,
            ViewConfig {
                splits: layout.splits,
                autoscale: Some(Autoscale::FitAll),
            },
            default_cell_width,
            cell_height,
        );
        chart.base_price_y = base_price_y;
        chart.latest_x = latest_x;

        let x_translation = 0.5
            * (chart.bounds.width / chart.scaling)
            - (8.0 * chart.cell_width / chart.scaling);
        chart.translation.x = x_translation;
        chart.translation.y = -chart.bounds.height / 2.0;

        let mut profile_study = VbpStudy::new();
        apply_profile_config_to_study(
            &mut profile_study,
            &config,
            &ticker_info,
        );

        let mut profile = ProfileChart {
            chart,
            chart_data,
            basis,
            ticker_info,
            last_tick: Instant::now(),
            drawings: DrawingManager::new(),
            profile_study,
            fingerprint: (0, 0, 0, 0),
            display_config: config,
            studies: Vec::new(),
            studies_dirty: false,
            last_visible_range: None,
            panel_cache: Cache::default(),
            panel_labels_cache: Cache::default(),
            panel_crosshair_cache: Cache::default(),
        };
        profile.recompute_profile();
        profile
    }

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

    pub fn chart_layout(&self) -> ViewConfig {
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

    /// Extract the `ProfileOutput` from the study, if computed.
    pub(super) fn profile_output(
        &self,
    ) -> Option<&ProfileOutput> {
        match self.profile_study.output() {
            StudyOutput::Profile(profiles, _) => {
                profiles.first()
            }
            _ => None,
        }
    }

    /// Extract all profiles and the render config.
    fn profiles_and_config(
        &self,
    ) -> Option<(
        &[ProfileOutput],
        &study::output::ProfileRenderConfig,
    )> {
        match self.profile_study.output() {
            StudyOutput::Profile(profiles, config)
                if !profiles.is_empty() =>
            {
                Some((profiles.as_slice(), config))
            }
            _ => None,
        }
    }

    /// Convenience: borrow the levels slice from the study output.
    fn profile_levels(
        &self,
    ) -> Option<&[study::output::ProfileLevel]> {
        self.profile_output().map(|o| o.levels.as_slice())
    }

    // ── Profile computation ─────────────────────────────────────

    /// Rebuild the volume profile via the internal VbpStudy.
    fn recompute_profile(&mut self) {
        let fp = (
            self.chart_data.trades.len(),
            self.chart_data.trades.first().map(|t| t.time.0).unwrap_or(0),
            self.chart_data.trades.last().map(|t| t.time.0).unwrap_or(0),
            self.chart_data.candles.len(),
        );
        if fp == self.fingerprint
            && !matches!(
                self.profile_study.output(),
                StudyOutput::Empty
            )
        {
            return;
        }
        self.fingerprint = fp;

        // Reapply config in case display_config changed
        apply_profile_config_to_study(
            &mut self.profile_study,
            &self.display_config,
            &self.ticker_info,
        );

        // Always pass all data — split mode handles segmentation
        let trades: Option<&[Trade]> =
            if !self.chart_data.trades.is_empty() {
                Some(&self.chart_data.trades)
            } else {
                None
            };
        let input = StudyInput {
            candles: &self.chart_data.candles,
            trades,
            basis: self.basis,
            tick_size: DomainPrice::from_f32(
                self.ticker_info.tick_size,
            ),
            visible_range: None,
        };
        if let Err(e) = self.profile_study.compute(&input) {
            log::warn!("Profile study compute error: {e}");
        }
    }

    pub fn invalidate(&mut self) {
        // Snapshot the price extremes from the study output before
        // we mutably borrow `self.chart` for autoscaling.
        let price_extremes = self
            .profile_levels()
            .filter(|l| !l.is_empty())
            .map(|levels| {
                let highest = levels
                    .last()
                    .map(|l| l.price as f32)
                    .unwrap_or(0.0);
                let lowest = levels
                    .first()
                    .map(|l| l.price as f32)
                    .unwrap_or(0.0);
                (highest, lowest)
            });

        let chart = &mut self.chart;

        // Fit-all autoscaling: fit price range to visible area
        if let Some(Autoscale::FitAll) = chart.layout.autoscale {
            if let Some((highest, lowest)) = price_extremes {
                let padding = (highest - lowest) * 0.05;
                let price_span =
                    (highest - lowest) + (2.0 * padding);

                if price_span > 0.0
                    && chart.bounds.height > f32::EPSILON
                {
                    let padded_highest = highest + padding;
                    let chart_height = chart.bounds.height;
                    let tick_size =
                        chart.tick_size.to_f32_lossy();

                    if tick_size > 0.0 {
                        chart.cell_height =
                            (chart_height * tick_size)
                                / price_span;
                        chart.base_price_y =
                            Price::from_f32(padded_highest);
                        chart.translation.y =
                            -chart_height / 2.0;
                    }
                }
            }
        }

        chart.cache.clear_all();
        self.panel_cache.clear();
        self.panel_labels_cache.clear();
        self.panel_crosshair_cache.clear();

        // Check if visible range changed (triggers study recompute)
        if chart.bounds.width > 0.0 {
            let region = chart.visible_region(chart.bounds.size());
            let (_, _) = chart.interval_range(&region);
            let price_range = chart.price_range(&region);
            let new_range = Some((price_range.1.units() as u64, price_range.0.units() as u64));
            if new_range != self.last_visible_range {
                self.last_visible_range = new_range;
                self.studies_dirty = true;
            }
        }

        if self.studies_dirty {
            self.recompute_studies();
            self.studies_dirty = false;
        }

        self.last_tick = Instant::now();
    }

    /// Rebuild the chart from scratch with the given trades.
    pub fn rebuild_from_trades(&mut self, trades: &[Trade]) {
        self.chart_data.trades.clear();
        self.chart_data.candles.clear();

        self.profile_study.reset();
        for s in &mut self.studies {
            s.reset();
        }

        for trade in trades {
            self.append_trade(trade);
        }

        self.fingerprint = (0, 0, 0, 0); // force recompute
        self.recompute_profile();
        self.studies_dirty = true;
        self.invalidate();
    }

    /// Append a single trade during replay.
    pub fn append_trade(&mut self, trade: &Trade) {
        self.chart_data.trades.push(*trade);

        let (buy_vol, sell_vol) = match trade.side {
            Side::Buy | Side::Bid => (data::Volume(trade.quantity.0), data::Volume(0.0)),
            Side::Sell | Side::Ask => (data::Volume(0.0), data::Volume(trade.quantity.0)),
        };

        match self.basis {
            ChartBasis::Time(tf) => {
                let interval = tf.to_milliseconds();
                if interval == 0 {
                    return;
                }
                let bucket_time = (trade.time.to_millis() / interval) * interval;

                if let Some(last) = self.chart_data.candles.last_mut()
                    && last.time.0 == bucket_time
                {
                    last.high = last.high.max(trade.price);
                    last.low = last.low.min(trade.price);
                    last.close = trade.price;
                    last.buy_volume = data::Volume(last.buy_volume.0 + buy_vol.0);
                    last.sell_volume = data::Volume(last.sell_volume.0 + sell_vol.0);
                    return;
                }
                self.chart_data.candles.push(Candle {
                    time: data::Timestamp::from_millis(bucket_time),
                    open: trade.price,
                    high: trade.price,
                    low: trade.price,
                    close: trade.price,
                    buy_volume: buy_vol,
                    sell_volume: sell_vol,
                });
            }
            ChartBasis::Tick(_) => {} // Profile doesn't use tick basis
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
fn apply_profile_config_to_study(
    study: &mut VbpStudy,
    cfg: &ProfileConfig,
    _info: &FuturesTickerInfo,
) {
    use data::state::pane::ProfileDisplayType as DT;
    use data::state::pane::ProfileNodeDetectionMethod as NM;

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
    set!(
        "period",
        ParameterValue::Choice("Split".into())
    );

    // Map ProfileSplitUnit + split_value to VBP study params
    use data::state::pane::ProfileSplitUnit;
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
        set!(
            "split_unit",
            ParameterValue::Choice(unit_str.into())
        );
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
        set!(
            "auto_grouping",
            ParameterValue::Choice("Manual".into())
        );
        set!(
            "manual_ticks",
            ParameterValue::Integer(
                cfg.auto_group_factor.max(1),
            )
        );
    } else {
        set!(
            "auto_grouping",
            ParameterValue::Choice("Manual".into())
        );
        set!(
            "manual_ticks",
            ParameterValue::Integer(cfg.manual_ticks.max(1))
        );
    }

    // ── Opacity / width (profile fills whole pane) ────────────
    set!(
        "opacity",
        ParameterValue::Float(cfg.opacity as f64)
    );
    // Profile fills most of the pane width.
    set!("width_pct", ParameterValue::Float(0.90));

    // ── Colors ────────────────────────────────────────────────
    if let Some(c) = cfg.volume_color {
        set!(
            "volume_color",
            ParameterValue::Color(c.into())
        );
    }
    if let Some(c) = cfg.bid_color {
        set!("bid_color", ParameterValue::Color(c.into()));
    }
    if let Some(c) = cfg.ask_color {
        set!("ask_color", ParameterValue::Color(c.into()));
    }

    // ── POC ───────────────────────────────────────────────────
    set!(
        "poc_show",
        ParameterValue::Boolean(cfg.show_poc)
    );
    if let Some(c) = cfg.poc_color {
        set!("poc_color", ParameterValue::Color(c.into()));
    }
    set!(
        "poc_line_width",
        ParameterValue::Float(cfg.poc_line_width as f64)
    );
    set!(
        "poc_line_style",
        ParameterValue::LineStyle(
            to_study_line_style(cfg.poc_line_style),
        )
    );
    set!(
        "poc_extend",
        ParameterValue::Choice(
            extend_to_str(cfg.poc_extend).into(),
        )
    );
    set!(
        "poc_show_label",
        ParameterValue::Boolean(cfg.show_poc_label)
    );

    // ── Value Area ────────────────────────────────────────────
    set!(
        "va_show",
        ParameterValue::Boolean(cfg.show_va_highlight)
    );
    set!(
        "value_area_pct",
        ParameterValue::Float(cfg.value_area_pct as f64)
    );
    set!(
        "va_show_highlight",
        ParameterValue::Boolean(cfg.show_va_highlight)
    );
    if let Some(c) = cfg.vah_color {
        set!("va_vah_color", ParameterValue::Color(c.into()));
    }
    set!(
        "va_vah_line_width",
        ParameterValue::Float(cfg.vah_line_width as f64)
    );
    set!(
        "va_vah_line_style",
        ParameterValue::LineStyle(
            to_study_line_style(cfg.vah_line_style),
        )
    );
    if let Some(c) = cfg.val_color {
        set!("va_val_color", ParameterValue::Color(c.into()));
    }
    set!(
        "va_val_line_width",
        ParameterValue::Float(cfg.val_line_width as f64)
    );
    set!(
        "va_val_line_style",
        ParameterValue::LineStyle(
            to_study_line_style(cfg.val_line_style),
        )
    );
    set!(
        "va_extend",
        ParameterValue::Choice(
            extend_to_str(cfg.va_extend).into(),
        )
    );
    set!(
        "va_show_labels",
        ParameterValue::Boolean(cfg.show_va_labels)
    );

    // VA fill
    set!(
        "va_show_fill",
        ParameterValue::Boolean(cfg.show_va_fill)
    );
    if let Some(c) = cfg.va_fill_color {
        set!(
            "va_fill_color",
            ParameterValue::Color(c.into())
        );
    }
    set!(
        "va_fill_opacity",
        ParameterValue::Float(cfg.va_fill_opacity as f64)
    );

    // ── Node detection ────────────────────────────────────────
    let hvn_method = match cfg.hvn_method {
        NM::Percentile => "Percentile",
        NM::Relative => "Relative",
        NM::StdDev => "Std Dev",
    };
    let lvn_method = match cfg.lvn_method {
        NM::Percentile => "Percentile",
        NM::Relative => "Relative",
        NM::StdDev => "Std Dev",
    };
    set!(
        "node_hvn_method",
        ParameterValue::Choice(hvn_method.into())
    );
    set!(
        "node_hvn_threshold",
        ParameterValue::Float(cfg.hvn_threshold as f64)
    );
    set!(
        "node_lvn_method",
        ParameterValue::Choice(lvn_method.into())
    );
    set!(
        "node_lvn_threshold",
        ParameterValue::Float(cfg.lvn_threshold as f64)
    );

    // HVN zones
    set!(
        "hvn_zone_show",
        ParameterValue::Boolean(cfg.show_hvn_zones)
    );
    if let Some(c) = cfg.hvn_zone_color {
        set!(
            "hvn_zone_color",
            ParameterValue::Color(c.into())
        );
    }
    set!(
        "hvn_zone_opacity",
        ParameterValue::Float(cfg.hvn_zone_opacity as f64)
    );

    // LVN zones
    set!(
        "lvn_zone_show",
        ParameterValue::Boolean(cfg.show_lvn_zones)
    );
    if let Some(c) = cfg.lvn_zone_color {
        set!(
            "lvn_zone_color",
            ParameterValue::Color(c.into())
        );
    }
    set!(
        "lvn_zone_opacity",
        ParameterValue::Float(cfg.lvn_zone_opacity as f64)
    );

    // Peak
    set!(
        "peak_show",
        ParameterValue::Boolean(cfg.show_peak_line)
    );
    if let Some(c) = cfg.peak_color {
        set!("peak_color", ParameterValue::Color(c.into()));
    }
    set!(
        "peak_line_style",
        ParameterValue::LineStyle(
            to_study_line_style(cfg.peak_line_style),
        )
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
    set!(
        "valley_show",
        ParameterValue::Boolean(cfg.show_valley_line)
    );
    if let Some(c) = cfg.valley_color {
        set!(
            "valley_color",
            ParameterValue::Color(c.into())
        );
    }
    set!(
        "valley_line_style",
        ParameterValue::LineStyle(
            to_study_line_style(cfg.valley_line_style),
        )
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
fn to_study_line_style(
    s: data::state::pane::ProfileLineStyle,
) -> study::config::LineStyleValue {
    use data::state::pane::ProfileLineStyle as P;
    match s {
        P::Solid => study::config::LineStyleValue::Solid,
        P::Dashed => study::config::LineStyleValue::Dashed,
        P::Dotted => study::config::LineStyleValue::Dotted,
    }
}

/// Convert a `ProfileExtendDirection` to the string the VBP study
/// understands.
fn extend_to_str(
    e: data::state::pane::ProfileExtendDirection,
) -> &'static str {
    use data::state::pane::ProfileExtendDirection as E;
    match e {
        E::None => "None",
        E::Left => "Left",
        E::Right => "Right",
        E::Both => "Both",
    }
}

/// Compute initial price scale from profile data.
fn compute_initial_price_scale(
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
