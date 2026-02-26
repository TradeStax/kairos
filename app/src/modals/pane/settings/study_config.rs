use crate::{
    components::primitives::{Icon, icon_text},
    style,
    style::tokens,
};
use data::ChartBasis;
#[cfg(feature = "heatmap")]
use data::domain::chart::heatmap::heatmap::{CLEANUP_THRESHOLD, HeatmapStudy, ProfileKind};
use iced::{
    Element, padding,
    widget::{button, checkbox, column, container, row, space},
};

#[derive(Debug, Clone, Copy)]
pub enum StudyMessage {
    #[cfg(feature = "heatmap")]
    Heatmap(Message<HeatmapStudy>),
}

pub trait Study: Sized + Copy + ToString + 'static {
    fn is_same_type(&self, other: &Self) -> bool;
    fn all() -> Vec<Self>;
    fn view_config<'a>(
        &self,
        basis: ChartBasis,
        on_change: impl Fn(Self) -> Message<Self> + Copy + 'a,
    ) -> Element<'a, Message<Self>>;
}

#[cfg(feature = "heatmap")]
impl Study for HeatmapStudy {
    fn is_same_type(&self, other: &Self) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }

    fn all() -> Vec<Self> {
        HeatmapStudy::ALL.to_vec()
    }

    fn view_config<'a>(
        &self,
        basis: ChartBasis,
        on_change: impl Fn(Self) -> Message<Self> + Copy + 'a,
    ) -> Element<'a, Message<Self>> {
        let interval_ms = match basis {
            ChartBasis::Time(interval) => interval.to_milliseconds(),
            ChartBasis::Tick(_) => {
                return iced::widget::center(text(
                    "Heatmap studies are not supported for tick-based charts",
                ))
                .into();
            }
        };

        match self {
            HeatmapStudy::VolumeProfile(kind) => match kind {
                ProfileKind::FixedWindow {
                    candles: datapoint_count,
                } => {
                    let duration_secs = (*datapoint_count as u64 * interval_ms) / 1000;
                    let min_range = CLEANUP_THRESHOLD / 20;

                    let duration_text = if duration_secs < 60 {
                        format!("{} seconds", duration_secs)
                    } else {
                        let minutes = duration_secs / 60;
                        let seconds = duration_secs % 60;
                        if seconds == 0 {
                            format!("{} minutes", minutes)
                        } else {
                            format!("{}m {}s", minutes, seconds)
                        }
                    };

                    let slider = slider(
                        min_range as f32..=CLEANUP_THRESHOLD as f32,
                        *datapoint_count as f32,
                        move |new_datapoint_count| {
                            on_change(HeatmapStudy::VolumeProfile(ProfileKind::FixedWindow {
                                candles: new_datapoint_count as usize,
                            }))
                        },
                    )
                    .step(40.0);

                    let switch_kind = button(text("Switch to visible range")).on_press(on_change(
                        HeatmapStudy::VolumeProfile(ProfileKind::VisibleRange),
                    ));

                    column![
                        row![space::horizontal(), switch_kind,],
                        text(format!(
                            "Window: {} datapoints ({})",
                            datapoint_count, duration_text
                        )),
                        slider,
                    ]
                    .padding(tokens::spacing::MD)
                    .spacing(tokens::spacing::XS)
                    .into()
                }
                ProfileKind::VisibleRange => {
                    let switch_kind = button(text("Switch to fixed window")).on_press(on_change(
                        HeatmapStudy::VolumeProfile(ProfileKind::FixedWindow {
                            candles: CLEANUP_THRESHOLD / 5,
                        }),
                    ));

                    column![row![space::horizontal(), switch_kind,],]
                        .padding(tokens::spacing::MD)
                        .spacing(tokens::spacing::XS)
                        .into()
                }
                ProfileKind::Fixed(_n) => {
                    let switch_kind = button(text("Switch to visible range")).on_press(on_change(
                        HeatmapStudy::VolumeProfile(ProfileKind::VisibleRange),
                    ));

                    column![row![space::horizontal(), switch_kind,],]
                        .padding(tokens::spacing::MD)
                        .spacing(tokens::spacing::XS)
                        .into()
                }
            },
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Message<S: Study> {
    CardToggled(S),
    StudyToggled(S, bool),
    StudyValueChanged(S),
}

pub enum Action<S: Study> {
    ToggleStudy(S, bool),
    ConfigureStudy(S),
}

pub struct Configurator<S: Study> {
    expanded_card: Option<S>,
}

impl<S: Study> Default for Configurator<S> {
    fn default() -> Self {
        Self {
            expanded_card: None,
        }
    }
}

impl<S: Study> Configurator<S> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update(&mut self, message: Message<S>) -> Option<Action<S>> {
        match message {
            Message::CardToggled(study) => {
                let should_collapse = self
                    .expanded_card
                    .as_ref()
                    .is_some_and(|expanded| expanded.is_same_type(&study));

                if should_collapse {
                    self.expanded_card = None;
                } else {
                    self.expanded_card = Some(study);
                }
            }
            Message::StudyToggled(study, is_checked) => {
                return Some(Action::ToggleStudy(study, is_checked));
            }
            Message::StudyValueChanged(study) => {
                return Some(Action::ConfigureStudy(study));
            }
        }

        None
    }

    pub fn view<'a>(
        &self,
        active_studies: &'a [S],
        basis: data::ChartBasis,
    ) -> Element<'a, Message<S>> {
        let mut content = column![].spacing(tokens::spacing::XS);

        for available_study in S::all() {
            content = content.push(self.create_study_row(available_study, active_studies, basis));
        }

        content.into()
    }

    fn create_study_row<'a>(
        &self,
        study: S,
        active_studies: &'a [S],
        basis: data::ChartBasis,
    ) -> Element<'a, Message<S>> {
        let (is_selected, study_config) = {
            let mut is_selected = false;
            let mut study_config = None;

            for s in active_studies {
                if s.is_same_type(&study) {
                    is_selected = true;
                    study_config = Some(*s);
                    break;
                }
            }
            (is_selected, study_config)
        };

        let checkbox = checkbox(is_selected)
            .label(study_config.map_or(study.to_string(), |s| s.to_string()))
            .on_toggle(move |checked| Message::StudyToggled(study, checked));

        let mut checkbox_row = row![checkbox, space::horizontal()]
            .height(36)
            .align_y(iced::Alignment::Center)
            .padding(padding::left(tokens::spacing::MD).right(tokens::spacing::XS))
            .spacing(tokens::spacing::XS);

        let is_expanded = self
            .expanded_card
            .as_ref()
            .is_some_and(|expanded| expanded.is_same_type(&study));

        if is_selected {
            checkbox_row = checkbox_row.push(
                button(icon_text(Icon::Cog, 12))
                    .on_press(Message::CardToggled(study))
                    .style(move |theme, status| {
                        style::button::transparent(theme, status, is_expanded)
                    }),
            );
        }

        let mut column = column![checkbox_row];

        if is_expanded && let Some(config) = study_config {
            column = column.push(config.view_config(basis, Message::StudyValueChanged));
        }

        container(column).style(style::modal_container).into()
    }
}
