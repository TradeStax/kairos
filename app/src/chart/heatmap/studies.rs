use crate::modals::pane::settings::study_config as study;

use super::HeatmapChart;
use super::render::HeatmapStudy;

impl HeatmapChart {
    /// Get study configurator
    pub fn study_configurator(&self) -> &study::Configurator<HeatmapStudy> {
        &self.study_configurator
    }

    /// Update study configurator (add/remove/configure studies)
    pub fn update_study_configurator(&mut self, message: study::Message<HeatmapStudy>) {
        let studies = &mut self.studies;

        match self.study_configurator.update(message) {
            Some(study::Action::ToggleStudy(study, is_selected)) => {
                if is_selected {
                    let already_exists = studies.iter().any(|s| s.is_same_type(&study));
                    if !already_exists {
                        studies.push(study);
                    }
                } else {
                    studies.retain(|s| !s.is_same_type(&study));
                }
            }
            Some(study::Action::ConfigureStudy(study)) => {
                if let Some(existing_study) =
                    studies.iter_mut().find(|s| s.is_same_type(&study))
                {
                    *existing_study = study;
                }
            }
            None => {}
        }

        self.invalidate(None);
    }

    /// Toggle indicator on/off
    pub fn toggle_indicator(&mut self, indicator: ::data::HeatmapIndicator) {
        self.indicators[indicator] = !self.indicators[indicator];
    }
}
