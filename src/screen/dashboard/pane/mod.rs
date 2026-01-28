mod content;
mod effects;
mod helpers;
mod view;

pub use content::Content;
pub use effects::Effect;

use crate::{
    chart::{self, comparison::ComparisonChart, heatmap::HeatmapChart, candlestick::KlineChart},
    modal::{
        self,
        pane::{
            Modal,
            mini_tickers_list::MiniPanel,
            settings::{comparison_cfg_view, heatmap_cfg_view, kline_cfg_view},
        },
    },
    screen::dashboard::panel::{self, ladder::Ladder, timeandsales::TimeAndSales},
    widget::toast::Toast,
};
use data::{
    ChartBasis, ChartConfig, ChartData, ChartType, ContentKind, DataSchema, DateRange,
    KlineIndicator, HeatmapIndicator, LinkGroup, LoadingStatus, Settings, Timeframe,
    UiIndicator, ViewConfig, VisualConfig,
};
use exchange::FuturesTickerInfo;
use iced::widget::pane_grid;
use std::time::Instant;

pub enum Action {
    Chart(chart::Action),
    Panel(panel::Action),
    LoadChart {
        config: ChartConfig,
        ticker_info: FuturesTickerInfo,
    },
}

#[derive(Debug, Clone)]
pub enum Message {
    PaneClicked(pane_grid::Pane),
    PaneResized(pane_grid::ResizeEvent),
    PaneDragged(pane_grid::DragEvent),
    ClosePane(pane_grid::Pane),
    SplitPane(pane_grid::Axis, pane_grid::Pane),
    MaximizePane(pane_grid::Pane),
    Restore,
    ReplacePane(pane_grid::Pane),
    Popout,
    Merge,
    SwitchLinkGroup(pane_grid::Pane, Option<LinkGroup>),
    VisualConfigChanged(pane_grid::Pane, VisualConfig, bool),
    PaneEvent(pane_grid::Pane, Event),
}

#[derive(Debug, Clone)]
pub enum Event {
    ShowModal(Modal),
    HideModal,
    ContentSelected(ContentKind),
    ChartInteraction(super::chart::Message),
    PanelInteraction(super::panel::Message),
    ToggleIndicator(UiIndicator),
    DeleteNotification(usize),
    ReorderIndicator(crate::widget::column_drag::DragEvent),
    DataManagementInteraction(crate::modal::pane::data_management::DataManagementMessage),
    ClusterKindSelected(data::ClusterKind),
    ClusterScalingSelected(data::ClusterScaling),
    StudyConfigurator(modal::pane::settings::study::StudyMessage),
    StreamModifierChanged(modal::stream::Message),
    ComparisonChartInteraction(super::chart::comparison::Message),
    MiniTickersListInteraction(modal::pane::mini_tickers_list::Message),
}

pub struct State {
    id: uuid::Uuid,
    pub modal: Option<Modal>,
    pub content: Content,
    pub settings: Settings,
    pub notifications: Vec<Toast>,
    pub loading_status: LoadingStatus,
    pub ticker_info: Option<FuturesTickerInfo>,
    pub chart_data: Option<ChartData>,
    pub link_group: Option<LinkGroup>,
}

impl State {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_config(
        content: Content,
        settings: Settings,
        link_group: Option<LinkGroup>,
        ticker_info: Option<FuturesTickerInfo>,
    ) -> Self {
        Self {
            content,
            settings,
            ticker_info,
            link_group,
            ..Default::default()
        }
    }

