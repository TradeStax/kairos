use crate::app::messages::BacktestMessage;
use crate::components::display::toast::Toast;
use crate::screen::backtest::launch::Action as BacktestLaunchAction;
use crate::screen::backtest::manager::ManagerAction;
use iced::Task;
use std::sync::Arc;

use super::super::{Kairos, Message};

impl Kairos {
    pub(crate) fn handle_backtest_message(
        &mut self,
        msg: BacktestMessage,
    ) -> Task<Message> {
        match msg {
            BacktestMessage::OpenLaunchModal => {
                log::info!("Opening backtest launch modal");
                let idx = data::lock_or_recover(&self.persistence.data_index);
                self.modals.backtest.backtest_launch_modal =
                    crate::screen::backtest::launch::BacktestLaunchModal::new(
                        &self.modals.backtest.strategy_registry,
                        &idx,
                    );
                self.modals.backtest.show_backtest_modal = true;
                Task::none()
            }

            BacktestMessage::OpenManager => {
                self.modals.backtest.show_backtest_manager = true;
                Task::none()
            }

            BacktestMessage::LaunchModalInteraction(modal_msg) => {
                match self.modals.backtest.backtest_launch_modal.update(modal_msg)
                {
                    Some(BacktestLaunchAction::RunRequested(config)) => {
                        self.modals.backtest.show_backtest_modal = false;
                        self.modals.backtest.backtest_launch_modal.set_running(true);
                        Task::done(Message::Backtest(
                            BacktestMessage::Run { config },
                        ))
                    }
                    Some(BacktestLaunchAction::Closed) => {
                        self.modals.backtest.show_backtest_modal = false;
                        Task::none()
                    }
                    None => Task::none(),
                }
            }

            BacktestMessage::Run { config } => {
                let Some(trade_repo) =
                    self.modals.backtest.backtest_trade_repo.clone()
                else {
                    log::warn!(
                        "Backtest requires a Databento API key \
                         — trade repository not available"
                    );
                    return Task::none();
                };

                let run_id = uuid::Uuid::new_v4();
                let strategy_name = {
                    let sid = &config.strategy_id;
                    self.modals.backtest.strategy_registry
                        .list()
                        .iter()
                        .find(|i| i.id == *sid)
                        .map(|i| i.name.clone())
                        .unwrap_or_else(|| sid.clone())
                };
                let ticker = config.ticker.as_str().to_string();

                self.modals.backtest.backtest_history.add_running(
                    run_id,
                    strategy_name.clone(),
                    ticker.clone(),
                    config.clone(),
                );

                // Auto-open manager and select running backtest
                self.modals.backtest.show_backtest_manager = true;
                self.modals.backtest.backtest_manager.select(
                    run_id,
                    &self.modals.backtest.backtest_history,
                );

                let strategy_registry = self.modals.backtest.strategy_registry.clone();
                let backtest_sender =
                    super::super::core::globals::get_backtest_sender();

                Task::perform(
                    async move {
                        let mut strategy = strategy_registry
                            .create(&config.strategy_id)
                            .ok_or_else(|| {
                                format!(
                                    "Unknown strategy: {}",
                                    config.strategy_id
                                )
                            })?;

                        for (key, value) in &config.strategy_params {
                            strategy
                                .set_parameter(key, value.clone())
                                .map_err(|e| format!("{key}: {e}"))?;
                        }

                        let runner =
                            backtest::BacktestRunner::new(trade_repo);
                        runner
                            .run_with_progress(
                                config,
                                strategy,
                                run_id,
                                backtest_sender,
                            )
                            .await
                            .map(Box::new)
                            .map_err(|e| e.to_string())
                    },
                    move |result| match result {
                        Ok(r) => Message::Backtest(
                            BacktestMessage::Completed {
                                run_id,
                                result: r,
                            },
                        ),
                        Err(e) => Message::Backtest(
                            BacktestMessage::Failed {
                                run_id,
                                error: e,
                            },
                        ),
                    },
                )
            }

            BacktestMessage::ProgressEvent(event) => {
                use backtest::BacktestProgressEvent;
                match event {
                    BacktestProgressEvent::TradeCompleted {
                        run_id,
                        trade,
                    } => {
                        self.modals.backtest.backtest_history
                            .append_live_trade(run_id, trade);
                    }
                    BacktestProgressEvent::SessionProcessed {
                        run_id,
                        index,
                        total_estimated,
                    } => {
                        let pct = index as f32
                            / total_estimated.max(1) as f32;
                        self.modals.backtest.backtest_history.update_progress(
                            run_id,
                            pct,
                            format!(
                                "Session {}/{}",
                                index, total_estimated
                            ),
                        );
                    }
                    BacktestProgressEvent::EquityUpdate {
                        run_id,
                        point,
                    } => {
                        self.modals.backtest.backtest_history
                            .append_live_equity(run_id, point);
                    }
                }
                Task::none()
            }

            BacktestMessage::Completed { run_id, result } => {
                self.modals.backtest.backtest_launch_modal.set_running(false);
                self.modals.backtest.backtest_history.mark_completed(
                    run_id,
                    Arc::new(*result.clone()),
                );
                log::info!(
                    "Backtest complete: {} trades, net PnL ${:.2}",
                    result.trades.len(),
                    result.metrics.net_pnl_usd,
                );
                // Auto-open manager and select the completed backtest
                self.modals.backtest.show_backtest_manager = true;
                self.modals.backtest.backtest_manager.select(
                    run_id,
                    &self.modals.backtest.backtest_history,
                );
                Task::none()
            }

            BacktestMessage::Failed { run_id, error } => {
                self.modals.backtest.backtest_launch_modal.set_running(false);
                self.modals.backtest.backtest_history
                    .mark_failed(run_id, error.clone());
                self.modals.backtest.backtest_launch_modal.validation_error =
                    Some(format!("Run failed: {}", error));
                self.modals.backtest.show_backtest_modal = true;
                log::error!("Backtest failed: {error}");
                Task::none()
            }

            BacktestMessage::CsvExported(outcome) => {
                match outcome {
                    Some(Ok(path)) => {
                        self.ui.notifications.push(Toast::success(
                            format!(
                                "CSV exported to {}",
                                path.display()
                            ),
                        ));
                    }
                    Some(Err(e)) => {
                        self.ui.notifications.push(Toast::error(
                            format!("CSV export failed: {e}"),
                        ));
                    }
                    None => {}
                }
                Task::none()
            }

            BacktestMessage::ManagerInteraction(manager_msg) => {
                let action = self.modals.backtest.backtest_manager.update(
                    manager_msg,
                    &self.modals.backtest.backtest_history,
                );
                match action {
                    ManagerAction::None => Task::none(),
                    ManagerAction::OpenLaunchModal => {
                        self.modals.backtest.show_backtest_manager = false;
                        Task::done(Message::Backtest(
                            BacktestMessage::OpenLaunchModal,
                        ))
                    }
                    ManagerAction::DeleteBacktest(id) => {
                        self.modals.backtest.backtest_history.remove(id);
                        // If we deleted the selected one, deselect
                        if self.modals.backtest.backtest_manager.selected_id
                            == Some(id)
                        {
                            self.modals.backtest.backtest_manager.selected_id = None;
                            self.modals.backtest.backtest_manager.analytics = None;
                        }
                        Task::none()
                    }
                    ManagerAction::ExportCsv(id) => {
                        self.export_backtest_csv(id)
                    }
                    ManagerAction::Close => {
                        self.modals.backtest.show_backtest_manager = false;
                        Task::none()
                    }
                }
            }
        }
    }

