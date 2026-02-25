use super::{Action, Content, State, TickAction};
use crate::chart::{
    candlestick::KlineChart, comparison::ComparisonChart, heatmap::HeatmapChart,
    profile::ProfileChart,
};
use crate::components::display::toast::Toast;
use data::{
    ChartBasis, ChartConfig, ChartData, ContentKind, DataSchema, DateRange, LoadingStatus,
    Timeframe, VisualConfig,
};
use exchange::FuturesTickerInfo;
use std::time::Instant;

impl State {
    /// Set chart data (called by dashboard after loading)
    pub fn set_chart_data(&mut self, chart_data: ChartData) {
        self.chart_data = Some(chart_data.clone());
        self.loading_status = LoadingStatus::Ready;

        let ticker_info = match self.ticker_info {
            Some(ti) => ti,
            None => return,
        };

        let basis = self
            .settings
            .selected_basis
            .unwrap_or(ChartBasis::Time(Timeframe::M5));

        match &mut self.content {
            Content::Candlestick {
                chart,
                layout,
                study_ids,
            } => {
                let mut new_chart = KlineChart::from_chart_data(
                    chart_data,
                    basis,
                    ticker_info,
                    layout.clone(),
                );
                if !self.settings.drawings.is_empty() {
                    new_chart
                        .drawings_mut()
                        .load_drawings(self.settings.drawings.clone());
                }
                // Apply saved candle style from visual config
                if let Some(VisualConfig::Kline(ref kline_cfg)) = self.settings.visual_config {
                    new_chart.set_candle_style(kline_cfg.candle_style.clone());
                }
                // Restore active studies with saved parameters
                let registry = crate::app::init::services::create_unified_registry();
                if !self.settings.studies.is_empty() {
                    for cfg in &self.settings.studies {
                        if let Some(mut s) = registry.create(&cfg.study_id) {
                            for (key, json_val) in &cfg.parameters {
                                if let Ok(pv) = serde_json::from_value::<study::ParameterValue>(
                                    json_val.clone(),
                                ) {
                                    if let Err(e) = s.set_parameter(key, pv) {
                                        log::warn!(
                                            "Failed to set study parameter: {}",
                                            e
                                        );
                                    }
                                }
                            }
                            new_chart.add_study(s);
                        }
                    }
                    *study_ids = self
                        .settings
                        .studies
                        .iter()
                        .filter(|c| c.enabled)
                        .map(|c| c.study_id.clone())
                        .collect();
                } else {
                    for sid in study_ids.iter() {
                        if let Some(s) = registry.create(sid) {
                            new_chart.add_study(s);
                        }
                    }
                }
                *chart = Some(new_chart);
            }
            Content::Heatmap {
                chart,
                indicators,
                layout,
                studies,
            } => {
                let chart_studies: Vec<crate::chart::heatmap::HeatmapStudy> = studies
                    .iter()
                    .map(|s| match s {
                        data::domain::chart::heatmap::HeatmapStudy::VolumeProfile(kind) => {
                            crate::chart::heatmap::HeatmapStudy::VolumeProfile(*kind)
                        }
                    })
                    .collect();
                let mut new_chart = HeatmapChart::from_chart_data(
                    chart_data,
                    basis,
                    ticker_info,
                    layout.clone(),
                    indicators,
                    chart_studies,
                );
                if !self.settings.drawings.is_empty() {
                    new_chart
                        .drawings_mut()
                        .load_drawings(self.settings.drawings.clone());
                }
                *chart = Some(new_chart);
            }
            Content::Comparison(chart_opt) => match chart_opt {
                Some(chart) => {
                    if let Err(e) = chart.add_ticker(&ticker_info, chart_data) {
                        self.notifications.push(Toast::warn(format!(
                            "Failed to add {}: {}",
                            ticker_info.ticker.as_str(),
                            e
                        )));
                    } else {
                        self.loading_status = LoadingStatus::Ready;
                    }
                }
                None => {
                    let config = self.settings.visual_config.as_ref().and_then(|vc| {
                        if let VisualConfig::Comparison(cfg) = vc {
                            Some(cfg.clone())
                        } else {
                            None
                        }
                    });

                    let new_chart = ComparisonChart::from_multi_chart_data(
                        vec![(ticker_info, chart_data)],
                        basis,
                        config,
                    );
                    *chart_opt = Some(new_chart);
                    self.loading_status = LoadingStatus::Ready;
                }
            },
            Content::Profile {
                chart,
                layout,
                study_ids,
            } => {
                let profile_config = self
                    .settings
                    .visual_config
                    .as_ref()
                    .and_then(|vc| vc.clone().profile())
                    .unwrap_or_default();

                let mut new_chart = ProfileChart::from_chart_data(
                    chart_data,
                    ticker_info,
                    layout.clone(),
                    profile_config,
                );
                if !self.settings.drawings.is_empty() {
                    new_chart
                        .drawings_mut()
                        .load_drawings(self.settings.drawings.clone());
                }
                // Restore active studies with saved parameters
                let registry = crate::app::init::services::create_unified_registry();
                if !self.settings.studies.is_empty() {
                    for cfg in &self.settings.studies {
                        if let Some(mut s) = registry.create(&cfg.study_id) {
                            for (key, json_val) in &cfg.parameters {
                                if let Ok(pv) =
                                    serde_json::from_value::<study::ParameterValue>(
                                        json_val.clone(),
                                    )
                                {
                                    if let Err(e) = s.set_parameter(key, pv) {
                                        log::warn!(
                                            "Failed to set study parameter: {}",
                                            e
                                        );
                                    }
                                }
                            }
                            new_chart.add_study(s);
                        }
                    }
                    *study_ids = self
                        .settings
                        .studies
                        .iter()
                        .filter(|c| c.enabled)
                        .map(|c| c.study_id.clone())
                        .collect();
                } else {
                    for sid in study_ids.iter() {
                        if let Some(s) = registry.create(sid) {
                            new_chart.add_study(s);
                        }
                    }
                }
                *chart = Some(new_chart);
            }
            Content::TimeAndSales(_panel) => {}
            Content::Ladder(_panel) => {}
            Content::Starter
            | Content::AiAssistant(_) => {}
        }
    }