    /// Set chart data (called by dashboard after loading)
    pub fn set_chart_data(&mut self, chart_data: ChartData) {
        self.chart_data = Some(chart_data.clone());
        self.loading_status = LoadingStatus::Ready;

        let ticker_info = match self.ticker_info {
            Some(ti) => ti,
            None => return, // No ticker info, can't create chart
        };

        let basis = self
            .settings
            .selected_basis
            .unwrap_or(ChartBasis::Time(Timeframe::M15));

        // Update content with the chart data
        match &mut self.content {
            Content::Kline {
                chart,
                indicators,
                kind,
                layout,
            } => {
                *chart = Some(KlineChart::from_chart_data(
                    chart_data,
                    basis,
                    ticker_info,
                    layout.clone(),
                    indicators,
                    kind.clone(),
                ));
            }
            Content::Heatmap {
                chart,
                indicators,
                layout,
                studies,
            } => {
                // Convert data::HeatmapStudy to chart::heatmap::HeatmapStudy
                let chart_studies: Vec<crate::chart::heatmap::HeatmapStudy> = studies
                    .iter()
                    .map(|s| match s {
                        data::domain::chart_ui_types::heatmap::HeatmapStudy::VolumeProfile(kind) => {
                            crate::chart::heatmap::HeatmapStudy::VolumeProfile(*kind)
                        }
                    })
                    .collect();
                log::info!("🟢 Constructing HeatmapChart from chart_data: {} trades, {} candles, {} depth snapshots",
                    chart_data.trades.len(),
                    chart_data.candles.len(),
                    chart_data.depth_snapshots.as_ref().map(|d| d.len()).unwrap_or(0)
                );

                *chart = Some(HeatmapChart::from_chart_data(
                    chart_data,
                    basis,
                    ticker_info,
                    layout.clone(),
                    indicators,
                    chart_studies,
                ));

                log::info!("🟢 HeatmapChart construction COMPLETE");
            }
            Content::Comparison(chart_opt) => {
                // Initialize or add ticker to comparison chart
                match chart_opt {
                    Some(chart) => {
                        // Add new ticker to existing comparison chart
                        if let Err(e) = chart.add_ticker(&ticker_info, chart_data) {
                            log::warn!("Failed to add ticker to comparison: {}", e);
                            self.notifications.push(Toast::warn(format!(
                                "Failed to add {}: {}",
                                ticker_info.ticker.as_str(),
                                e
                            )));
                        } else {
                            log::info!("Added ticker {} to comparison chart", ticker_info.ticker.as_str());
                            self.loading_status = LoadingStatus::Ready;
                        }
                    }
                    None => {
                        // Create new comparison chart with first ticker
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
                        log::info!("Created comparison chart with ticker {}", ticker_info.ticker.as_str());
                    }
                }
            }
            Content::TimeAndSales(_panel) => {
                // TimeAndSales panel doesn't need chart data
            }
            Content::Ladder(_panel) => {
                // Ladder panel doesn't need chart data
            }
            Content::Starter => {}
        }
    }

    pub fn get_ticker(&self) -> Option<FuturesTickerInfo> {
        self.ticker_info
    }

    /// Set content and request chart loading with specified date range
    pub fn set_content_with_range(
        &mut self,
        ticker_info: FuturesTickerInfo,
        kind: ContentKind,
        date_range: DateRange,
    ) -> Effect {
        log::info!("PANE: set_content_with_range called with {:?} ContentKind::{:?}, range {} to {}",
            ticker_info.ticker, kind, date_range.start, date_range.end);

        // Determine basis (time or tick)
        let basis = self
            .settings
            .selected_basis
            .unwrap_or(ChartBasis::Time(Timeframe::M15));

        // Set ticker info
        self.ticker_info = Some(ticker_info);

        // Initialize content (empty for now, will be populated when data loads)
        self.content = Content::new_for_kind(kind, ticker_info, &self.settings);

        // Set loading status to show "Loading..." in UI
        let days_total = date_range.num_days() as usize;
        self.loading_status = LoadingStatus::LoadingFromCache {
            schema: DataSchema::Trades, // Default to Trades
            days_total,
            days_loaded: 0,
            items_loaded: 0,
        };

        // Create chart config with registered date range
        let config = ChartConfig {
            ticker: ticker_info.ticker,
            basis,
            date_range, // Use registered range from downloaded_tickers
            chart_type: kind.to_chart_type(),
        };

        // Request chart loading
        Effect::LoadChart {
            config,
            ticker_info,
        }
    }

    /// Set content and request chart loading (legacy - uses default 1 day)
    pub fn set_content(&mut self, ticker_info: FuturesTickerInfo, kind: ContentKind) -> Effect {
        self.set_content_with_range(ticker_info, kind, DateRange::last_n_days(1))
    }

