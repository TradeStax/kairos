use crate::{
    chart::{self, comparison::ComparisonChart, heatmap::HeatmapChart, kline::KlineChart},
    modal::{
        self, ModifierKind,
        pane::{
            Modal,
            mini_tickers_list::MiniPanel,
            settings::{comparison_cfg_view, heatmap_cfg_view, kline_cfg_view},
            stack_modal,
        },
    },
    screen::dashboard::{
        panel::{self, ladder::Ladder, timeandsales::TimeAndSales},
        tickers_table::TickersTable,
    },
    style::{self, Icon, icon_text},
    widget::{self, button_with_tooltip, column_drag, link_group_button, toast::Toast},
    window::{self, Window},
};
use data::{
    ChartBasis, ChartConfig, ChartData, ContentKind, DateRange, FootprintStudy, FuturesTicker,
    HeatmapIndicator, KlineIndicator, LinkGroup, LoadingStatus, Settings, Timeframe, UiIndicator,
    UserTimezone, ViewConfig, VisualConfig,
};
use exchange::{FuturesTickerInfo, TickMultiplier};
use iced::{
    Alignment, Element, Length, Renderer, Theme,
    alignment::Vertical,
    padding,
    widget::{button, center, column, container, pane_grid, pick_list, row, text, tooltip},
};
use std::time::Instant;

