use super::{Content, Effect, Event, State};
use crate::{
    chart,
    modal::{self, pane::Modal},
    screen::dashboard::panel,
};
use data::{
    ChartBasis, ChartConfig, ChartType, ContentKind, DateRange, Timeframe,
};

impl State {
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
                    let modal = Modal::MiniTickersList(
                        crate::modal::pane::tickers::MiniPanel::new(),
                    );

                    if let Some(effect) = self.show_modal_with_focus(modal) {
                        return Some(effect);
                    }
                }
            }
            Event::ChartInteraction(msg) => {
                match msg {
                    chart::Message::DrawingClick(point) => {
                        self.handle_drawing_click(point);
                    }
                    chart::Message::DrawingMove(point) => {
                        self.handle_drawing_move(point);
                    }
                    chart::Message::DrawingCancel => {
                        self.handle_drawing_cancel();
                    }
                    chart::Message::DrawingDelete => {
                        self.handle_drawing_delete();
                    }
                    chart::Message::CrosshairMoved => {
                        // Optimize crosshair updates when a drawing tool is active:
                        // Only clear the main crosshair cache, skip indicator caches
                        use crate::chart::Chart;
                        match &mut self.content {
                            Content::Kline {
                                chart: Some(c), ..
                            } => {
                                if c.drawings.active_tool()
                                    != data::DrawingTool::None
                                {
                                    c.mut_state().cache.clear_crosshair();
                                } else {
                                    chart::update(c, &msg);
                                }
                            }
                            Content::Heatmap {
                                chart: Some(c), ..
                            } => {
                                if c.drawings.active_tool()
                                    != data::DrawingTool::None
                                {
                                    c.mut_state().cache.clear_crosshair();
                                } else {
                                    chart::update(c, &msg);
                                }
                            }
                            _ => {}
                        }
                    }
                    _ => {
                        match &mut self.content {
                            Content::Heatmap {
                                chart: Some(c), ..
                            } => {
                                chart::update(c, &msg);
                            }
                            Content::Kline {
                                chart: Some(c), ..
                            } => {
                                chart::update(c, &msg);
                            }
                            _ => {}
                        }
                    }
                }
            }
            Event::PanelInteraction(msg) => match &mut self.content {
                Content::Ladder(Some(p)) => panel::update(p, msg),
                Content::TimeAndSales(Some(p)) => panel::update(p, msg),
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
                    if let Content::Kline { chart, kind, .. } =
                        &mut self.content
                        && let Some(c) = chart
                    {
                        c.update_study_configurator(m);
                        *kind = c.kind().clone();
                    }
                }
                modal::pane::settings::study::StudyMessage::Heatmap(m) => {
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
                if let Some(Modal::StreamModifier(modifier)) =
                    self.modal.take()
                {
                    let mut modifier = modifier;

                    if let Some(action) = modifier.update(message) {
                        match action {
                            modal::stream::Action::TabSelected(tab) => {
                                modifier.tab = tab;
                            }
                            modal::stream::Action::BasisSelected(
                                new_basis,
                            ) => {
                                modifier.update_kind_with_basis(new_basis);
                                self.settings.selected_basis =
                                    Some(new_basis);

                                match &mut self.content {
                                    Content::Heatmap {
                                        chart: Some(c), ..
                                    } => {
                                        c.set_basis(new_basis);
                                    }
                                    Content::Kline {
                                        chart: Some(c), ..
                                    } => {
                                        if let Some(ticker) =
                                            self.ticker_info
                                        {
                                            c.switch_basis(
                                                new_basis, ticker,
                                            );
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
            }
            Event::ComparisonChartInteraction(message) => {
                if let Content::Comparison(chart_opt) = &mut self.content
                    && let Some(chart) = chart_opt
                    && let Some(action) = chart.update(message)
                {
                    match action {
                        chart::comparison::Action::SeriesColorChanged(
                            t, color,
                        ) => {
                            chart.set_series_color(t, color);
                        }
                        chart::comparison::Action::SeriesNameChanged(
                            t, name,
                        ) => {
                            chart.set_series_name(t, name);
                        }
                        chart::comparison::Action::OpenSeriesEditor => {
                            self.modal = Some(Modal::Settings);
                        }
                        chart::comparison::Action::RemoveSeries(
                            ticker_info,
                        ) => {
                            if let Content::Comparison(Some(chart)) =
                                &mut self.content
                            {
                                chart.remove_ticker(&ticker_info);
                                log::info!(
                                    "Removed ticker {:?} from comparison chart",
                                    ticker_info.ticker
                                );
                            }
                        }
                    }
                }
            }
            Event::MiniTickersListInteraction(message) => {
                if let Some(Modal::MiniTickersList(ref mut mini_panel)) =
                    self.modal
                    && let Some(action) = mini_panel.update(message)
                {
                    self.modal = Some(Modal::MiniTickersList(
                        mini_panel.clone(),
                    ));

                    let crate::modal::pane::tickers::Action::RowSelected(
                        sel,
                    ) = action;
                    match sel {
                        crate::modal::pane::tickers::RowSelection::Add(
                            ticker_info,
                        ) => {
                            log::info!(
                                "Adding ticker {:?} to comparison chart",
                                ticker_info.ticker
                            );

                            let basis = self
                                .settings
                                .selected_basis
                                .unwrap_or(ChartBasis::Time(
                                    Timeframe::M15,
                                ));

                            let date_range = DateRange::new(
                                chrono::Local::now().date_naive()
                                    - chrono::Duration::days(7),
                                chrono::Local::now().date_naive(),
                            );

                            let chart_config = ChartConfig {
                                ticker: ticker_info.ticker,
                                basis,
                                date_range,
                                chart_type: ChartType::Candlestick,
                            };

                            return Some(Effect::LoadChart {
                                config: chart_config,
                                ticker_info,
                            });
                        }
                        crate::modal::pane::tickers::RowSelection::Remove(
                            ticker_info,
                        ) => {
                            if let Content::Comparison(Some(chart)) =
                                &mut self.content
                            {
                                chart.remove_ticker(&ticker_info);
                                log::info!(
                                    "Removed ticker {:?} from comparison chart",
                                    ticker_info.ticker
                                );
                            }
                        }
                        crate::modal::pane::tickers::RowSelection::Switch(
                            ti,
                        ) => {
                            return Some(
                                Effect::SwitchTickersInGroup(ti),
                            );
                        }
                    }
                }
            }
            Event::DataManagementInteraction(message) => {
                if let Some(Modal::DataManagement(ref mut panel)) =
                    self.modal
                {
                    if let Some(action) = panel.update(message) {
                        self.modal =
                            Some(Modal::DataManagement(panel.clone()));

                        match action {
                            crate::modal::pane::download::data_management::Action::EstimateRequested { ticker, schema, date_range } => {
                                log::info!("Estimate requested: {:?} {:?} {:?}", ticker, schema, date_range);
                                return Some(Effect::EstimateDataCost { ticker, schema, date_range });
                            }
                            crate::modal::pane::download::data_management::Action::DownloadRequested { ticker, schema, date_range } => {
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

    fn show_modal_with_focus(
        &mut self,
        requested_modal: Modal,
    ) -> Option<Effect> {
        let should_toggle_close = match (&self.modal, &requested_modal) {
            (
                Some(Modal::StreamModifier(open)),
                Modal::StreamModifier(req),
            ) => open.view_mode == req.view_mode,
            (Some(open), req) => {
                core::mem::discriminant(open)
                    == core::mem::discriminant(req)
            }
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
}
