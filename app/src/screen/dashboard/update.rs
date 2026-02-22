use super::{Dashboard, Event, Message, pane};
use crate::{
    components::display::toast::Toast, modals::download::DownloadProgress, modals::pane::Modal,
    window::Window,
};
use data::LoadingStatus;
use iced::Task;
use iced::widget::pane_grid;

impl Dashboard {
    pub fn update(
        &mut self,
        message: Message,
        main_window: &Window,
    ) -> (Task<Message>, Option<Event>) {
        match message {
            Message::SavePopoutSpecs(specs) => {
                for (window_id, new_spec) in specs {
                    if let Some((_, spec)) = self.popout.get_mut(&window_id) {
                        *spec = new_spec;
                    }
                }
            }
            Message::ErrorOccurred(pane_id, err) => match pane_id {
                Some(id) => {
                    if let Some(state) = self.get_mut_pane_state_by_uuid(main_window.id, id) {
                        state.loading_status = LoadingStatus::Ready;
                        state.notifications.push(Toast::error(err.to_string()));
                    }
                }
                _ => {
                    return (
                        Task::done(Message::Notification(Toast::error(err.to_string()))),
                        None,
                    );
                }
            },
            Message::Pane(window, message) => match message {
                pane::Message::PaneClicked(pane) => {
                    self.focus = Some((window, pane));
                }
                pane::Message::PaneResized(pane_grid::ResizeEvent { split, ratio }) => {
                    self.panes.resize(split, ratio);
                }
                pane::Message::PaneDragged(event) => {
                    if let pane_grid::DragEvent::Dropped { pane, target } = event {
                        self.panes.drop(pane, target);
                    }
                }
                pane::Message::SplitPane(axis, pane) => {
                    let focus_pane = if let Some((new_pane, _)) =
                        self.panes.split(axis, pane, pane::State::new())
                    {
                        Some(new_pane)
                    } else {
                        None
                    };

                    if Some(focus_pane).is_some() {
                        self.focus = Some((window, focus_pane.unwrap()));
                    }
                }
                pane::Message::ClosePane(pane) => {
                    // Get pane UUID before closing to clean up chart state
                    if let Some(pane_state) = self.panes.get(pane) {
                        let uuid = pane_state.unique_id();
                        self.charts.remove(&uuid);
                        log::debug!("Cleaned up chart state for closed pane {}", uuid);
                    }

                    if let Some((_, sibling)) = self.panes.close(pane) {
                        self.focus = Some((window, sibling));
                    }
                }
                pane::Message::MaximizePane(pane) => {
                    self.panes.maximize(pane);
                }
                pane::Message::Restore => {
                    self.panes.restore();
                }
                pane::Message::ReplacePane(pane) => {
                    if let Some(pane) = self.panes.get_mut(pane) {
                        *pane = pane::State::new();
                    }
                }
                pane::Message::VisualConfigChanged(pane, cfg, to_sync) => {
                    if to_sync {
                        if let Some(state) = self.get_pane(main_window.id, window, pane) {
                            // Extract studies from heatmap content if present
                            let studies_cfg: Option<
                                Vec<data::domain::chart::heatmap::HeatmapStudy>,
                            > = match &state.content {
                                pane::Content::Heatmap { studies, .. } => Some(studies.clone()),
                                _ => None,
                            };
                            self.iter_all_panes_mut(main_window.id)
                                .for_each(|(_, _, state)| {
                                    let should_apply = match state.settings.visual_config {
                                        Some(ref current_cfg) => {
                                            std::mem::discriminant(current_cfg)
                                                == std::mem::discriminant(&cfg)
                                        }
                                        None => matches!(
                                            (&cfg, &state.content),
                                            (
                                                data::state::pane::VisualConfig::Kline(_),
                                                pane::Content::Kline { .. }
                                            ) | (
                                                data::state::pane::VisualConfig::Heatmap(_),
                                                pane::Content::Heatmap { .. }
                                            ) | (
                                                data::state::pane::VisualConfig::TimeAndSales(_),
                                                pane::Content::TimeAndSales(_)
                                            ) | (
                                                data::state::pane::VisualConfig::Comparison(_),
                                                pane::Content::Comparison(_)
                                            )
                                        ),
                                    };

                                    if should_apply {
                                        state.settings.visual_config = Some(cfg.clone());
                                        state.content.change_visual_config(cfg.clone());

                                        // Update studies for heatmap content
                                        if let Some(studies) = &studies_cfg
                                            && let pane::Content::Heatmap {
                                                studies: hm_studies,
                                                ..
                                            } = &mut state.content
                                        {
                                            *hm_studies = studies.clone();
                                        }
                                    }
                                });
                        }
                    } else if let Some(state) = self.get_mut_pane(main_window.id, window, pane) {
                        state.settings.visual_config = Some(cfg.clone());
                        state.content.change_visual_config(cfg);
                    }
                }
                pane::Message::SwitchLinkGroup(pane, group) => {
                    if group.is_none() {
                        if let Some(state) = self.get_mut_pane(main_window.id, window, pane) {
                            state.link_group = None;
                        }
                        return (Task::none(), None);
                    }

                    // Find the ticker from an existing pane in this link group
                    let group_ticker_info = self
                        .iter_all_panes(main_window.id)
                        .filter(|(w, p, _)| !(*w == window && *p == pane))
                        .find_map(|(_, _, other_state)| {
                            if other_state.link_group == group {
                                other_state.ticker_info
                            } else {
                                None
                            }
                        });

                    if let Some(state) = self.get_mut_pane(main_window.id, window, pane) {
                        state.link_group = group;
                        state.modal = None;

                        // If the group already has a ticker, switch this pane to it
                        if let Some(ticker_info) = group_ticker_info {
                            if state.ticker_info != Some(ticker_info) {
                                let pane_id = state.unique_id();
                                let task = self.switch_tickers_in_group(
                                    main_window.id,
                                    ticker_info,
                                    None,
                                    Some(pane_id),
                                );
                                return (task, None);
                            }
                        }
                    }
                }
                pane::Message::Popout => {
                    return (self.popout_pane(main_window), None);
                }
                pane::Message::Merge => {
                    return (self.merge_pane(main_window), None);
                }
                pane::Message::PaneEvent(pane, local) => {
                    if let Some(state) = self.get_mut_pane(main_window.id, window, pane) {
                        let Some(effect) = state.update(local) else {
                            return (Task::none(), None);
                        };

                        // Handle pane effects
                        let pane_id = state.unique_id();
                        let triggering_pane_link_group = state.link_group; // Capture link group BEFORE matching on effect
                        let (task, event) = match effect {
                            pane::Effect::LoadChart {
                                mut config,
                                ticker_info,
                            } => {
                                // Override placeholder range with canonical
                                // DataIndex resolution
                                if let Some(range) =
                                    data::lock_or_recover(&self.data_index)
                                        .resolve_chart_range(
                                            ticker_info.ticker.as_str(),
                                            config.chart_type,
                                        )
                                {
                                    config.date_range = range;
                                }
                                let event = self.load_chart(pane_id, config, ticker_info);
                                (Task::none(), Some(event))
                            }
                            pane::Effect::SwitchTickersInGroup(ticker_info) => {
                                // Switch tickers for all panes in the same link group
                                // If no link group, pass pane_id to switch just this single pane
                                let task = self.switch_tickers_in_group(
                                    main_window.id,
                                    ticker_info,
                                    triggering_pane_link_group,
                                    Some(pane_id),
                                );
                                (task, None)
                            }
                            pane::Effect::FocusWidget(id) => {
                                (iced::widget::operation::focus(id), None)
                            }
                            pane::Effect::EstimateDataCost {
                                ticker,
                                schema,
                                date_range,
                            } => {
                                // Trigger cost estimation
                                let task = Task::done(Message::EstimateDataCost {
                                    pane_id,
                                    ticker,
                                    schema,
                                    date_range,
                                });
                                (task, None)
                            }
                            pane::Effect::DownloadData {
                                ticker,
                                schema,
                                date_range,
                            } => {
                                // Trigger data download
                                let task = Task::done(Message::DownloadData {
                                    pane_id,
                                    ticker,
                                    schema,
                                    date_range,
                                });
                                (task, None)
                            }
                            pane::Effect::DrawingToolChanged(tool) => {
                                (Task::none(), Some(Event::DrawingToolChanged(tool)))
                            }
                        };
                        return (task, event);
                    }
                }
            },
            Message::ChangePaneStatus(pane_id, status) => {
                if let Some(pane_state) = self.get_mut_pane_state_by_uuid(main_window.id, pane_id) {
                    pane_state.loading_status = status;
                }
            }
            Message::ChartDataLoaded {
                pane_id,
                chart_data,
            } => {
                return (
                    self.handle_chart_data_loaded(main_window.id, pane_id, chart_data),
                    None,
                );
            }
            Message::LoadChart {
                pane_id,
                config,
                ticker_info,
            } => {
                let event = self.load_chart(pane_id, config, ticker_info);
                return (Task::none(), Some(event));
            }
            Message::Notification(toast) => {
                return (Task::none(), Some(Event::Notification(toast)));
            }
            Message::EstimateDataCost {
                pane_id,
                ticker,
                schema,
                date_range,
            } => {
                // This message should be forwarded to main - return as Event
                return (
                    Task::none(),
                    Some(Event::EstimateDataCost {
                        pane_id,
                        ticker,
                        schema,
                        date_range,
                    }),
                );
            }
            Message::DownloadData {
                pane_id,
                ticker,
                schema,
                date_range,
            } => {
                // This message should be forwarded to main - return as Event
                return (
                    Task::none(),
                    Some(Event::DownloadData {
                        pane_id,
                        ticker,
                        schema,
                        date_range,
                    }),
                );
            }
            Message::DataDownloadProgress {
                pane_id,
                current,
                total,
            } => {
                // Update progress in data management modal
                if let Some(pane_state) = self.get_mut_pane_state_by_uuid(main_window.id, pane_id)
                    && let Some(Modal::DataManagement(ref mut panel)) = pane_state.modal
                {
                    panel.set_download_progress(DownloadProgress::Downloading {
                        current_day: current,
                        total_days: total,
                    });
                }
            }
            Message::DataDownloadComplete {
                pane_id,
                days_downloaded,
            } => {
                // Mark download as complete in modal
                if let Some(pane_state) = self.get_mut_pane_state_by_uuid(main_window.id, pane_id)
                    && let Some(Modal::DataManagement(ref mut panel)) = pane_state.modal
                {
                    panel.set_download_progress(DownloadProgress::Complete { days_downloaded });
                }
            }
            Message::DrawingToolSelected(tool) => {
                // Set the drawing tool on the focused pane's chart
                if let Some((window_id, pane)) = self.focus
                    && let Some(state) = self.get_mut_pane(main_window.id, window_id, pane)
                {
                    state.content.set_drawing_tool(tool);
                }
            }
            Message::DrawingSnapToggled => {
                // Toggle snap mode on the focused pane's chart
                if let Some((window_id, pane)) = self.focus
                    && let Some(state) = self.get_mut_pane(main_window.id, window_id, pane)
                {
                    state.content.toggle_drawing_snap();
                }
            }
            Message::DrawingUndo => {
                if let Some((window_id, pane)) = self.focus
                    && let Some(state) = self.get_mut_pane(main_window.id, window_id, pane)
                    && let Some(chart) = state.content.drawing_chart_mut()
                {
                    chart.drawings_mut().undo();
                    chart.invalidate_drawings_cache();
                    chart.invalidate_crosshair_cache();
                }
            }
            Message::DrawingRedo => {
                if let Some((window_id, pane)) = self.focus
                    && let Some(state) = self.get_mut_pane(main_window.id, window_id, pane)
                    && let Some(chart) = state.content.drawing_chart_mut()
                {
                    chart.drawings_mut().redo();
                    chart.invalidate_drawings_cache();
                    chart.invalidate_crosshair_cache();
                }
            }
            Message::DrawingDuplicate => {
                if let Some((window_id, pane)) = self.focus
                    && let Some(state) = self.get_mut_pane(main_window.id, window_id, pane)
                    && let Some(chart) = state.content.drawing_chart_mut()
                {
                    let selected: Vec<_> =
                        chart.drawings().selected_ids().iter().copied().collect();
                    for id in selected {
                        if let Some(drawing) = chart.drawings().get(id) {
                            let mut clone = drawing.clone_with_new_id();
                            // Offset slightly so it's visually distinct
                            for point in &mut clone.points {
                                point.time += 5000; // 5 sec offset
                            }
                            chart.drawings_mut().add_drawing(clone);
                        }
                    }
                    chart.invalidate_drawings_cache();
                    chart.invalidate_crosshair_cache();
                }
            }
            Message::ScrollToLatest => {
                if let Some((window_id, pane)) = self.focus
                    && let Some(state) = self.get_mut_pane(main_window.id, window_id, pane)
                {
                    state.content.scroll_to_latest();
                }
            }
            Message::ZoomStep(factor) => {
                if let Some((window_id, pane)) = self.focus
                    && let Some(state) = self.get_mut_pane(main_window.id, window_id, pane)
                {
                    state.content.zoom_step(factor);
                }
            }
            Message::ReplayTrades(ticker_info, trades) => {
                for (_, state) in self.panes.iter_mut() {
                    if let Some(ti) = state.ticker_info {
                        if ti.ticker == ticker_info.ticker && state.is_replaying() {
                            for trade in &trades {
                                state.content.append_trade(trade);
                            }
                        }
                    }
                }
                // Also route to popout windows
                for (_, (popout_panes, _)) in self.popout.iter_mut() {
                    for (_, state) in popout_panes.iter_mut() {
                        if let Some(ti) = state.ticker_info {
                            if ti.ticker == ticker_info.ticker && state.is_replaying() {
                                for trade in &trades {
                                    state.content.append_trade(trade);
                                }
                            }
                        }
                    }
                }
            }
            Message::ReplayRebuild(ticker_info, trades) => {
                for (_, state) in self.panes.iter_mut() {
                    if let Some(ti) = state.ticker_info {
                        if ti.ticker == ticker_info.ticker && state.is_replaying() {
                            state.content.rebuild_from_trades(&trades);
                        }
                    }
                }
                // Also route to popout windows
                for (_, (popout_panes, _)) in self.popout.iter_mut() {
                    for (_, state) in popout_panes.iter_mut() {
                        if let Some(ti) = state.ticker_info {
                            if ti.ticker == ticker_info.ticker && state.is_replaying() {
                                state.content.rebuild_from_trades(&trades);
                            }
                        }
                    }
                }
            }
            Message::ReplaySyncPane { pane_id, trades } => {
                if let Some(pane_state) = self.get_mut_pane_state_by_uuid(main_window.id, pane_id) {
                    pane_state.enter_replay_mode();
                    pane_state.content.rebuild_from_trades(&trades);
                }
            }
            Message::ExchangeEvent(event) => {
                // Forward live streaming events to chart panes
                log::trace!("Dashboard received exchange event");
                match &event {
                    exchange::Event::TradeReceived(_stream_kind, trade) => {
                        // Convert exchange::Trade to domain::Trade and append
                        let domain_trade = data::Trade::from_raw(
                            trade.time,
                            trade.price,
                            trade.qty,
                            trade.side == exchange::TradeSide::Sell,
                        );
                        for chart_state in self.charts.values_mut() {
                            chart_state.append_live_trade(domain_trade);
                        }
                    }
                    exchange::Event::DepthReceived(_stream_kind, _ts, _depth, _trades) => {
                        // Depth updates handled by heatmap panes
                    }
                    _ => {}
                }
            }
        }

        (Task::none(), None)
    }
}