    /// Set content and request chart loading with specified date range
    pub fn set_content_with_range(
        &mut self,
        ticker_info: FuturesTickerInfo,
        kind: ContentKind,
        date_range: DateRange,
    ) -> Action {
        let basis = self
            .settings
            .selected_basis
            .unwrap_or(ChartBasis::Time(Timeframe::M5));

        self.ticker_info = Some(ticker_info);
        self.loaded_date_range = Some(date_range);
        self.content = Content::new_for_kind(kind, ticker_info, &self.settings);

        let days_total = date_range.num_days() as usize;
        self.loading_status = LoadingStatus::LoadingFromCache {
            schema: DataSchema::Trades,
            days_total,
            days_loaded: 0,
            items_loaded: 0,
        };

        let config = ChartConfig {
            ticker: ticker_info.ticker,
            basis,
            date_range,
            chart_type: kind.to_chart_type(),
        };

        Action::LoadChart {
            config,
            ticker_info,
        }
    }

    pub fn invalidate(&mut self, now: Instant) -> Option<TickAction> {
        match &mut self.content {
            Content::Heatmap { chart, .. } => {
                if let Some(c) = chart.as_mut() {
                    c.invalidate(Some(now));
                }
                None
            }
            Content::Candlestick { chart, .. } => {
                if let Some(c) = chart.as_mut() {
                    c.invalidate()
                }
                None
            }
            Content::TimeAndSales(panel) => panel
                .as_mut()
                .and_then(|p| p.invalidate(Some(now)).map(TickAction::Panel)),
            Content::Ladder(panel) => panel
                .as_mut()
                .and_then(|p| p.invalidate(Some(now)).map(TickAction::Panel)),
            Content::Profile { chart, .. } => {
                if let Some(c) = chart.as_mut() {
                    c.invalidate();
                }
                None
            }
            Content::Starter | Content::AiAssistant(_) => None,
            Content::Comparison(_) => None,
        }
    }

    /// Enter replay mode: back up current chart data and clear the chart.
    pub fn enter_replay_mode(&mut self) {
        if let Some(chart_data) = self.chart_data.clone() {
            self.replay_backup = Some(chart_data);
        }
        // Clear the chart to build candles from scratch during replay
        let empty = ChartData::from_trades(vec![], vec![]);
        self.set_chart_data(empty);
    }

    /// Exit replay mode: restore the backed-up chart data.
    pub fn exit_replay_mode(&mut self) {
        if let Some(backup) = self.replay_backup.take() {
            self.set_chart_data(backup);
        }
    }

    /// Whether this pane is currently in replay mode.
    pub fn is_replaying(&self) -> bool {
        self.replay_backup.is_some()
    }

    pub fn update_interval(&self) -> Option<u64> {
        // Faster invalidation during replay for smoother updates
        if self.replay_backup.is_some() {
            return Some(200);
        }
        match &self.content {
            Content::Candlestick { .. }
            | Content::Comparison(_)
            | Content::Profile { .. } => Some(1000),
            Content::Heatmap { chart, .. } => {
                if let Some(chart) = chart {
                    chart.basis_interval()
                } else {
                    None
                }
            }
            Content::Ladder(_) | Content::TimeAndSales(_) => Some(100),
            Content::Starter
            | Content::AiAssistant(_) => None,
        }
    }

    pub fn last_tick(&self) -> Option<Instant> {
        self.content.last_tick()
    }

    pub fn tick(&mut self, now: Instant) -> Option<TickAction> {
        let invalidate_interval: Option<u64> = self.update_interval();
        let last_tick: Option<Instant> = self.last_tick();

        if !self.content.initialized() {
            return None;
        }

        match (invalidate_interval, last_tick) {
            (Some(interval_ms), Some(previous_tick_time)) => {
                if interval_ms > 0 {
                    let interval_duration = std::time::Duration::from_millis(interval_ms);
                    if now.duration_since(previous_tick_time) >= interval_duration {
                        return self.invalidate(now);
                    }
                }
            }
            (Some(interval_ms), None) => {
                if interval_ms > 0 {
                    return self.invalidate(now);
                }
            }
            (None, _) => {}
        }

        None
    }
}
