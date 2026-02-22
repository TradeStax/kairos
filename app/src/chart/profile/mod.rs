mod render;
mod studies;

use crate::chart::{
    Chart, PlotLimits, ViewState,
    drawing::{ChartDrawingAccess, DrawingManager},
};
use data::state::pane::{ProfileConfig, ProfilePeriod};
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
        Vector::new(chart.translation.x, chart.translation.y)
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
            max_cell_width: 1.0,
            min_cell_width: 1.0,
            max_cell_height: 200.0,
            min_cell_height: 0.1,
            default_cell_width: 1.0,
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

        // latest_x is not meaningful for profile, but ViewState needs it
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
            1.0, // cell_width fixed at 1.0
            cell_height,
        );
        chart.base_price_y = base_price_y;
        chart.latest_x = latest_x;
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

    /// Extract both the profile output and its render config.
    fn profile_and_config(
        &self,
    ) -> Option<(
        &ProfileOutput,
        &study::output::ProfileRenderConfig,
    )> {
        match self.profile_study.output() {
            StudyOutput::Profile(profiles, config) => {
                profiles.first().map(|p| (p, config))
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
        // Compute fingerprint (scoped borrow so we can mutate
        // self.fingerprint afterwards).
        let fp = {
            let (cs, ts) = resolve_data_slice(
                &self.chart_data,
                &self.display_config,
            );
            (
                ts.len(),
                ts.first().map(|t| t.time.0).unwrap_or(0),
                ts.last().map(|t| t.time.0).unwrap_or(0),
                cs.len(),
            )
        };
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

        // Feed sliced data into the study. We borrow chart_data
        // separately from profile_study for disjoint access.
        let (candle_slice, trade_slice) = resolve_data_slice(
            &self.chart_data,
            &self.display_config,
        );
        let trades: Option<&[Trade]> =
            if !trade_slice.is_empty() {
                Some(trade_slice)
            } else {
                None
            };
        let input = StudyInput {
            candles: candle_slice,
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

    /// Resolve the data slice based on period settings.
    fn resolve_data_slice(&self) -> (&[Candle], &[Trade]) {
        resolve_data_slice(
            &self.chart_data,
            &self.display_config,
        )
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

/// Resolve the candle/trade data slice based on period settings.
///
/// Free function to allow disjoint borrows (chart_data vs study).
fn resolve_data_slice<'a>(
    data: &'a ChartData,
    cfg: &ProfileConfig,
) -> (&'a [Candle], &'a [Trade]) {
    match cfg.period {
        ProfilePeriod::AllData => {
            (&data.candles, &data.trades)
        }
        ProfilePeriod::Length => {
            let cutoff_ms = compute_length_cutoff(data, cfg);
            let cs = data
                .candles
                .partition_point(|c| c.time.0 < cutoff_ms);
            let ts = data
                .trades
                .partition_point(|t| t.time.0 < cutoff_ms);
            (&data.candles[cs..], &data.trades[ts..])
        }
        ProfilePeriod::Custom => {
            let start = cfg.custom_start as u64;
            let end = cfg.custom_end as u64;
            if start == 0 && end == 0 {
                return (&data.candles, &data.trades);
            }
            let cs =
                data.candles.partition_point(|c| c.time.0 < start);
            let ce =
                data.candles.partition_point(|c| c.time.0 <= end);
            let ts =
                data.trades.partition_point(|t| t.time.0 < start);
            let te =
                data.trades.partition_point(|t| t.time.0 <= end);
            (&data.candles[cs..ce], &data.trades[ts..te])
        }
    }
}

fn compute_length_cutoff(
    data: &ChartData,
    cfg: &ProfileConfig,
) -> u64 {
    use data::state::pane::ProfileLengthUnit;
    let latest_ms = data
        .candles
        .last()
        .map(|c| c.time.0)
        .or_else(|| data.trades.last().map(|t| t.time.0))
        .unwrap_or(0);

    match cfg.length_unit {
        ProfileLengthUnit::Days => {
            let ms =
                cfg.length_value as u64 * 24 * 60 * 60 * 1000;
            latest_ms.saturating_sub(ms)
        }
        ProfileLengthUnit::Minutes => {
            let ms = cfg.length_value as u64 * 60 * 1000;
            latest_ms.saturating_sub(ms)
        }
        ProfileLengthUnit::Contracts => {
            let target = cfg.length_value as f64;
            let mut accum = 0.0;
            for trade in data.trades.iter().rev() {
                accum += trade.quantity.value();
                if accum >= target {
                    return trade.time.0;
                }
            }
            0
        }
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

    // ── Period: always "Custom" (caller pre-slices data) ──────
    set!("period", ParameterValue::Choice("Custom".into()));

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
