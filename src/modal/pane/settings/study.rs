use crate::{
    split_column,
    style::{self, Icon, icon_text},
};
use data::ChartBasis;
use data::domain::chart_ui_types::heatmap::{CLEANUP_THRESHOLD, HeatmapStudy, ProfileKind};
use data::domain::chart_ui_types::FootprintStudy;
use iced::{
    Element, padding,
    widget::{button, checkbox, column, container, row, slider, space, text},
};

#[derive(Debug, Clone, Copy)]
pub enum StudyMessage {
    Footprint(Message<FootprintStudy>),
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

impl Study for FootprintStudy {
    fn is_same_type(&self, other: &Self) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }

    fn all() -> Vec<Self> {
        FootprintStudy::ALL.to_vec()
    }

    fn view_config<'a>(
        &self,
        _basis: ChartBasis,
        on_change: impl Fn(Self) -> Message<Self> + Copy + 'a,
    ) -> Element<'a, Message<Self>> {
        match *self {
            FootprintStudy::Imbalance {
                threshold,
                ignore_zeros,
                color_scale,
            } => {
                let qty_threshold = {
                    let info_text = text(format!("Ask:Bid threshold: {threshold}%"));

                    let threshold_slider =
                        slider(100.0..=800.0, threshold as f32, move |new_value| {
                            on_change(FootprintStudy::Imbalance {
                                threshold: new_value as u8,
                                color_scale,
                                ignore_zeros,
                            })
                        })
                        .step(25.0);

                    column![info_text, threshold_slider,].padding(8).spacing(4)
                };

                let color_scaling = {
                    let color_scale_enabled = color_scale;
                    // TODO: Add `color_scale_value: u16` field to
                    // FootprintStudy::Imbalance to persist this setting.
                    // For now the slider is display-only at a fixed value.
                    let color_scale_value: u16 = 100;

                    let color_scale_checkbox = checkbox(color_scale_enabled)
                        .label("Dynamic color scaling")
                        .on_toggle(move |is_enabled| {
                            on_change(FootprintStudy::Imbalance {
                                threshold,
                                color_scale: is_enabled,
                                ignore_zeros,
                            })
                        });

                    if color_scale_enabled {
                        let scaling_slider = column![
                            text(format!("Opaque color at: {color_scale_value}x")),
                            slider(50.0..=2000.0, color_scale_value as f32, move |_new_value| {
                                // No-op: FootprintStudy::Imbalance lacks a
                                // color_scale_value field to store slider state
                                on_change(FootprintStudy::Imbalance {
                                    threshold,
                                    color_scale: true,
                                    ignore_zeros,
                                })
                            })
                            .step(50.0)
                        ]
                        .spacing(2);

                        column![color_scale_checkbox, scaling_slider]
                            .padding(8)
                            .spacing(8)
                    } else {
                        column![color_scale_checkbox].padding(8)
                    }
                };

                let ignore_zeros_checkbox = {
                    let cbox = checkbox(ignore_zeros).label("Ignore zeros").on_toggle(
                        move |is_checked| {
                            on_change(FootprintStudy::Imbalance {
                                threshold,
                                color_scale,
                                ignore_zeros: is_checked,
                            })
                        },
                    );

                    column![cbox].padding(8).spacing(4)
                };

                split_column![qty_threshold, color_scaling, ignore_zeros_checkbox]
                    .padding(4)
                    .into()
            }
            FootprintStudy::NPoC { lookback } => {
                let slider_ui = slider(10.0..=400.0, lookback as f32, move |new_value| {
                    on_change(FootprintStudy::NPoC {
                        lookback: new_value as usize,
                    })
                })
                .step(10.0);

                column![text(format!("Lookback: {lookback} datapoints")), slider_ui]
                    .padding(8)
                    .spacing(4)
                    .into()
            }
            FootprintStudy::PointOfControl => {
                column![text("Point of Control - no configuration")].padding(8).into()
            }
            FootprintStudy::ValueArea => {
                column![text("Value Area - no configuration")].padding(8).into()
            }
        }
    }
}

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
                ProfileKind::FixedWindow { candles: datapoint_count } => {
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

                    let switch_kind = button(text("Switch to visible range")).on_press(
                        on_change(HeatmapStudy::VolumeProfile(ProfileKind::VisibleRange)),
                    );

                    column![
                        row![space::horizontal(), switch_kind,],
                        text(format!(
                            "Window: {} datapoints ({})",
                            datapoint_count, duration_text
                        )),
                        slider,
                    ]
                    .padding(8)
                    .spacing(4)
                    .into()
                }
                ProfileKind::VisibleRange => {
                    let switch_kind = button(text("Switch to fixed window")).on_press(
                        on_change(HeatmapStudy::VolumeProfile(ProfileKind::FixedWindow {
                            candles: CLEANUP_THRESHOLD / 5,
                        })),
                    );

                    column![row![space::horizontal(), switch_kind,],]
                        .padding(8)
                        .spacing(4)
                        .into()
                }
                ProfileKind::Fixed(_n) => {
                    let switch_kind = button(text("Switch to visible range")).on_press(
                        on_change(HeatmapStudy::VolumeProfile(ProfileKind::VisibleRange)),
                    );

                    column![row![space::horizontal(), switch_kind,],]
                        .padding(8)
                        .spacing(4)
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
        let mut content = column![].spacing(4);

        for available_study in S::all() {
            content =
                content.push(self.create_study_row(available_study, active_studies, basis));
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
            .padding(padding::left(8).right(4))
            .spacing(4);

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