    pub fn update(&mut self, msg: Event) -> Option<Effect> {
        match msg {
            Event::ShowModal(requested_modal) => {
                return self.show_modal_with_focus(requested_modal);
            }
            Event::HideModal => {
                self.modal = None;
            }
            Event::ContentSelected(kind) => {
                self.content = Content::placeholder(kind);

                if !matches!(kind, ContentKind::Starter) {
                    let modal = Modal::MiniTickersList(MiniPanel::new());

                    if let Some(effect) = self.show_modal_with_focus(modal) {
                        return Some(effect);
                    }
                }
            }
            Event::ChartInteraction(msg) => match &mut self.content {
                Content::Heatmap { chart: Some(c), .. } => {
                    super::chart::update(c, &msg);
                }
                Content::Kline { chart: Some(c), .. } => {
                    super::chart::update(c, &msg);
                }
                _ => {}
            },
            Event::PanelInteraction(msg) => match &mut self.content {
                Content::Ladder(Some(p)) => super::panel::update(p, msg),
                Content::TimeAndSales(Some(p)) => super::panel::update(p, msg),
                _ => {}
            },
            Event::ToggleIndicator(ind) => {
                self.content.toggle_indicator(ind);
            }
            Event::DeleteNotification(idx) => {
                if idx < self.notifications.len() {
                    self.notifications.remove(idx);
                }
            }
            Event::ReorderIndicator(e) => {
                self.content.reorder_indicators(&e);
            }
            Event::ClusterKindSelected(kind) => {
                if let Content::Kline {
                    chart, kind: cur, ..
                } = &mut self.content
                    && let Some(c) = chart
                {
                    c.set_cluster_kind(kind);
                    *cur = c.kind().clone();
                }
            }
            Event::ClusterScalingSelected(scaling) => {
                if let Content::Kline { chart, kind, .. } = &mut self.content
                    && let Some(c) = chart
                {
                    c.set_cluster_scaling(scaling);
                    *kind = c.kind().clone();
                }
            }
            Event::StudyConfigurator(study_msg) => match study_msg {
                modal::pane::settings::study::StudyMessage::Footprint(m) => {
                    if let Content::Kline { chart, kind, .. } = &mut self.content
                        && let Some(c) = chart
                    {
                        c.update_study_configurator(m);
                        *kind = c.kind().clone();
                    }
                }
                modal::pane::settings::study::StudyMessage::Heatmap(m) => {
                    if let Content::Heatmap { chart, studies, .. } = &mut self.content
                        && let Some(c) = chart
                    {
                        c.update_study_configurator(m);
                        // Convert chart studies back to data studies
                        *studies = c
                            .studies
                            .iter()
                            .map(|s| match s {
                                crate::chart::heatmap::HeatmapStudy::VolumeProfile(kind) => {
                                    data::domain::chart_ui_types::heatmap::HeatmapStudy::VolumeProfile(
                                        *kind,
                                    )
                                }
                            })
                            .collect();
                    }
                }
            },
            Event::StreamModifierChanged(message) => {
                if let Some(Modal::StreamModifier(modifier)) = self.modal.take() {
                    let mut modifier = modifier;

                    if let Some(action) = modifier.update(message) {
                        match action {
                            modal::stream::Action::TabSelected(tab) => {
                                modifier.tab = tab;
                            }
                            // TicksizeSelected removed - tick multiplier only for crypto
                            modal::stream::Action::BasisSelected(new_basis) => {
                                modifier.update_kind_with_basis(new_basis);
                                self.settings.selected_basis = Some(new_basis);

                                // When basis changes, we need to re-aggregate the trades
                                // This is instant because trades are already in memory
                                match &mut self.content {
                                    Content::Heatmap { chart: Some(c), .. } => {
                                        c.set_basis(new_basis);
                                    }
                                    Content::Kline { chart: Some(c), .. } => {
                                        if let Some(ticker) = self.ticker_info {
                                            c.switch_basis(new_basis, ticker);
                                        }
                                    }
                                    Content::Comparison(_) => {
                                        // ComparisonChart doesn't support dynamic basis switching
                                        // Would require reloading data
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }

                    self.modal = Some(Modal::StreamModifier(modifier));
                }
            }
            Event::ComparisonChartInteraction(message) => {
                if let Content::Comparison(chart_opt) = &mut self.content
                    && let Some(chart) = chart_opt
                    && let Some(action) = chart.update(message)
                {
                    match action {
                        super::chart::comparison::Action::SeriesColorChanged(t, color) => {
                            chart.set_series_color(t, color);
                        }
                        super::chart::comparison::Action::SeriesNameChanged(t, name) => {
                            chart.set_series_name(t, name);
                        }
                        super::chart::comparison::Action::OpenSeriesEditor => {
                            self.modal = Some(Modal::Settings);
                        }
                        super::chart::comparison::Action::RemoveSeries(ticker_info) => {
                            if let Content::Comparison(Some(chart)) = &mut self.content {
                                chart.remove_ticker(&ticker_info);
                                log::info!("Removed ticker {:?} from comparison chart", ticker_info.ticker);
                            }
                        }
                    }
                }
            }
            Event::MiniTickersListInteraction(message) => {
                if let Some(Modal::MiniTickersList(ref mut mini_panel)) = self.modal
                    && let Some(action) = mini_panel.update(message)
                {
                    self.modal = Some(Modal::MiniTickersList(mini_panel.clone()));

                    let crate::modal::pane::mini_tickers_list::Action::RowSelected(sel) = action;
                    match sel {
                        crate::modal::pane::mini_tickers_list::RowSelection::Add(ticker_info) => {
                            // Add ticker to comparison chart by loading its chart data
                            log::info!("Adding ticker {:?} to comparison chart", ticker_info.ticker);

                            // Get current basis or use default
                            let basis = self.settings.selected_basis.unwrap_or(ChartBasis::Time(Timeframe::M15));

                            // Get date range - use a reasonable default
                            // TODO: Make this configurable via UI
                            let date_range = DateRange::new(
                                chrono::Local::now().date_naive() - chrono::Duration::days(7),
                                chrono::Local::now().date_naive(),
                            );

                            // Create chart config for this ticker
                            let chart_config = ChartConfig {
                                ticker: ticker_info.ticker,
                                basis,
                                date_range,
                                chart_type: ChartType::Candlestick, // Comparison uses candlestick data
                            };

                            // Trigger chart data loading
                            return Some(Effect::LoadChart {
                                config: chart_config,
                                ticker_info,
                            });
                        }
                        crate::modal::pane::mini_tickers_list::RowSelection::Remove(ticker_info) => {
                            if let Content::Comparison(Some(chart)) = &mut self.content {
                                chart.remove_ticker(&ticker_info);
                                log::info!("Removed ticker {:?} from comparison chart", ticker_info.ticker);
                            }
                        }
                        crate::modal::pane::mini_tickers_list::RowSelection::Switch(ti) => {
                            return Some(Effect::SwitchTickersInGroup(ti));
                        }
                    }
                }
            }
            Event::DataManagementInteraction(message) => {
                if let Some(Modal::DataManagement(ref mut panel)) = self.modal {
                    if let Some(action) = panel.update(message) {
                        self.modal = Some(Modal::DataManagement(panel.clone()));

                        match action {
                            crate::modal::pane::data_management::Action::EstimateRequested { ticker, schema, date_range } => {
                                log::info!("Estimate requested: {:?} {:?} {:?}", ticker, schema, date_range);
                                return Some(Effect::EstimateDataCost { ticker, schema, date_range });
                            }
                            crate::modal::pane::data_management::Action::DownloadRequested { ticker, schema, date_range } => {
                                log::info!("Download requested: {:?} {:?} {:?}", ticker, schema, date_range);
                                return Some(Effect::DownloadData { ticker, schema, date_range });
                            }
                        }
                    }
                }
            }
        }
        None
    }

    fn show_modal_with_focus(&mut self, requested_modal: Modal) -> Option<Effect> {
        let should_toggle_close = match (&self.modal, &requested_modal) {
            (Some(Modal::StreamModifier(open)), Modal::StreamModifier(req)) => {
                open.view_mode == req.view_mode
            }
            (Some(open), req) => core::mem::discriminant(open) == core::mem::discriminant(req),
            _ => false,
        };

        if should_toggle_close {
            self.modal = None;
            return None;
        }

        let focus_widget_id = match &requested_modal {
            Modal::MiniTickersList(m) => Some(m.search_box_id.clone()),
            _ => None,
        };

        self.modal = Some(requested_modal);
        focus_widget_id.map(Effect::FocusWidget)
    }

    pub fn invalidate(&mut self, now: Instant) -> Option<Action> {
        match &mut self.content {
            Content::Heatmap { chart, .. } => chart
                .as_mut()
                .and_then(|c| c.invalidate(Some(now)).map(Action::Chart)),
            Content::Kline { chart, .. } => {
                chart.as_mut().map(|c| c.invalidate());
                None // KlineChart::invalidate doesn't return an Action
            }
            Content::TimeAndSales(panel) => panel
                .as_mut()
                .and_then(|p| p.invalidate(Some(now)).map(Action::Panel)),
            Content::Ladder(panel) => panel
                .as_mut()
                .and_then(|p| p.invalidate(Some(now)).map(Action::Panel)),
            Content::Starter => None,
            Content::Comparison(_) => {
                // ComparisonChart doesn't have invalidate method
                None
            }
        }
    }

    pub fn update_interval(&self) -> Option<u64> {
        match &self.content {
            Content::Kline { .. } | Content::Comparison(_) => Some(1000),
            Content::Heatmap { chart, .. } => {
                if let Some(chart) = chart {
                    chart.basis_interval()
                } else {
                    None
                }
            }
            Content::Ladder(_) | Content::TimeAndSales(_) => Some(100),
            Content::Starter => None,
        }
    }

    pub fn last_tick(&self) -> Option<Instant> {
        self.content.last_tick()
    }

    pub fn tick(&mut self, now: Instant) -> Option<Action> {
        let invalidate_interval: Option<u64> = self.update_interval();
        let last_tick: Option<Instant> = self.last_tick();

        if !self.content.initialized() {
            return None; // Wait for content to be initialized
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

    pub fn unique_id(&self) -> uuid::Uuid {
        self.id
    }
}

impl Default for State {
    fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            modal: None,
            content: Content::Starter,
            settings: Settings::default(),
            notifications: vec![],
            loading_status: LoadingStatus::Idle,
            ticker_info: None,
            chart_data: None,
            link_group: None,
        }
    }
}
