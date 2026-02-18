use iced::Task;

use crate::widget::toast::{Notification, Toast};

use super::super::{Flowsurface, Message, OptionsMessage};

impl Flowsurface {
    pub(crate) fn handle_load_option_chain(
        &mut self,
        pane_id: uuid::Uuid,
        underlying_ticker: String,
        date: chrono::NaiveDate,
    ) -> Task<Message> {
        let secrets = data::SecretsManager::new();
        if !secrets.has_api_key(data::ApiProvider::Massive) {
            log::warn!("Massive API key not configured");
            self.notifications
                .push(Toast::error("Massive API key not configured.".to_string()));
            return Task::none();
        }

        if let Some(service) = self.options_service.clone() {
            return Task::perform(
                async move {
                    service
                        .get_chain_with_greeks(&underlying_ticker, date)
                        .await
                        .map_err(|e| e.to_string())
                },
                move |result| {
                    Message::Options(OptionsMessage::OptionChainLoaded { pane_id, result })
                },
            );
        } else {
            log::warn!("Options service not available - reinitializing may be required");
            self.notifications.push(Toast::error(
                "Options service not initialized - try reconfiguring API key".to_string(),
            ));
        }
        Task::none()
    }

    pub(crate) fn handle_option_chain_loaded(
        &mut self,
        pane_id: uuid::Uuid,
        result: Result<data::domain::OptionChain, String>,
    ) {
        match result {
            Ok(chain) => {
                log::info!(
                    "Option chain loaded for pane {}: {} contracts for {}",
                    pane_id,
                    chain.contract_count(),
                    chain.underlying_ticker
                );
                self.notifications
                    .push(Toast::new(Notification::Info(format!(
                        "Loaded {} option contracts",
                        chain.contract_count()
                    ))));
            }
            Err(e) => {
                log::error!("Failed to load option chain for pane {}: {}", pane_id, e);
                self.notifications
                    .push(Toast::error(format!("Failed to load option chain: {}", e)));
            }
        }
    }

    pub(crate) fn handle_load_gex_profile(
        &mut self,
        pane_id: uuid::Uuid,
        underlying_ticker: String,
        date: chrono::NaiveDate,
    ) -> Task<Message> {
        let secrets = data::SecretsManager::new();
        if !secrets.has_api_key(data::ApiProvider::Massive) {
            log::warn!("Massive API key not configured");
            self.notifications
                .push(Toast::error("Massive API key not configured.".to_string()));
            return Task::none();
        }

        if let Some(service) = self.options_service.clone() {
            return Task::perform(
                async move {
                    service
                        .get_gex_profile(&underlying_ticker, date)
                        .await
                        .map_err(|e| e.to_string())
                },
                move |result| {
                    Message::Options(OptionsMessage::GexProfileLoaded { pane_id, result })
                },
            );
        } else {
            log::warn!("Options service not available - reinitializing may be required");
            self.notifications.push(Toast::error(
                "Options service not initialized - try reconfiguring API key".to_string(),
            ));
        }
        Task::none()
    }

    pub(crate) fn handle_gex_profile_loaded(
        &mut self,
        pane_id: uuid::Uuid,
        result: Result<data::domain::GexProfile, String>,
    ) {
        match result {
            Ok(profile) => {
                log::info!(
                    "GEX profile loaded for pane {}: {} exposure levels for {}",
                    pane_id,
                    profile.exposure_count(),
                    profile.underlying_ticker
                );

                if let Some(zero_gamma) = profile.zero_gamma_level {
                    log::info!("Zero gamma level: ${:.2}", zero_gamma.to_f64());
                }

                self.notifications
                    .push(Toast::new(Notification::Info(format!(
                        "Loaded GEX: {} key levels",
                        profile.key_levels.len()
                    ))));
            }
            Err(e) => {
                log::error!("Failed to load GEX profile for pane {}: {}", pane_id, e);
                self.notifications
                    .push(Toast::error(format!("Failed to load GEX: {}", e)));
            }
        }
    }
}
