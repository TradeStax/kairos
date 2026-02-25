use crate::chart;
use crate::modals::{self, pane::Modal};
use crate::screen::dashboard::pane::{Action, Content, ContextMenuAction, State};
use data::{ChartBasis, ChartConfig, ChartType, DateRange, Timeframe};

impl State {
    pub(super) fn handle_stream_modifier(
        &mut self,
        message: crate::modals::stream::Message,
    ) -> Option<Action> {
        if let Some(Modal::StreamModifier(modifier)) = self.modal.take() {
            let mut modifier = modifier;

            if let Some(action) = modifier.update(message) {
                match action {
                    modals::stream::Action::TabSelected(tab) => {
                        modifier.tab = tab;
                    }
                    modals::stream::Action::BasisSelected(new_basis) => {
                        modifier.update_kind_with_basis(new_basis);
                        self.settings.selected_basis = Some(new_basis);

                        match &mut self.content {
                            Content::Heatmap { chart: Some(c), .. } => {
                                c.set_basis(new_basis);
                            }
                            Content::Candlestick { chart: Some(c), .. } => {
                                if let Some(ticker) = self.ticker_info {
                                    c.switch_basis(new_basis, ticker);
                                }
                            }
                            Content::Comparison(_) => {}
                            _ => {}
                        }
                    }
                }
            }

            self.modal = Some(Modal::StreamModifier(modifier));
        }
        None
    }

    pub(super) fn handle_mini_tickers_list(
        &mut self,
        message: crate::modals::pane::tickers::Message,
    ) -> Option<Action> {
        if let Some(Modal::MiniTickersList(ref mut mini_panel)) = self.modal
            && let Some(action) = mini_panel.update(message)
        {
            self.modal = Some(Modal::MiniTickersList(mini_panel.clone()));

            let crate::modals::pane::tickers::Action::RowSelected(sel) = action;
            match sel {
                crate::modals::pane::tickers::RowSelection::Add(ticker_info) => {
                    let basis = self
                        .settings
                        .selected_basis
                        .unwrap_or(ChartBasis::Time(Timeframe::M5));

                    let date_range = DateRange::new(
                        chrono::Local::now().date_naive()
                            - chrono::Duration::days(7),
                        chrono::Local::now().date_naive(),
                    )
                    .expect("invariant: 7 days ago < today");

                    let chart_config = ChartConfig {
                        ticker: ticker_info.ticker,
                        basis,
                        date_range,
                        chart_type: ChartType::Candlestick,
                    };

                    return Some(Action::LoadChart {
                        config: chart_config,
                        ticker_info,
                    });
                }
                crate::modals::pane::tickers::RowSelection::Remove(
                    ticker_info,
                ) => {
                    if let Content::Comparison(Some(chart)) = &mut self.content {
                        chart.remove_ticker(&ticker_info);
                    }
                }
                crate::modals::pane::tickers::RowSelection::Switch(ti) => {
                    return Some(Action::SwitchTickersInGroup(ti));
                }
            }
        }
        None
    }

    pub(super) fn handle_data_management(
        &mut self,
        message: crate::modals::download::DataManagementMessage,
    ) -> Option<Action> {
        if let Some(Modal::DataManagement(ref mut panel)) = self.modal
            && let Some(action) = panel.update(message)
        {
            self.modal = Some(Modal::DataManagement(panel.clone()));

            match action {
                crate::modals::download::data_management::Action::EstimateRequested {
                    ticker,
                    schema,
                    date_range,
                } => {
                    return Some(Action::EstimateDataCost {
                        ticker,
                        schema,
                        date_range,
                    });
                }
                crate::modals::download::data_management::Action::DownloadRequested {
                    ticker,
                    schema,
                    date_range,
                } => {
                    return Some(Action::DownloadData {
                        ticker,
                        schema,
                        date_range,
                    });
                }
            }
        }
        None
    }

    pub(super) fn handle_drawing_properties_modal(
        &mut self,
        message: crate::modals::drawing::properties::Message,
    ) -> Option<Action> {
        if let Some(Modal::DrawingProperties(ref mut modal)) = self.modal {
            let mut modal = modal.clone();
            if let Some(action) = modal.update(message) {
                match action {
                    crate::modals::drawing::properties::Action::Applied(
                        id,
                        update,
                    ) => {
                        self.apply_drawing_style(id, &update);
                        let snapshot = modal.before_snapshot().clone();
                        self.finalize_drawing_properties(id, snapshot);
                        self.modal = None;
                    }
                    crate::modals::drawing::properties::Action::Cancelled(
                        id,
                        original,
                    ) => {
                        self.apply_drawing_style(id, &original);
                        self.modal = None;
                    }
                }
            } else {
                // No action yet -- apply live preview
                let id = modal.drawing_id();
                let update = modal.build_update();
                self.apply_drawing_style(id, &update);
                self.modal = Some(Modal::DrawingProperties(modal));
            }
        }
        None
    }

