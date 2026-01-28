use crate::screen::dashboard::pane::{Event, Message};
use crate::split_column;
use crate::style;

use data::{ChartBasis, ClusterKind, FootprintStudy, KlineChartKind};
use data::state::pane_config::{VisualConfig, KlineConfig};

use iced::{
    Alignment, Element,
    widget::{column, pick_list, row, slider, space, text},
};
use iced::widget::pane_grid;

use super::study::{self, StudyMessage};
use super::common::{cfg_view_container, sync_all_button};

pub fn kline_cfg_view<'a>(
    cfg: KlineConfig,
    study_config: &'a study::Configurator<FootprintStudy>,
    kind: &'a KlineChartKind,
    pane: pane_grid::Pane,
    basis: ChartBasis,
) -> Element<'a, Message> {
    let content = match kind {
        KlineChartKind::Candles => column![text(
            "This chart type doesn't have any configurations, WIP..."
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
                                data::domain::chart_ui_types::ClusterScaling::Hybrid { weight: new_weight },
                            ),
                        )
                    })
                    .step(0.05);

                    column![
                        picklist,
                        hybrid_slider,
                        text("Blend visible-range and per-candle scaling"),
                    ]
                    .spacing(8)
                } else {
                    column![picklist].spacing(8)
                }
            };

            let study_cfg = study_config.view(studies, basis).map(move |msg| {
                Message::PaneEvent(
                    pane,
                    Event::StudyConfigurator(StudyMessage::Footprint(msg)),
                )
            });

            split_column![
                column![text("Cluster type").size(14), cluster_picklist].spacing(8),
                column![text("Cluster scaling").size(14), scaling].spacing(8),
                column![text("Studies").size(14), study_cfg].spacing(8),
                row![
                    space::horizontal(),
                    sync_all_button(pane, VisualConfig::Kline(cfg))
                ],
                ; spacing = 12, align_x = Alignment::Start
            ]
        }
    };

    cfg_view_container(360, content)
}