#[derive(Debug, Clone)]
pub enum Effect {
    LoadChart {
        config: ChartConfig,
        ticker_info: FuturesTickerInfo,
    },
    SwitchTickersInGroup(FuturesTickerInfo),
    FocusWidget(iced::widget::Id),
    EstimateDataCost {
        ticker: FuturesTicker,
        schema: exchange::DatabentoSchema,
        date_range: DateRange,
    },
    DownloadData {
        ticker: FuturesTicker,
        schema: exchange::DatabentoSchema,
        date_range: DateRange,
    },
}

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
    ReorderIndicator(column_drag::DragEvent),
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
                *chart = Some(HeatmapChart::from_chart_data(
                    chart_data,
                    basis,
                    ticker_info,
                    layout.clone(),
                    indicators,
                    chart_studies,
                ));
            }
            Content::Comparison(chart_opt) => {
                // Comparison chart already loaded during construction
                // For now, just mark as ready
                if chart_opt.is_some() {
                    self.loading_status = LoadingStatus::Ready;
                }
                // Note: Adding/removing series requires separate implementation
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

    /// Set content and request chart loading
    pub fn set_content(&mut self, ticker_info: FuturesTickerInfo, kind: ContentKind) -> Effect {
        // Determine basis (time or tick)
        let basis = self
            .settings
            .selected_basis
            .unwrap_or(ChartBasis::Time(Timeframe::M15));

        // Set ticker info
        self.ticker_info = Some(ticker_info);

        // Initialize content (empty for now, will be populated when data loads)
        self.content = Content::new_for_kind(kind, ticker_info, &self.settings);
        self.loading_status = LoadingStatus::Idle;

        // Create chart config
        let config = ChartConfig {
            ticker: ticker_info.ticker,
            basis,
            date_range: DateRange::last_n_days(1), // 1 day default
            chart_type: kind.to_chart_type(),
        };

        // Request chart loading
        Effect::LoadChart {
            config,
            ticker_info,
        }
    }

    pub fn view<'a>(
        &'a self,
        id: pane_grid::Pane,
        panes: usize,
        is_focused: bool,
        maximized: bool,
        window: window::Id,
        main_window: &'a Window,
        timezone: UserTimezone,
        tickers_table: &'a TickersTable,
    ) -> pane_grid::Content<'a, Message, Theme, Renderer> {
        let mut stream_info_element = if Content::Starter == self.content {
            row![]
        } else {
            row![link_group_button(id, self.link_group, |id| {
                Message::PaneEvent(id, Event::ShowModal(Modal::LinkGroup))
            })]
        };

        // Show ticker info button if we have ticker info
        if let Some(ticker_info) = self.ticker_info {
            let exchange_icon = icon_text(style::exchange_icon(ticker_info.ticker.venue), 14);
            let symbol = ticker_info.ticker.as_str().to_string();

            let content = row![exchange_icon, text(symbol).size(14)]
                .align_y(Vertical::Center)
                .spacing(4);

            let tickers_list_btn = button(content)
                .on_press(Message::PaneEvent(
                    id,
                    Event::ShowModal(Modal::MiniTickersList(MiniPanel::new())),
                ))
                .style(|theme, status| {
                    style::button::modifier(
                        theme,
                        status,
                        !matches!(self.modal, Some(Modal::MiniTickersList(_))),
                    )
                })
                .padding([4, 10]);

            stream_info_element = stream_info_element.push(tickers_list_btn);
        } else if !matches!(self.content, Content::Starter) {
            // No ticker selected - show prompt
            let content = row![text("Choose a ticker").size(13)]
                .align_y(Alignment::Center)
                .spacing(4);

            let tickers_list_btn = button(content)
                .on_press(Message::PaneEvent(
                    id,
                    Event::ShowModal(Modal::MiniTickersList(MiniPanel::new())),
                ))
                .style(|theme, status| {
                    style::button::modifier(
                        theme,
                        status,
                        !matches!(self.modal, Some(Modal::MiniTickersList(_))),
                    )
                })
                .padding([4, 10]);

            stream_info_element = stream_info_element.push(tickers_list_btn);
        }

        let modifier: Option<modal::stream::Modifier> = self.modal.clone().and_then(|m| {
            if let Modal::StreamModifier(modifier) = m {
                Some(modifier)
            } else {
                None
            }
        });

        let compact_controls = if self.modal == Some(Modal::Controls) {
            Some(
                container(self.view_controls(id, panes, maximized, window != main_window.id))
                    .style(style::chart_modal)
                    .into(),
            )
        } else {
            None
        };

        let uninitialized_base = |kind: ContentKind| -> Element<'a, Message> {
            if self.loading_status.is_loading() {
                center(text("Loading…").size(16)).into()
            } else {
                let content = column![
                    text(kind.to_string()).size(16),
                    text("No ticker selected").size(14)
                ]
                .spacing(8)
                .align_x(Alignment::Center);

                center(content).into()
            }
        };

        let body = match &self.content {
            Content::Starter => {
                let content_picklist =
                    pick_list(ContentKind::ALL, Some(ContentKind::Starter), move |kind| {
                        Message::PaneEvent(id, Event::ContentSelected(kind))
                    });

                let base: Element<_> = widget::toast::Manager::new(
                    center(
                        column![
                            text("Choose a view to get started").size(16),
                            content_picklist
                        ]
                        .align_x(Alignment::Center)
                        .spacing(12),
                    ),
                    &self.notifications,
                    Alignment::End,
                    move |msg| Message::PaneEvent(id, Event::DeleteNotification(msg)),
                )
                .into();

                self.compose_stack_view(
                    base,
                    id,
                    None,
                    compact_controls,
                    || column![].into(),
                    None,
                    tickers_table,
                )
            }
            Content::Comparison(chart) => {
                if let Some(c) = chart {
                    let selected_basis = self
                        .settings
                        .selected_basis
                        .unwrap_or(ChartBasis::Time(Timeframe::M15));
                    let kind = ModifierKind::Comparison(selected_basis);

                    let modifiers =
                        row![basis_modifier(id, selected_basis, modifier, kind),].spacing(4);

                    stream_info_element = stream_info_element.push(modifiers);

                    let base = c.view(timezone).map(move |message| {
                        Message::PaneEvent(id, Event::ComparisonChartInteraction(message))
                    });

                    let settings_modal = || comparison_cfg_view(id, c);
                    let selected_tickers = c.selected_tickers();
                    // Use Box::leak to create a static reference for the title bar
                    let selected_tickers_static: &'static [_] = Box::leak(selected_tickers.into_boxed_slice());

                    self.compose_stack_view(
                        base,
                        id,
                        None,
                        compact_controls,
                        settings_modal,
                        Some(selected_tickers_static),
                        tickers_table,
                    )
                } else {
                    let base = uninitialized_base(ContentKind::ComparisonChart);
                    self.compose_stack_view(
                        base,
                        id,
                        None,
                        compact_controls,
                        || column![].into(),
                        None,
                        tickers_table,
                    )
                }
            }
            Content::TimeAndSales(panel) => {
                if let Some(panel) = panel {
                    let base = panel::view(panel, timezone).map(move |message| {
                        Message::PaneEvent(id, Event::PanelInteraction(message))
                    });

                    let settings_modal =
                        || modal::pane::settings::timesales_cfg_view(panel.config.clone(), id);

                    self.compose_stack_view(
                        base,
                        id,
                        None,
                        compact_controls,
                        settings_modal,
                        None,
                        tickers_table,
                    )
                } else {
                    let base = uninitialized_base(ContentKind::TimeAndSales);
                    self.compose_stack_view(
                        base,
                        id,
                        None,
                        compact_controls,
                        || column![].into(),
                        None,
                        tickers_table,
                    )
                }
            }
            Content::Ladder(panel) => {
                if let Some(panel) = panel {
                    let basis = self
                        .settings
                        .selected_basis
                        .unwrap_or(ChartBasis::Time(Timeframe::M5));
                    let tick_multiply = TickMultiplier(1); // Default for ladder

                    let kind = ModifierKind::Orderbook(basis, tick_multiply);

                    let base_ticksize = panel.tick_size();
                    let exchange = self.ticker_info.map(|ti| ti.ticker.venue);

                    let modifiers = ticksize_modifier(
                        id,
                        base_ticksize,
                        tick_multiply,
                        modifier,
                        kind,
                        exchange,
                    );

                    stream_info_element = stream_info_element.push(modifiers);

                    let base = panel::view(panel, timezone).map(move |message| {
                        Message::PaneEvent(id, Event::PanelInteraction(message))
                    });

                    let settings_modal =
                        || modal::pane::settings::ladder_cfg_view(panel.config.clone(), id);

                    self.compose_stack_view(
                        base,
                        id,
                        None,
                        compact_controls,
                        settings_modal,
                        None,
                        tickers_table,
                    )
                } else {
                    let base = uninitialized_base(ContentKind::Ladder);
                    self.compose_stack_view(
                        base,
                        id,
                        None,
                        compact_controls,
                        || column![].into(),
                        None,
                        tickers_table,
                    )
                }
            }
            Content::Heatmap {
                chart, indicators, ..
            } => {
                if let Some(chart) = chart {
                    let ticker_info = self.ticker_info;
                    let exchange = ticker_info.as_ref().map(|info| info.ticker.venue);

                    let basis = self
                        .settings
                        .selected_basis
                        .unwrap_or(ChartBasis::Time(Timeframe::M5));
                    let tick_multiply = TickMultiplier(5); // Default for heatmap

                    let kind = ModifierKind::Heatmap(basis, tick_multiply);
                    let base_ticksize = chart.tick_size();

                    let modifiers = row![
                        basis_modifier(id, basis, modifier, kind),
                        ticksize_modifier(
                            id,
                            base_ticksize,
                            tick_multiply,
                            modifier,
                            kind,
                            exchange
                        ),
                    ]
                    .spacing(4);

                    stream_info_element = stream_info_element.push(modifiers);

                    let base = chart::view(chart, indicators, timezone).map(move |message| {
                        Message::PaneEvent(id, Event::ChartInteraction(message))
                    });
                    let settings_modal = || {
                        // Convert chart::heatmap::VisualConfig to data::HeatmapConfig
                        let visual = chart.visual_config();
                        let cfg = data::state::pane_config::HeatmapConfig {
                            trade_size_filter: visual.trade_size_filter,
                            order_size_filter: visual.order_size_filter,
                            trade_size_scale: visual.trade_size_scale,
                            coalescing: None, // CoalesceKind is not exposed, use None
                        };
                        // Convert chart::heatmap::HeatmapStudy to data studies and leak for 'static lifetime
                        let data_studies: Vec<data::domain::chart_ui_types::heatmap::HeatmapStudy> =
                            chart
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
                        // Use Box::leak to create a static reference
                        let studies_static: &'static [_] = Box::leak(data_studies.into_boxed_slice());
                        heatmap_cfg_view(cfg, id, chart.study_configurator(), studies_static, basis)
                    };

                    let indicator_modal = if self.modal == Some(Modal::Indicators) {
                        Some(modal::pane::indicators::content_row_heatmap(
                            id,
                            indicators,
                            false, // Heatmap doesn't allow dragging
                        ))
                    } else {
                        None
                    };

                    self.compose_stack_view(
                        base,
                        id,
                        indicator_modal,
                        compact_controls,
                        settings_modal,
                        None,
                        tickers_table,
                    )
                } else {
                    let base = uninitialized_base(ContentKind::HeatmapChart);
                    self.compose_stack_view(
                        base,
                        id,
                        None,
                        compact_controls,
                        || column![].into(),
                        None,
                        tickers_table,
                    )
                }
            }
            Content::Kline {
                chart,
                indicators,
                kind: chart_kind,
                ..
            } => {
                if let Some(chart) = chart {
                    match chart_kind {
                        data::KlineChartKind::Footprint { .. } => {
                            let basis = self
                                .settings
                                .selected_basis
                                .unwrap_or(ChartBasis::Time(Timeframe::M5));
                            let tick_multiply = TickMultiplier(10); // Default for footprint

                            let kind = ModifierKind::Footprint(basis, tick_multiply);
                            let base_ticksize = chart.tick_size();

                            let exchange =
                                self.ticker_info.as_ref().map(|info| info.ticker.venue);

                            let modifiers = row![
                                basis_modifier(id, basis, modifier, kind),
                                ticksize_modifier(
                                    id,
                                    base_ticksize,
                                    tick_multiply,
                                    modifier,
                                    kind,
                                    exchange
                                ),
                            ]
                            .spacing(4);

                            stream_info_element = stream_info_element.push(modifiers);
                        }
                        data::KlineChartKind::Candles => {
                            let selected_basis = self
                                .settings
                                .selected_basis
                                .unwrap_or(ChartBasis::Time(Timeframe::M15));
                            let kind = ModifierKind::Candlestick(selected_basis);

                            let modifiers =
                                row![basis_modifier(id, selected_basis, modifier, kind),]
                                    .spacing(4);

                            stream_info_element = stream_info_element.push(modifiers);
                        }
                    }

                    let base = chart::view(chart, indicators, timezone).map(move |message| {
                        Message::PaneEvent(id, Event::ChartInteraction(message))
                    });
                    let settings_modal = || {
                        // KlineChart doesn't expose visual config - use default
                        let cfg = data::state::pane_config::KlineConfig::default();
                        kline_cfg_view(cfg, chart.study_configurator(), chart_kind, id, chart.basis())
                    };

                    let indicator_modal = if self.modal == Some(Modal::Indicators) {
                        Some(modal::pane::indicators::content_row_kline(
                            id,
                            indicators,
                            true, // Kline allows dragging
                        ))
                    } else {
                        None
                    };

                    self.compose_stack_view(
                        base,
                        id,
                        indicator_modal,
                        compact_controls,
                        settings_modal,
                        None,
                        tickers_table,
                    )
                } else {
                    let content_kind = match chart_kind {
                        data::KlineChartKind::Candles => ContentKind::CandlestickChart,
                        data::KlineChartKind::Footprint { .. } => ContentKind::FootprintChart,
                    };
                    let base = uninitialized_base(content_kind);
                    self.compose_stack_view(
                        base,
                        id,
                        None,
                        compact_controls,
                        || column![].into(),
                        None,
                        tickers_table,
                    )
                }
            }
        };

        // Show loading status in title bar
        match &self.loading_status {
            LoadingStatus::Downloading {
                schema,
                days_complete,
                days_total,
                ..
            } => {
                stream_info_element = stream_info_element.push(text(format!(
                    "Downloading {} ({}/{})",
                    schema, days_complete, days_total
                )));
            }
            LoadingStatus::LoadingFromCache {
                schema, days_loaded, ..
            } => {
                stream_info_element = stream_info_element
                    .push(text(format!("Loading {} ({} days)", schema, days_loaded)));
            }
            LoadingStatus::Building { operation, progress } => {
                stream_info_element = stream_info_element.push(text(format!(
                    "{} ({:.0}%)",
                    operation,
                    progress * 100.0
                )));
            }
            LoadingStatus::Ready | LoadingStatus::Idle => {}
            LoadingStatus::Error { message } => {
                stream_info_element = stream_info_element.push(text(format!("Error: {}", message)));
            }
        }

        let content = pane_grid::Content::new(body)
            .style(move |theme| style::pane_background(theme, is_focused));

        let controls = {
            let compact_control = container(
                button(text("...").size(13).align_y(Alignment::End))
                    .on_press(Message::PaneEvent(id, Event::ShowModal(Modal::Controls)))
                    .style(move |theme, status| {
                        style::button::transparent(
                            theme,
                            status,
                            self.modal == Some(Modal::Controls)
                                || self.modal == Some(Modal::Settings),
                        )
                    }),
            )
            .align_y(Alignment::Center)
            .height(Length::Fixed(32.0))
            .padding(4);

            if self.modal == Some(Modal::Controls) {
                pane_grid::Controls::new(compact_control)
            } else {
                pane_grid::Controls::dynamic(
                    self.view_controls(id, panes, maximized, window != main_window.id),
                    compact_control,
                )
            }
        };

        let title_bar = pane_grid::TitleBar::new(
            stream_info_element
                .padding(padding::left(4).top(1))
                .align_y(Vertical::Center)
                .spacing(8)
                .height(Length::Fixed(32.0)),
        )
        .controls(controls)
        .style(style::pane_title_bar);

        content.title_bar(if self.modal.is_none() {
            title_bar
        } else {
            title_bar.always_show_controls()
        })
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
                            modal::stream::Action::TicksizeSelected(tm) => {
                                modifier.update_kind_with_multiplier(tm);

                                if let Some(ticker) = self.ticker_info {
                                    match &mut self.content {
                                        Content::Kline { chart: Some(c), .. } => {
                                            c.change_tick_size(
                                                tm.multiply_with_min_tick_size(ticker),
                                            );
                                        }
                                        Content::Heatmap { chart: Some(c), .. } => {
                                            c.change_tick_size(
                                                tm.multiply_with_min_tick_size(ticker),
                                            );
                                        }
                                        Content::Ladder(Some(p)) => {
                                            p.set_tick_size(tm.multiply_with_min_tick_size(ticker));
                                        }
                                        _ => {}
                                    }
                                }
                            }
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
                            // Add ticker to comparison chart
                            // This requires loading chart data for the new ticker
                            // For now, log the request - full implementation needs async loading
                            log::info!("Request to add ticker {:?} to comparison chart", ticker_info.ticker);
                            self.notifications.push(Toast::warn(
                                "Adding tickers to comparison requires chart data loading - not yet implemented".to_string()
                            ));
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

    fn view_controls(
        &'_ self,
        pane: pane_grid::Pane,
        total_panes: usize,
        is_maximized: bool,
        is_popout: bool,
    ) -> Element<'_, Message> {
        let modal_btn_style = |modal: Modal| {
            let is_active = self.modal == Some(modal);
            move |theme: &Theme, status: button::Status| {
                style::button::transparent(theme, status, is_active)
            }
        };

        let control_btn_style = |is_active: bool| {
            move |theme: &Theme, status: button::Status| {
                style::button::transparent(theme, status, is_active)
            }
        };

        let treat_as_starter =
            matches!(&self.content, Content::Starter) || !self.content.initialized();

        let tooltip_pos = tooltip::Position::Bottom;
        let mut buttons = row![];

        let show_modal = |modal: Modal| Message::PaneEvent(pane, Event::ShowModal(modal));

        if !treat_as_starter {
            // Settings button
            buttons = buttons.push(button_with_tooltip(
                icon_text(Icon::Cog, 12),
                show_modal(Modal::Settings),
                None,
                tooltip_pos,
                modal_btn_style(Modal::Settings),
            ));
        }
        if !treat_as_starter
            && matches!(
                &self.content,
                Content::Heatmap { .. } | Content::Kline { .. }
            )
        {
            buttons = buttons.push(button_with_tooltip(
                icon_text(Icon::ChartOutline, 12),
                show_modal(Modal::Indicators),
                Some("Indicators"),
                tooltip_pos,
                modal_btn_style(Modal::Indicators),
            ));
        }

        if is_popout {
            buttons = buttons.push(button_with_tooltip(
                icon_text(Icon::Popout, 12),
                Message::Merge,
                Some("Merge"),
                tooltip_pos,
                control_btn_style(is_popout),
            ));
        } else if total_panes > 1 {
            buttons = buttons.push(button_with_tooltip(
                icon_text(Icon::Popout, 12),
                Message::Popout,
                Some("Pop out"),
                tooltip_pos,
                control_btn_style(is_popout),
            ));
        }

        if total_panes > 1 {
            let (resize_icon, message) = if is_maximized {
                (Icon::ResizeSmall, Message::Restore)
            } else {
                (Icon::ResizeFull, Message::MaximizePane(pane))
            };

            buttons = buttons.push(button_with_tooltip(
                icon_text(resize_icon, 12),
                message,
                None,
                tooltip_pos,
                control_btn_style(is_maximized),
            ));

            buttons = buttons.push(button_with_tooltip(
                icon_text(Icon::Close, 12),
                Message::ClosePane(pane),
                None,
                tooltip_pos,
                control_btn_style(false),
            ));
        }

        buttons
            .padding(padding::right(4).left(4))
            .align_y(Vertical::Center)
            .height(Length::Fixed(32.0))
            .into()
    }

    fn compose_stack_view<'a, F>(
        &'a self,
        base: Element<'a, Message>,
        pane: pane_grid::Pane,
        indicator_modal: Option<Element<'a, Message>>,
        compact_controls: Option<Element<'a, Message>>,
        settings_modal: F,
        selected_tickers: Option<&'a [FuturesTickerInfo]>,
        tickers_table: &'a TickersTable,
    ) -> Element<'a, Message>
    where
        F: FnOnce() -> Element<'a, Message>,
    {
        let base =
            widget::toast::Manager::new(base, &self.notifications, Alignment::End, move |msg| {
                Message::PaneEvent(pane, Event::DeleteNotification(msg))
            })
            .into();

        let on_blur = Message::PaneEvent(pane, Event::HideModal);

        match &self.modal {
            Some(Modal::LinkGroup) => {
                let content = link_group_modal(pane, self.link_group);

                stack_modal(
                    base,
                    content,
                    on_blur,
                    padding::right(12).left(4),
                    Alignment::Start,
                )
            }
            Some(Modal::StreamModifier(modifier)) => stack_modal(
                base,
                modifier.view(self.ticker_info).map(move |message| {
                    Message::PaneEvent(pane, Event::StreamModifierChanged(message))
                }),
                Message::PaneEvent(pane, Event::HideModal),
                padding::right(12).left(48),
                Alignment::Start,
            ),
            Some(Modal::MiniTickersList(panel)) => {
                let mini_list = panel
                    .view(tickers_table, selected_tickers, self.ticker_info)
                    .map(move |msg| {
                        Message::PaneEvent(pane, Event::MiniTickersListInteraction(msg))
                    });

                let content: Element<_> = container(mini_list)
                    .max_width(260)
                    .padding(16)
                    .style(style::chart_modal)
                    .into();

                stack_modal(
                    base,
                    content,
                    Message::PaneEvent(pane, Event::HideModal),
                    padding::left(12),
                    Alignment::Start,
                )
            }
            Some(Modal::Settings) => stack_modal(
                base,
                settings_modal(),
                on_blur,
                padding::right(12).left(12),
                Alignment::End,
            ),
            Some(Modal::Indicators) => stack_modal(
                base,
                indicator_modal.unwrap_or_else(|| column![].into()),
                on_blur,
                padding::right(12).left(12),
                Alignment::End,
            ),
            Some(Modal::Controls) => stack_modal(
                base,
                if let Some(controls) = compact_controls {
                    controls
                } else {
                    column![].into()
                },
                on_blur,
                padding::left(12),
                Alignment::End,
            ),
            Some(Modal::DataManagement(panel)) => {
                let pane_id = pane;
                stack_modal(
                    base,
                    panel.view().map(move |msg| Message::PaneEvent(pane_id, Event::DataManagementInteraction(msg))),
                    on_blur,
                    padding::all(12),
                    Alignment::Center,
                )
            }
            None => base,
        }
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

#[derive(Default)]
pub enum Content {
    #[default]
    Starter,
    Heatmap {
        chart: Option<HeatmapChart>,
        indicators: Vec<HeatmapIndicator>,
        layout: ViewConfig,
        studies: Vec<data::domain::chart_ui_types::heatmap::HeatmapStudy>,
    },
    Kline {
        chart: Option<KlineChart>,
        indicators: Vec<KlineIndicator>,
        layout: ViewConfig,
        kind: data::KlineChartKind,
    },
    TimeAndSales(Option<TimeAndSales>),
    Ladder(Option<Ladder>),
    Comparison(Option<ComparisonChart>),
}

impl Content {
    /// Create empty content for a given kind (will be populated when chart data loads)
    pub fn new_for_kind(
        kind: ContentKind,
        ticker_info: FuturesTickerInfo,
        settings: &Settings,
    ) -> Self {
        match kind {
            ContentKind::HeatmapChart => Content::Heatmap {
                chart: None,
                indicators: vec![HeatmapIndicator::Volume],
                layout: ViewConfig {
                    splits: vec![],
                    autoscale: Some(data::Autoscale::CenterLatest),
                },
                studies: vec![],
            },
            ContentKind::CandlestickChart => Content::Kline {
                chart: None,
                indicators: vec![KlineIndicator::Volume],
                layout: ViewConfig {
                    splits: vec![0.8],
                    autoscale: Some(data::Autoscale::FitAll),
                },
                kind: data::KlineChartKind::Candles,
            },
            ContentKind::FootprintChart => Content::Kline {
                chart: None,
                indicators: vec![KlineIndicator::Volume],
                layout: ViewConfig {
                    splits: vec![0.8],
                    autoscale: Some(data::Autoscale::FitAll),
                },
                kind: data::KlineChartKind::Footprint {
                    clusters: data::ClusterKind::default(),
                    scaling: data::ClusterScaling::default(),
                    studies: vec![],
                },
            },
            ContentKind::TimeAndSales => {
                let state_config = settings
                    .visual_config
                    .clone()
                    .and_then(|v| v.time_and_sales());
                // Convert state config to panel config
                let panel_config = state_config.map(|cfg| {
                    let mut defaults = data::panel::timeandsales::Config::default();
                    defaults.max_rows = cfg.max_rows;
                    defaults
                });
                Content::TimeAndSales(Some(TimeAndSales::new(panel_config, ticker_info.into())))
            }
            ContentKind::Ladder => {
                let state_config = settings.visual_config.clone().and_then(|v| v.ladder());
                // Convert state config to panel config
                let panel_config = state_config.map(|cfg| {
                    let mut defaults = data::panel::ladder::Config::default();
                    defaults.levels = cfg.levels;
                    defaults
                });
                Content::Ladder(Some(Ladder::new(panel_config, ticker_info.into(), ticker_info.tick_size)))
            }
            ContentKind::ComparisonChart => Content::Comparison(None),
            ContentKind::Starter => Content::Starter,
        }
    }

    fn placeholder(kind: ContentKind) -> Self {
        match kind {
            ContentKind::Starter => Content::Starter,
            ContentKind::CandlestickChart => Content::Kline {
                chart: None,
                indicators: vec![KlineIndicator::Volume],
                kind: data::KlineChartKind::Candles,
                layout: ViewConfig {
                    splits: vec![],
                    autoscale: Some(data::Autoscale::FitAll),
                },
            },
            ContentKind::FootprintChart => Content::Kline {
                chart: None,
                indicators: vec![KlineIndicator::Volume],
                kind: data::KlineChartKind::Footprint {
                    clusters: data::ClusterKind::default(),
                    scaling: data::ClusterScaling::default(),
                    studies: vec![],
                },
                layout: ViewConfig {
                    splits: vec![],
                    autoscale: Some(data::Autoscale::FitAll),
                },
            },
            ContentKind::HeatmapChart => Content::Heatmap {
                chart: None,
                indicators: vec![HeatmapIndicator::Volume],
                studies: vec![],
                layout: ViewConfig {
                    splits: vec![],
                    autoscale: Some(data::Autoscale::CenterLatest),
                },
            },
            ContentKind::ComparisonChart => Content::Comparison(None),
            ContentKind::TimeAndSales => Content::TimeAndSales(None),
            ContentKind::Ladder => Content::Ladder(None),
        }
    }

    pub fn last_tick(&self) -> Option<Instant> {
        match self {
            Content::Heatmap { chart, .. } => Some(chart.as_ref()?.last_update()),
            Content::Kline { chart, .. } => Some(chart.as_ref()?.last_update()),
            Content::TimeAndSales(panel) => Some(panel.as_ref()?.last_update()),
            Content::Ladder(panel) => Some(panel.as_ref()?.last_update()),
            Content::Comparison(chart) => Some(chart.as_ref()?.last_update()),
            Content::Starter => None,
        }
    }

    pub fn chart_kind(&self) -> Option<data::KlineChartKind> {
        match self {
            Content::Kline { chart, .. } => Some(chart.as_ref()?.kind().clone()),
            _ => None,
        }
    }

    pub fn toggle_indicator(&mut self, indicator: UiIndicator) {
        match (self, indicator) {
            (
                Content::Heatmap {
                    chart, indicators, ..
                },
                UiIndicator::Heatmap(ind),
            ) => {
                let Some(chart) = chart else {
                    return;
                };

                if indicators.contains(&ind) {
                    indicators.retain(|i| i != &ind);
                } else {
                    indicators.push(ind);
                }
                chart.toggle_indicator(ind);
            }
            (
                Content::Kline {
                    chart, indicators, ..
                },
                UiIndicator::Kline(ind),
            ) => {
                let Some(chart) = chart else {
                    return;
                };

                if indicators.contains(&ind) {
                    indicators.retain(|i| i != &ind);
                } else {
                    indicators.push(ind);
                }
                chart.toggle_indicator(ind);
            }
            _ => panic!("indicator toggle on {indicator:?} pane",),
        }
    }

    pub fn reorder_indicators(&mut self, event: &column_drag::DragEvent) {
        match self {
            Content::Heatmap { indicators, .. } => column_drag::reorder_vec(indicators, event),
            Content::Kline { indicators, .. } => column_drag::reorder_vec(indicators, event),
            Content::TimeAndSales(_)
            | Content::Ladder(_)
            | Content::Starter
            | Content::Comparison(_) => {
                panic!("indicator reorder on {} pane", self)
            }
        }
    }

    pub fn change_visual_config(&mut self, config: VisualConfig) {
        match (self, config) {
            (Content::Heatmap { chart: Some(c), .. }, VisualConfig::Heatmap(cfg)) => {
                // Convert data::HeatmapConfig to chart::heatmap::VisualConfig
                let visual = crate::chart::heatmap::VisualConfig {
                    order_size_filter: cfg.order_size_filter,
                    trade_size_filter: cfg.trade_size_filter,
                    trade_size_scale: cfg.trade_size_scale,
                    coalescing: None, // CoalesceKind is not exposed, use None
                };
                c.set_visual_config(visual);
            }
            (Content::Kline { .. }, VisualConfig::Kline(_cfg)) => {
                // KlineChart doesn't expose set_visual_config
                // Future: implement if needed
            }
            (Content::TimeAndSales(Some(panel)), VisualConfig::TimeAndSales(cfg)) => {
                // Convert state config to panel config
                let stacked_bar = cfg.stacked_bar.map(|(is_compact, ratio)| {
                    if is_compact {
                        data::panel::timeandsales::StackedBar::Compact(ratio)
                    } else {
                        data::panel::timeandsales::StackedBar::Full(ratio)
                    }
                });

                panel.config = data::panel::timeandsales::Config {
                    max_rows: cfg.max_rows,
                    show_delta: cfg.show_delta,
                    stacked_bar,
                    trade_size_filter: cfg.trade_size_filter,
                    trade_retention: std::time::Duration::from_secs(cfg.trade_retention_secs),
                };
            }
            (Content::Ladder(Some(panel)), VisualConfig::Ladder(cfg)) => {
                // Convert state config to panel config
                panel.config = data::panel::ladder::Config {
                    levels: cfg.levels,
                    group_by_ticks: panel.config.group_by_ticks, // Preserve existing value
                    show_chase: panel.config.show_chase,         // Preserve existing value
                    show_chase_tracker: cfg.show_chase_tracker,
                    show_spread: cfg.show_spread,
                    trade_retention: std::time::Duration::from_secs(cfg.trade_retention_secs),
                };
            }
            (Content::Comparison(_), VisualConfig::Comparison(_cfg)) => {
                // ComparisonChart doesn't expose set_config for runtime changes
                // Config is set during construction
            }
            _ => {}
        }
    }

    pub fn heatmap_studies(&self) -> Option<Vec<data::domain::chart_ui_types::heatmap::HeatmapStudy>> {
        match &self {
            Content::Heatmap { studies, .. } => Some(studies.clone()),
            _ => None,
        }
    }

    pub fn footprint_studies(&self) -> Option<Vec<FootprintStudy>> {
        match &self {
            Content::Kline { kind, .. } => {
                if let data::KlineChartKind::Footprint { studies, .. } = kind {
                    Some(studies.clone())
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    pub fn update_heatmap_studies(
        &mut self,
        studies: Vec<data::domain::chart_ui_types::heatmap::HeatmapStudy>,
    ) {
        if let Content::Heatmap {
            chart,
            studies: previous,
            ..
        } = self
        {
            if let Some(c) = chart {
                // Convert data studies to chart studies
                c.studies = studies
                    .iter()
                    .map(|s| match s {
                        data::domain::chart_ui_types::heatmap::HeatmapStudy::VolumeProfile(kind) => {
                            crate::chart::heatmap::HeatmapStudy::VolumeProfile(*kind)
                        }
                    })
                    .collect();
            }
            *previous = studies;
        }
    }

    pub fn update_footprint_studies(&mut self, studies: Vec<FootprintStudy>) {
        if let Content::Kline { chart, kind, .. } = self {
            if let Some(c) = chart {
                c.set_studies(studies.clone());
            }
            if let data::KlineChartKind::Footprint {
                studies: k_studies, ..
            } = kind
            {
                *k_studies = studies;
            }
        }
    }

    pub fn kind(&self) -> ContentKind {
        match self {
            Content::Heatmap { .. } => ContentKind::HeatmapChart,
            Content::Kline { kind, .. } => match kind {
                data::KlineChartKind::Footprint { .. } => ContentKind::FootprintChart,
                data::KlineChartKind::Candles => ContentKind::CandlestickChart,
            },
            Content::TimeAndSales(_) => ContentKind::TimeAndSales,
            Content::Ladder(_) => ContentKind::Ladder,
            Content::Comparison(_) => ContentKind::ComparisonChart,
            Content::Starter => ContentKind::Starter,
        }
    }

    fn initialized(&self) -> bool {
        match self {
            Content::Heatmap { chart, .. } => chart.is_some(),
            Content::Kline { chart, .. } => chart.is_some(),
            Content::TimeAndSales(panel) => panel.is_some(),
            Content::Ladder(panel) => panel.is_some(),
            Content::Comparison(chart) => chart.is_some(),
            Content::Starter => true,
        }
    }
}

impl std::fmt::Display for Content {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.kind())
    }
}

impl PartialEq for Content {
    fn eq(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (Content::Starter, Content::Starter)
                | (Content::Heatmap { .. }, Content::Heatmap { .. })
                | (Content::Kline { .. }, Content::Kline { .. })
                | (Content::TimeAndSales(_), Content::TimeAndSales(_))
                | (Content::Ladder(_), Content::Ladder(_))
        )
    }
}

fn link_group_modal<'a>(
    pane: pane_grid::Pane,
    selected_group: Option<LinkGroup>,
) -> Element<'a, Message> {
    let mut grid = column![].spacing(4);
    let rows = LinkGroup::ALL.chunks(3);

    for row_groups in rows {
        let mut button_row = row![].spacing(4);

        for &group in row_groups {
            let is_selected = selected_group == Some(group);
            let btn_content = text(group.to_string()).font(style::AZERET_MONO);

            let btn = if is_selected {
                button_with_tooltip(
                    btn_content.align_x(iced::Alignment::Center),
                    Message::SwitchLinkGroup(pane, None),
                    Some("Unlink"),
                    tooltip::Position::Bottom,
                    move |theme, status| style::button::menu_body(theme, status, true),
                )
            } else {
                button(btn_content.align_x(iced::Alignment::Center))
                    .on_press(Message::SwitchLinkGroup(pane, Some(group)))
                    .style(move |theme, status| style::button::menu_body(theme, status, false))
                    .into()
            };

            button_row = button_row.push(btn);
        }

        grid = grid.push(button_row);
    }

    container(grid)
        .max_width(240)
        .padding(16)
        .style(style::chart_modal)
        .into()
}

