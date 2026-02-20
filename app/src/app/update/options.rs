use crate::components::display::toast::{Notification, Toast};

use super::super::Kairos;

impl Kairos {
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
