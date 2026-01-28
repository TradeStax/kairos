use crate::screen::dashboard::pane::{Event, Message};
use crate::split_column;
use crate::style;
use crate::widget::{classic_slider_row, labeled_slider};

use data::ChartBasis;
use data::domain::chart_ui_types::heatmap::{CoalesceKind, HeatmapStudy};
use data::state::pane_config::{VisualConfig, HeatmapConfig};
use data::util::format_with_commas;

use iced::{
    Alignment, Element,
    widget::{checkbox, column, container, pane_grid, radio, row, slider, space, text},
};

use super::study::{self, StudyMessage};
use super::common::{cfg_view_container, sync_all_button};

pub fn heatmap_cfg_view<'a>(
    cfg: HeatmapConfig,
    pane: pane_grid::Pane,
    study_config: &'a study::Configurator<HeatmapStudy>,
    studies: &'a [HeatmapStudy],
    basis: ChartBasis,
) -> Element<'a, Message> {
    let trade_size_slider = {
        let filter = cfg.trade_size_filter;
        labeled_slider(
            "Min trade size (contracts)",
            0.0..=1000.0,
            filter,
            move |value| {
                Message::VisualConfigChanged(
                    pane,
                    VisualConfig::Heatmap(HeatmapConfig {
                        trade_size_filter: value,
                        order_size_filter: cfg.order_size_filter,
                        trade_size_scale: cfg.trade_size_scale,
                        coalescing: cfg.coalescing,
                        rendering_mode: cfg.rendering_mode,
                        max_trade_markers: cfg.max_trade_markers,
                        performance_preset: None,
                    }),
                    false,
                )
            },
            |value| {
                if *value == 0.0 {
                    "Show all".to_string()
                } else {
                    format!("≥ {} contracts", value.round())
                }
            },
            Some(10.0),
        )
    };

    let order_size_slider = {
        let filter = cfg.order_size_filter;
        labeled_slider(
            "Min order size (contracts)",
            0.0..=10000.0,
            filter,
            move |value| {
                Message::VisualConfigChanged(
                    pane,
                    VisualConfig::Heatmap(HeatmapConfig {
                        trade_size_filter: cfg.trade_size_filter,
                        order_size_filter: value,
                        trade_size_scale: cfg.trade_size_scale,
                        coalescing: cfg.coalescing,
                        rendering_mode: cfg.rendering_mode,
                        max_trade_markers: cfg.max_trade_markers,
                        performance_preset: None,
                    }),
                    false,
                )
            },
            |value| {
                if *value == 0.0 {
                    "Show all".to_string()
                } else {
                    format!("≥ {} contracts", format_with_commas(*value))
                }
            },
            Some(100.0),
        )
    };

    let circle_scaling_slider = cfg.trade_size_scale.map(|radius_scale| {
        classic_slider_row(
            text("Circle radius scaling"),
            slider(10..=200, radius_scale, move |value| {
                Message::VisualConfigChanged(
                    pane,
                    VisualConfig::Heatmap(HeatmapConfig {
                        trade_size_filter: cfg.trade_size_filter,
                        order_size_filter: cfg.order_size_filter,
                        trade_size_scale: Some(value),
                        coalescing: cfg.coalescing,
                        rendering_mode: cfg.rendering_mode,
                        max_trade_markers: cfg.max_trade_markers,
                        performance_preset: None,
                    }),
                    false,
                )
            })
            .step(10u16)
            .into(),
            Some(text(format!("{}%", radius_scale)).size(13)),
        )
    });

    let coalescer_cfg: Option<Element<_>> = if let Some(coalescing) = cfg.coalescing {
        let threshold_pct = coalescing.threshold();

        let coalescer_kinds = {
            let average = radio(
                "Average",
                CoalesceKind::Average(threshold_pct),
                Some(coalescing),
                move |value| {
                    Message::VisualConfigChanged(
                        pane,
                        VisualConfig::Heatmap(HeatmapConfig {
                            trade_size_filter: cfg.trade_size_filter,
                            order_size_filter: cfg.order_size_filter,
                            trade_size_scale: cfg.trade_size_scale,
                            coalescing: Some(value),
                            rendering_mode: cfg.rendering_mode,
                            max_trade_markers: cfg.max_trade_markers,
                            performance_preset: None,
                        }),
                        false,
                    )
                },
            )
            .spacing(4);

            let first = radio(
                "First",
                CoalesceKind::First(threshold_pct),
                Some(coalescing),
                move |value| {
                    Message::VisualConfigChanged(
                        pane,
                        VisualConfig::Heatmap(HeatmapConfig {
                            trade_size_filter: cfg.trade_size_filter,
                            order_size_filter: cfg.order_size_filter,
                            trade_size_scale: cfg.trade_size_scale,
                            coalescing: Some(value),
                            rendering_mode: cfg.rendering_mode,
                            max_trade_markers: cfg.max_trade_markers,
                            performance_preset: None,
                        }),
                        false,
                    )
                },
            )
            .spacing(4);

            let max = radio(
                "Max",
                CoalesceKind::Max(threshold_pct),
                Some(coalescing),
                move |value| {
                    Message::VisualConfigChanged(
                        pane,
                        VisualConfig::Heatmap(HeatmapConfig {
                            trade_size_filter: cfg.trade_size_filter,
                            order_size_filter: cfg.order_size_filter,
                            trade_size_scale: cfg.trade_size_scale,
                            coalescing: Some(value),
                            rendering_mode: cfg.rendering_mode,
                            max_trade_markers: cfg.max_trade_markers,
                            performance_preset: None,
                        }),
                        false,
                    )
                },
            )
            .spacing(4);

            row![
                text("Merge method: "),
                row![average, first, max].spacing(12)
            ]
            .spacing(12)
        };

        let threshold_slider = classic_slider_row(
            text("Size similarity"),
            slider(0.05..=0.8, threshold_pct, move |value| {
                Message::VisualConfigChanged(
                    pane,
                    VisualConfig::Heatmap(HeatmapConfig {
                        trade_size_filter: cfg.trade_size_filter,
                        order_size_filter: cfg.order_size_filter,
                        trade_size_scale: cfg.trade_size_scale,
                        coalescing: Some(coalescing.with_threshold(value)),
                        rendering_mode: cfg.rendering_mode,
                        max_trade_markers: cfg.max_trade_markers,
                        performance_preset: None,
                    }),
                    false,
                )
            })
            .step(0.05)
            .into(),
            Some(text(format!("{:.0}%", threshold_pct * 100.0)).size(13)),
        );

        Some(
            container(column![coalescer_kinds, threshold_slider].spacing(8))
                .style(style::modal_container)
                .padding(8)
                .into(),
        )
    } else {
        None
    };

    let size_filters_column = column![
        text("Size filters").size(14),
        column![trade_size_slider, order_size_slider].spacing(8),
    ]
    .spacing(8);

    let noise_filters_column = {
        let merge_checkbox = checkbox(cfg.coalescing.is_some())
            .label("Merge orders if sizes are similar")
            .on_toggle(move |value| {
                Message::VisualConfigChanged(
                    pane,
                    VisualConfig::Heatmap(HeatmapConfig {
                        trade_size_filter: cfg.trade_size_filter,
                        order_size_filter: cfg.order_size_filter,
                        trade_size_scale: cfg.trade_size_scale,
                        coalescing: if value {
                            Some(CoalesceKind::Average(0.15))
                        } else {
                            None
                        },
                        rendering_mode: cfg.rendering_mode,
                        max_trade_markers: cfg.max_trade_markers,
                        performance_preset: None,
                    }),
                    false,
                )
            });

        let mut col = column![text("Noise filters").size(14), merge_checkbox].spacing(8);
        if let Some(c) = coalescer_cfg {
            col = col.push(c);
        }
        col
    };

    let trade_viz_column = {
        let dyn_checkbox = checkbox(cfg.trade_size_scale.is_some())
            .label("Dynamic circle radius")
            .on_toggle(move |value| {
                Message::VisualConfigChanged(
                    pane,
                    VisualConfig::Heatmap(HeatmapConfig {
                        trade_size_filter: cfg.trade_size_filter,
                        order_size_filter: cfg.order_size_filter,
                        trade_size_scale: if value { Some(100) } else { None },
                        coalescing: cfg.coalescing,
                        rendering_mode: cfg.rendering_mode,
                        max_trade_markers: cfg.max_trade_markers,
                        performance_preset: None,
                    }),
                    false,
                )
            });

        let mut col = column![text("Trade visualization").size(14), dyn_checkbox].spacing(8);
        if let Some(slider) = circle_scaling_slider {
            col = col.push(slider);
        }
        col
    };

    let study_cfg = study_config.view(studies, basis).map(move |msg| {
        Message::PaneEvent(
            pane,
            Event::StudyConfigurator(StudyMessage::Heatmap(msg)),
        )
    });

    let content = split_column![
        size_filters_column,
        noise_filters_column,
        trade_viz_column,
        column![text("Studies").size(14), study_cfg].spacing(8),
        row![
            space::horizontal(),
            sync_all_button(pane, VisualConfig::Heatmap(cfg.clone()))
        ]
        ; spacing = 12, align_x = Alignment::Start
    ];

    cfg_view_container(360, content)
}