fn ticksize_modifier<'a>(
    id: pane_grid::Pane,
    base_ticksize: f32,
    multiplier: TickMultiplier,
    modifier: Option<modal::stream::Modifier>,
    kind: ModifierKind,
    exchange: Option<data::FuturesVenue>,
) -> Element<'a, Message> {
    let modifier_modal = Modal::StreamModifier(
        modal::stream::Modifier::new(kind).with_ticksize_view(base_ticksize, multiplier, exchange),
    );

    let is_active = modifier.is_some_and(|m| {
        matches!(
            m.view_mode,
            modal::stream::ViewMode::TicksizeSelection { .. }
        )
    });

    button(text(multiplier.to_string()))
        .style(move |theme, status| style::button::modifier(theme, status, !is_active))
        .on_press(Message::PaneEvent(id, Event::ShowModal(modifier_modal)))
        .into()
}

fn basis_modifier<'a>(
    id: pane_grid::Pane,
    selected_basis: ChartBasis,
    modifier: Option<modal::stream::Modifier>,
    kind: ModifierKind,
) -> Element<'a, Message> {
    let modifier_modal = Modal::StreamModifier(
        modal::stream::Modifier::new(kind).with_view_mode(modal::stream::ViewMode::BasisSelection),
    );

    let is_active =
        modifier.is_some_and(|m| m.view_mode == modal::stream::ViewMode::BasisSelection);

    button(text(selected_basis.to_string()))
        .style(move |theme, status| style::button::modifier(theme, status, !is_active))
        .on_press(Message::PaneEvent(id, Event::ShowModal(modifier_modal)))
        .into()
}