    /// Export a backtest's trade list to CSV via a native save dialog.
    fn export_backtest_csv(
        &self,
        backtest_id: uuid::Uuid,
    ) -> Task<Message> {
        let Some(entry) = self.modals.backtest.backtest_history.get(backtest_id) else {
            return Task::none();
        };
        let Some(result) = &entry.result else {
            return Task::none();
        };

        let mut csv = String::from(
            "Trade,Entry Time,Exit Time,Side,P&L ($),P&L (ticks),\
             MAE (ticks),MFE (ticks),Exit Reason\n",
        );
        for (i, t) in result.trades.iter().enumerate() {
            let side = if t.side == data::Side::Buy {
                "Long"
            } else {
                "Short"
            };
            csv.push_str(&format!(
                "{},{},{},{},{:.2},{},{},{},{}\n",
                i + 1,
                t.entry_time.0,
                t.exit_time.0,
                side,
                t.pnl_net_usd,
                t.pnl_ticks,
                t.mae_ticks,
                t.mfe_ticks,
                t.exit_reason,
            ));
        }

        let default_name = format!(
            "backtest_{}.csv",
            entry.strategy_name.replace(' ', "_"),
        );

        Task::perform(
            async move {
                let handle = rfd::AsyncFileDialog::new()
                    .set_title("Export Backtest CSV")
                    .set_file_name(&default_name)
                    .add_filter("CSV Files", &["csv"])
                    .save_file()
                    .await;

                let Some(handle) = handle else {
                    return None;
                };

                let path = handle.path().to_path_buf();
                match tokio::fs::write(&path, csv).await {
                    Ok(()) => {
                        log::info!(
                            "CSV exported to {}",
                            path.display()
                        );
                        Some(Ok(path))
                    }
                    Err(e) => {
                        log::error!(
                            "Failed to write CSV: {e}"
                        );
                        Some(Err(e.to_string()))
                    }
                }
            },
            |outcome| {
                Message::Backtest(BacktestMessage::CsvExported(
                    outcome,
                ))
            },
        )
    }
}