    pub(super) fn handle_indicator_manager(
        &mut self,
        message: modals::pane::indicator::Message,
    ) -> Option<Action> {
        if let Some(Modal::IndicatorManager(ref mut manager)) = self.modal {
            let mut manager = manager.clone();
            if let Some(action) = manager.update(message) {
                use modals::pane::indicator::Action;
                match action {
                    Action::ToggleStudy(study_id) => {
                        self.content.toggle_study(&study_id);
                    }
                    Action::ReorderIndicators(event) => {
                        self.content.reorder_indicators(&event);
                    }
                    Action::StudyParameterUpdated {
                        study_id,
                        key,
                        value,
                    } => {
                        self.content.update_study_parameter(
                            &study_id,
                            &key,
                            value.clone(),
                        );
                        // Reload chart data when days_to_load changes
                        if study_id == "big_trades"
                            && key == "days_to_load"
                            && let study::ParameterValue::Integer(days) = value
                        {
                            self.modal =
                                Some(Modal::IndicatorManager(manager));
                            return self.rebuild_chart_with_days(days);
                        }
                    }
                    Action::Close => {
                        self.modal = None;
                        return None;
                    }
                }
            }
            self.modal = Some(Modal::IndicatorManager(manager));
        }
        None
    }

    pub(super) fn handle_study_configurator(
        &mut self,
        study_msg: modals::pane::settings::StudyMessage,
    ) {
        match study_msg {
            modals::pane::settings::StudyMessage::Heatmap(m) => {
                if let Content::Heatmap {
                    chart, studies, ..
                } = &mut self.content
                    && let Some(c) = chart
                {
                    c.update_study_configurator(m);
                    *studies = c
                        .studies
                        .iter()
                        .map(|s| match s {
                            crate::chart::heatmap::HeatmapStudy::VolumeProfile(
                                kind,
                            ) => {
                                data::domain::chart::heatmap::HeatmapStudy::VolumeProfile(*kind)
                            }
                        })
                        .collect();
                }
            }
        }
    }

    pub(super) fn handle_comparison_chart(
        &mut self,
        message: chart::comparison::Message,
    ) -> Option<Action> {
        if let Content::Comparison(chart_opt) = &mut self.content
            && let Some(chart) = chart_opt
            && let Some(action) = chart.update(message)
        {
            match action {
                chart::comparison::Action::SeriesColorChanged(t, color) => {
                    chart.set_series_color(t, color);
                }
                chart::comparison::Action::SeriesNameChanged(t, name) => {
                    chart.set_series_name(t, name);
                }
                chart::comparison::Action::OpenSeriesEditor => {
                    self.modal = Some(Modal::Settings);
                }
                chart::comparison::Action::RemoveSeries(ticker_info) => {
                    if let Content::Comparison(Some(chart)) = &mut self.content
                    {
                        chart.remove_ticker(&ticker_info);
                    }
                }
            }
        }
        None
    }

    pub(super) fn handle_context_menu_action(
        &mut self,
        action: ContextMenuAction,
    ) -> Option<Action> {
        self.context_menu = None;
        match action {
            ContextMenuAction::RebuildChart => {
                return self.rebuild_current_chart();
            }
            ContextMenuAction::CenterLastPrice => {
                self.center_last_price();
            }
            ContextMenuAction::OpenIndicators => {
                self.open_indicator_manager();
            }
            ContextMenuAction::DeleteDrawing(id) => {
                self.handle_drawing_context_delete(id);
            }
            ContextMenuAction::ToggleLockDrawing(id) => {
                self.handle_drawing_toggle_lock(id);
            }
            ContextMenuAction::CloneDrawing(id) => {
                self.handle_drawing_clone(id);
            }
            ContextMenuAction::OpenDrawingProperties(id) => {
                self.handle_open_drawing_properties(id);
            }
            ContextMenuAction::OpenStudyProperties(idx) => {
                self.open_indicator_manager_for_study(idx);
            }
            ContextMenuAction::CopyAiMessageText(idx) => {
                if let Content::AiAssistant(ai) = &self.content {
                    if let Some(text) = ai.message_text(idx) {
                        return Some(Action::CopyToClipboard(text));
                    }
                }
            }
        }
        None
    }
}
