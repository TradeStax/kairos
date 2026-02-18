use crate::component::primitives::label::title;
use crate::screen::dashboard::pane::{Event, Message};
use crate::split_column;
use crate::style::tokens;

use data::state::pane_config::{KlineConfig, VisualConfig};
use data::{ChartBasis, ClusterKind, FootprintStudy, KlineChartKind};

use iced::widget::pane_grid;
use iced::{
    Alignment, Element,
    widget::{column, pick_list, row, slider, space, text},
};

use super::common::{cfg_view_container, sync_all_button};
use super::study::{self, StudyMessage};

pub fn kline_cfg_view<'a>(
    cfg: KlineConfig,
    study_config: &'a study::Configurator<FootprintStudy>,
    kind: &'a KlineChartKind,
    pane: pane_grid::Pane,
    basis: ChartBasis,
) -> Element<'a, Message> {
    let content = match kind {
        KlineChartKind::Candles => column![text(
            "No configuration options for candle charts."
        )],
        KlineChartKind::Footprint {
            clusters,
            scaling,
            studies,
        } => {
            let cluster_picklist =
                pick_list(ClusterKind::ALL, Some(clusters), move |new_cluster_kind| {
                    Message::PaneEvent(pane, Event::ClusterKindSelected(new_cluster_kind))
                });

            let scaling = {
                let picklist = pick_list(
                    data::domain::chart_ui_types::ClusterScaling::ALL,
                    Some(scaling),
                    move |new_scaling| {
                        Message::PaneEvent(pane, Event::ClusterScalingSelected(new_scaling))
                    },
                );

                if let data::domain::chart_ui_types::ClusterScaling::Hybrid { weight } = scaling {
                    let hybrid_slider = slider(0.0..=1.0, *weight, move |new_weight| {
                        Message::PaneEvent(
                            pane,
                            Event::ClusterScalingSelected(
                                data::domain::chart_ui_types::ClusterScaling::Hybrid {
                                    weight: new_weight,
                                },
                            ),
                        )
                    })
                    .step(0.05);

                    column![
                        picklist,
                        hybrid_slider,
                        text("Blend visible-range and per-candle scaling"),
                    ]
                    .spacing(tokens::spacing::MD)
                } else {
                    column![picklist].spacing(tokens::spacing::MD)
                }
            };

            let study_cfg = study_config.view(studies, basis).map(move |msg| {
                Message::PaneEvent(pane, Event::StudyConfigurator(StudyMessage::Footprint(msg)))
            });

            split_column![
                column![title("Cluster type"), cluster_picklist]
                    .spacing(tokens::spacing::MD),
                column![title("Cluster scaling"), scaling]
                    .spacing(tokens::spacing::MD),
                column![title("Studies"), study_cfg]
                    .spacing(tokens::spacing::MD),
                row![
                    space::horizontal(),
                    sync_all_button(pane, VisualConfig::Kline(cfg))
                ],
                ; spacing = tokens::spacing::LG, align_x = Alignment::Start
            ]
        }
    };

    cfg_view_container(360, content)
}
