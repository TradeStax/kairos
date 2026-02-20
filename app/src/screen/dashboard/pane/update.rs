use super::{Content, ContextMenuAction, ContextMenuKind, Effect, Event, State};
use crate::{
    chart,
    modals::{self, pane::Modal},
    screen::dashboard::panel,
};
use data::{ChartBasis, ChartConfig, ChartType, ContentKind, DateRange, Timeframe};

impl State {
    pub fn update(&mut self, msg: Event) -> Option<Effect> {
        // Dismiss context menu on meaningful interactions (not passive mouse movement)
        if self.context_menu.is_some()
            && !matches!(
                msg,
                Event::ContextMenuAction(_)
                    | Event::DismissContextMenu
                    | Event::ChartInteraction(chart::Message::CrosshairMoved)
                    | Event::ChartInteraction(chart::Message::BoundsChanged(_))
            )
        {
            self.context_menu = None;
        }

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
                    let modal =
                        Modal::MiniTickersList(crate::modals::pane::tickers::MiniPanel::new());

                    if let Some(effect) = self.show_modal_with_focus(modal) {
                        return Some(effect);
                    }
                }
            }
            Event::ChartInteraction(msg) => {
                match msg {
                    chart::Message::IndicatorClicked(_) => {
                        // Indicator panels removed; no-op
                    }
                    chart::Message::DrawingClick(point, shift_held) => {
                        if self.handle_drawing_click(point, shift_held) {
                            return Some(Effect::DrawingToolChanged(data::DrawingTool::None));
                        }
                    }
                    chart::Message::DrawingMove(point, shift_held) => {
                        self.handle_drawing_move(point, shift_held);
                    }
                    chart::Message::ClonePlacementMove(point) => {
                        self.handle_clone_move(point);
                    }
                    chart::Message::ClonePlacementConfirm(point) => {
                        self.handle_clone_confirm(point);
                    }
                    chart::Message::ClonePlacementCancel => {
                        self.handle_clone_cancel();
                    }
                    chart::Message::DrawingCancel => {
                        self.handle_drawing_cancel();
                    }
                    chart::Message::DrawingDelete => {
                        self.handle_drawing_delete();
                    }
                    chart::Message::DrawingSelect(id) => {
                        self.handle_drawing_select(id);
                    }
                    chart::Message::DrawingDeselect => {
                        self.handle_drawing_deselect();
                    }
                    chart::Message::DrawingDrag(point, shift_held) => {
                        self.handle_drawing_drag(point, shift_held);
                    }
                    chart::Message::DrawingHandleDrag(point, handle_index, shift_held) => {
                        self.handle_drawing_handle_drag(point, handle_index, shift_held);
                    }
                    chart::Message::DrawingDragEnd => {
                        self.handle_drawing_drag_end();
                    }
                    chart::Message::DrawingDoubleClick(id) => {
                        self.handle_open_drawing_properties(id);
                    }
                    chart::Message::ContextMenu(position, drawing_id) => {
                        self.modal = None;

                        // Use hit-tested drawing, or fall back to
                        // currently selected drawing
                        let effective_id = drawing_id.or_else(|| self.get_selected_drawing_id());

                        if let Some(id) = effective_id {
                            let locked = self.get_drawing_locked(id);
                            self.context_menu = Some(ContextMenuKind::Drawing {
                                position,
                                id,
                                locked,
                            });
                        } else {
                            self.context_menu = Some(ContextMenuKind::Chart { position });
                        }
                    }
                    chart::Message::CrosshairMoved => {
                        // Optimize crosshair updates when a drawing tool is active:
                        // Only clear the main crosshair cache, skip indicator caches
                        use crate::chart::Chart;
                        match &mut self.content {
                            Content::Kline { chart: Some(c), .. } => {
                                if c.drawings.active_tool() != data::DrawingTool::None {
                                    c.mut_state().cache.clear_crosshair();
                                } else {
                                    chart::update(c, &msg);
                                }
                            }
                            Content::Heatmap { chart: Some(c), .. } => {
                                if c.drawings.active_tool() != data::DrawingTool::None {
                                    c.mut_state().cache.clear_crosshair();
                                } else {
                                    chart::update(c, &msg);
                                }
                            }
                            _ => {}
                        }
                    }
                    _ => match &mut self.content {
                        Content::Heatmap { chart: Some(c), .. } => {
                            chart::update(c, &msg);
                        }
                        Content::Kline { chart: Some(c), .. } => {
                            chart::update(c, &msg);
                        }
                        _ => {}
                    },
                }
            }
            Event::PanelInteraction(msg) => match &mut self.content {
                Content::Ladder(Some(p)) => panel::update(p, msg),
                Content::TimeAndSales(Some(p)) => panel::update(p, msg),
                _ => {}
            },
            Event::ToggleStudy(study_id) => {
                self.content.toggle_study(&study_id);
            }
            Event::DeleteNotification(idx) => {
                if idx < self.notifications.len() {
                    self.notifications.remove(idx);
                }
            }
            Event::ReorderIndicator(e) => {
                self.content.reorder_indicators(&e);
            }
            Event::FootprintStudyChanged(new_config) => {
                if let Content::Kline { chart, .. } = &mut self.content
                    && let Some(c) = chart
                {
                    c.set_footprint(new_config);
                }
            }
            Event::StudyConfigurator(study_msg) => match study_msg {
                modals::pane::settings::study::StudyMessage::Heatmap(m) => {
                    if let Content::Heatmap { chart, studies, .. } = &mut self.content
                        && let Some(c) = chart
                    {
                        c.update_study_configurator(m);
                        *studies = c
                            .studies
                            .iter()
                            .map(|s| match s {
                                crate::chart::heatmap::HeatmapStudy::VolumeProfile(kind) => {
                                    data::domain::chart::heatmap::HeatmapStudy::VolumeProfile(*kind)
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
                                    Content::Kline { chart: Some(c), .. } => {
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
            }
            Event::ComparisonChartInteraction(message) => {
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
                            if let Content::Comparison(Some(chart)) = &mut self.content {
                                chart.remove_ticker(&ticker_info);
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

                    let crate::modals::pane::tickers::Action::RowSelected(sel) = action;
                    match sel {
                        crate::modals::pane::tickers::RowSelection::Add(ticker_info) => {
                            let basis = self
                                .settings
                                .selected_basis
                                .unwrap_or(ChartBasis::Time(Timeframe::M5));

                            let date_range = DateRange::new(
                                chrono::Local::now().date_naive() - chrono::Duration::days(7),
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
                        crate::modals::pane::tickers::RowSelection::Remove(ticker_info) => {
                            if let Content::Comparison(Some(chart)) = &mut self.content {
                                chart.remove_ticker(&ticker_info);
                            }
                        }
                        crate::modals::pane::tickers::RowSelection::Switch(ti) => {
                            return Some(Effect::SwitchTickersInGroup(ti));
                        }
                    }
                }
            }
            Event::DataManagementInteraction(message) => {
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
                            return Some(Effect::EstimateDataCost {
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
                            return Some(Effect::DownloadData {
                                ticker,
                                schema,
                                date_range,
                            });
                        }
                    }
                }
            }
            Event::DismissContextMenu => {
                self.context_menu = None;
            }
            Event::ContextMenuAction(action) => {
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
                }
            }
            Event::DrawingPropertiesChanged(message) => {
                if let Some(Modal::DrawingProperties(ref mut modal)) = self.modal {
                    let mut modal = modal.clone();
                    if let Some(action) = modal.update(message) {
                        match action {
                            crate::modals::drawing_properties::Action::Applied(id, update) => {
                                self.apply_drawing_style(id, &update);
                                let snapshot = modal.before_snapshot().clone();
                                self.finalize_drawing_properties(id, snapshot);
                                self.modal = None;
                            }
                            crate::modals::drawing_properties::Action::Cancelled(id, original) => {
                                self.apply_drawing_style(id, &original);
                                self.modal = None;
                            }
                        }
                    } else {
                        // No action yet — apply live preview
                        let id = modal.drawing_id();
                        let update = modal.build_update();
                        self.apply_drawing_style(id, &update);
                        self.modal = Some(Modal::DrawingProperties(modal));
                    }
                }
            }
            Event::OpenIndicatorManager => {
                self.open_indicator_manager();
            }
            Event::IndicatorManagerInteraction(message) => {
                if let Some(Modal::IndicatorManager(ref mut manager)) = self.modal {
                    let mut manager = manager.clone();
                    if let Some(action) = manager.update(message) {
                        use modals::pane::indicator_manager::Action;
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
                                    &study_id, &key, value.clone(),
                                );
                                // Reload chart data when days_to_load changes
                                if study_id == "big_trades"
                                    && key == "days_to_load"
                                    && let study::ParameterValue::Integer(
                                        days,
                                    ) = value
                                {
                                    self.modal =
                                        Some(Modal::IndicatorManager(
                                            manager,
                                        ));
                                    return self
                                        .rebuild_chart_with_days(days);
                                }
                            }
                            Action::OpenBigTradesDebug => {
                                self.modal = Some(Modal::BigTradesDebug);
                                return None;
                            }
                            Action::Close => {
                                self.modal = None;
                                return None;
                            }
                        }
                    }
                    self.modal = Some(Modal::IndicatorManager(manager));
                }
            }
        }
        None
    }

    fn open_indicator_manager(&mut self) {
        use modals::pane::indicator_manager::IndicatorManagerModal;

        let content_kind = self.content.kind();
        let active_study_ids = match &self.content {
            Content::Kline { study_ids, .. } => study_ids.clone(),
            _ => vec![],
        };
        let studies: Vec<Box<dyn study::Study>> = match &self.content {
            Content::Kline { chart: Some(c), .. } => {
                c.studies().iter().map(|s| s.clone_study()).collect()
            }
            _ => vec![],
        };

        let manager = IndicatorManagerModal::new(
            content_kind,
            active_study_ids,
            studies,
        );
        self.modal = Some(Modal::IndicatorManager(manager));
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
}
