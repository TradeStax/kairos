use super::Content;
use crate::screen::dashboard::pane::config::StudyInstanceConfig;

impl Content {
    pub fn toggle_study(&mut self, study_id: &str) {
        macro_rules! toggle {
            ($chart:expr, $study_ids:expr) => {
                if let Some(pos) = $study_ids.iter().position(|id| id == study_id) {
                    $study_ids.remove(pos);
                    if let Some(c) = $chart {
                        c.remove_study(study_id);
                    }
                } else {
                    let registry = crate::app::init::services::create_unified_registry();
                    if let Some(study) = registry.create(study_id) {
                        $study_ids.push(study_id.to_string());
                        if let Some(c) = $chart {
                            c.add_study(study);
                        }
                    }
                }
            };
        }
        match self {
            Content::Candlestick {
                chart, study_ids, ..
            } => toggle!(&mut **chart, study_ids),
            Content::Profile {
                chart, study_ids, ..
            } => toggle!(&mut **chart, study_ids),
            _ => {}
        }
    }

    pub fn update_study_parameter(
        &mut self,
        study_id: &str,
        key: &str,
        value: study::ParameterValue,
    ) {
        match self {
            Content::Candlestick { chart, .. } => {
                if let Some(c) = (**chart).as_mut() {
                    c.update_study_parameter(study_id, key, value);
                }
            }
            Content::Profile { chart, .. } => {
                if let Some(c) = (**chart).as_mut() {
                    c.update_study_parameter(study_id, key, value);
                }
            }
            _ => {}
        }
    }

    /// Serialize active study configs for persistence
    pub fn serialize_studies(&self) -> Vec<StudyInstanceConfig> {
        macro_rules! serialize {
            ($chart:expr, $study_ids:expr) => {
                $chart
                    .studies()
                    .iter()
                    .map(|s| {
                        let parameters = s
                            .config()
                            .values
                            .iter()
                            .filter_map(|(k, v)| {
                                serde_json::to_value(v).ok().map(|jv| (k.clone(), jv))
                            })
                            .collect();
                        StudyInstanceConfig {
                            study_id: s.id().to_string(),
                            enabled: $study_ids.contains(&s.id().to_string()),
                            parameters,
                            config_version: s.metadata().config_version,
                        }
                    })
                    .collect()
            };
        }
        match self {
            Content::Candlestick {
                chart, study_ids, ..
            } => {
                if let Some(c) = (**chart).as_ref() {
                    serialize!(c, study_ids)
                } else {
                    vec![]
                }
            }
            Content::Profile {
                chart, study_ids, ..
            } => {
                if let Some(c) = (**chart).as_ref() {
                    serialize!(c, study_ids)
                } else {
                    vec![]
                }
            }
            _ => vec![],
        }
    }

    pub fn has_indicators(&self) -> bool {
        #[cfg(feature = "heatmap")]
        if matches!(self, Content::Heatmap { .. }) {
            return true;
        }
        matches!(self, Content::Candlestick { .. } | Content::Profile { .. })
    }
}
