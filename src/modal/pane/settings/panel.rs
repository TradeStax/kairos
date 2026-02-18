use crate::component::primitives::label::{body, label_text, title};
use crate::screen::dashboard::pane::Message;
use crate::screen::dashboard::panel::timeandsales;
use crate::split_column;
use crate::style;
use crate::style::tokens;
use crate::widget::{classic_slider_row, labeled_slider, tooltip};

use data::panel_config::ladder;
use data::panel_config::timeandsales::{StackedBar, StackedBarRatio};
use data::state::pane_config::{LadderConfig, TimeAndSalesConfig, VisualConfig};
use data::util::format_with_commas;

use iced::{
    Alignment, Element, Length,
    widget::{
        button, checkbox, column, container, pane_grid, pick_list, radio, row, rule, slider, space,
        text, tooltip::Position as TooltipPosition,
    },
};

use super::common::{cfg_view_container, sync_all_button};

pub fn timesales_cfg_view<'a>(
    cfg: timeandsales::Config,
    pane: pane_grid::Pane,
) -> Element<'a, Message> {
    let trade_size_column = {
        let filter = cfg.trade_size_filter;
        let slider = labeled_slider(
            "Trade",
            0.0..=50000.0,
            filter,
            move |value| {
                let stacked_bar = cfg.stacked_bar.map(|sb| {
                    let is_compact =
                        matches!(sb, data::panel::timeandsales::StackedBar::Compact(_));
                    let ratio = sb.ratio();
                    (is_compact, ratio)
                });
                Message::VisualConfigChanged(
                    pane,
                    VisualConfig::TimeAndSales(TimeAndSalesConfig {
                        max_rows: cfg.max_rows,
                        trade_size_filter: value,
                        trade_retention_secs: cfg.trade_retention.as_secs(),
                        show_delta: cfg.show_delta,
                        stacked_bar,
                    }),
                    false,
                )
            },
            |value| format!(">${}", format_with_commas(*value)),
            Some(500.0),
        );

        column![title("Size filter"), slider].spacing(tokens::spacing::MD)
    };

    let retention_minutes = (cfg.trade_retention.as_secs_f32() / 60.0).max(1.0);
    let retention_slider = {
        let slider_ui = slider(1.0..=60.0, retention_minutes, move |new_minutes| {
            let stacked_bar = cfg.stacked_bar.map(|sb| {
                let is_compact = matches!(sb, data::panel::timeandsales::StackedBar::Compact(_));
                let ratio = sb.ratio();
                (is_compact, ratio)
            });
            Message::VisualConfigChanged(
                pane,
                VisualConfig::TimeAndSales(TimeAndSalesConfig {
                    max_rows: cfg.max_rows,
                    trade_size_filter: cfg.trade_size_filter,
                    trade_retention_secs: (new_minutes * 60.0) as u64,
                    show_delta: cfg.show_delta,
                    stacked_bar,
                }),
                false,
            )
        })
        .step(1.0);

        classic_slider_row(
            text("Keep trades for"),
            slider_ui.into(),
            Some(label_text(format!(
                "≈ {} min",
                retention_minutes.round() as u64
            ))),
        )
    };

    let history_column = column![
        row![
            title("History"),
            tooltip(
                button("i").style(style::button::info),
                Some("Affects the stacked bar, colors and how much you can scroll down"),
                TooltipPosition::Top,
            )
        ]
        .spacing(tokens::spacing::XS)
        .align_y(Alignment::Center),
        retention_slider
    ]
    .spacing(tokens::spacing::MD);

    let stacked_bar: Element<_> = {
        let is_shown = cfg.stacked_bar.is_some();

        let enable_checkbox = checkbox(is_shown).label("Show stacked bar").on_toggle({
            move |value| {
                let stacked_bar = if value {
                    // Enable with default settings: Full mode, Volume ratio
                    Some((false, StackedBarRatio::Volume))
                } else {
                    None
                };
                Message::VisualConfigChanged(
                    pane,
                    VisualConfig::TimeAndSales(TimeAndSalesConfig {
                        max_rows: cfg.max_rows,
                        trade_size_filter: cfg.trade_size_filter,
                        trade_retention_secs: cfg.trade_retention.as_secs(),
                        show_delta: cfg.show_delta,
                        stacked_bar,
                    }),
                    false,
                )
            }
        });

        let controls: Option<Element<_>> = cfg.stacked_bar.map(|hist| {
            let ratio = hist.ratio();
            let is_compact = matches!(hist, StackedBar::Compact(_));

            let compact = radio("Compact", true, Some(is_compact), {
                move |v| {
                    let stacked_bar = Some((v, ratio));
                    Message::VisualConfigChanged(
                        pane,
                        VisualConfig::TimeAndSales(TimeAndSalesConfig {
                            max_rows: cfg.max_rows,
                            trade_size_filter: cfg.trade_size_filter,
                            trade_retention_secs: cfg.trade_retention.as_secs(),
                            show_delta: cfg.show_delta,
                            stacked_bar,
                        }),
                        false,
                    )
                }
            })
            .spacing(tokens::spacing::XS);

            let full = radio("Full", false, Some(is_compact), {
                move |v| {
                    let stacked_bar = Some((v, ratio));
                    Message::VisualConfigChanged(
                        pane,
                        VisualConfig::TimeAndSales(TimeAndSalesConfig {
                            max_rows: cfg.max_rows,
                            trade_size_filter: cfg.trade_size_filter,
                            trade_retention_secs: cfg.trade_retention.as_secs(),
                            show_delta: cfg.show_delta,
                            stacked_bar,
                        }),
                        false,
                    )
                }
            })
            .spacing(tokens::spacing::XS);

            let metric_picklist = pick_list(StackedBarRatio::ALL, Some(ratio), move |new_ratio| {
                let stacked_bar = Some((is_compact, new_ratio));
                Message::VisualConfigChanged(
                    pane,
                    VisualConfig::TimeAndSales(TimeAndSalesConfig {
                        max_rows: cfg.max_rows,
                        trade_size_filter: cfg.trade_size_filter,
                        trade_retention_secs: cfg.trade_retention.as_secs(),
                        show_delta: cfg.show_delta,
                        stacked_bar,
                    }),
                    false,
                )
            });

            column![
                rule::horizontal(1),
                body("Mode"),
                row![compact, full].spacing(tokens::spacing::LG),
                body("Metric"),
                metric_picklist,
            ]
            .spacing(tokens::spacing::MD)
            .into()
        });

        let mut inner = column![enable_checkbox]
            .width(Length::Fill)
            .padding(tokens::spacing::XS)
            .spacing(tokens::spacing::MD);

        if let Some(ctrls) = controls {
            inner = inner.push(ctrls);
        }

        container(inner)
            .style(style::modal_container)
            .padding(tokens::spacing::MD)
            .into()
    };

    let content = split_column![
        trade_size_column,
        history_column,
        stacked_bar,
        row![space::horizontal(), sync_all_button(pane, VisualConfig::TimeAndSales(TimeAndSalesConfig {
            max_rows: cfg.max_rows,
            trade_size_filter: cfg.trade_size_filter,
            trade_retention_secs: cfg.trade_retention.as_secs(),
            show_delta: cfg.show_delta,
            stacked_bar: cfg.stacked_bar.map(|sb| {
                let is_compact = matches!(sb, data::panel::timeandsales::StackedBar::Compact(_));
                (is_compact, sb.ratio())
            }),
        }))],
        ; spacing = tokens::spacing::LG, align_x = Alignment::Start
    ];

    cfg_view_container(320, content)
}

pub fn ladder_cfg_view<'a>(cfg: ladder::Config, pane: pane_grid::Pane) -> Element<'a, Message> {
    let display_options = {
        let spread = checkbox(cfg.show_spread)
            .label("Show Spread")
            .on_toggle(move |value| {
                Message::VisualConfigChanged(
                    pane,
                    VisualConfig::Ladder(LadderConfig {
                        levels: cfg.levels,
                        show_spread: value,
                        show_chase_tracker: cfg.show_chase_tracker,
                        trade_retention_secs: cfg.trade_retention.as_secs(),
                    }),
                    false,
                )
            });

        let chase_tracker = checkbox(cfg.show_chase_tracker)
            .label("Show Chase Tracker")
            .on_toggle(move |value| {
                Message::VisualConfigChanged(
                    pane,
                    VisualConfig::Ladder(LadderConfig {
                        levels: cfg.levels,
                        show_spread: cfg.show_spread,
                        show_chase_tracker: value,
                        trade_retention_secs: cfg.trade_retention.as_secs(),
                    }),
                    false,
                )
            });

        column![
            title("Display Options"),
            column![
                spread,
                row![
                    chase_tracker,
                    tooltip(
                        button("i").style(style::button::info),
                        Some("Highlights consecutive best-price moves and fades when momentum stalls.\nCalculated using raw ungrouped data."),
                        TooltipPosition::Top,
                    )
                ]
                .align_y(Alignment::Center)
                .spacing(tokens::spacing::XS)
            ]
            .spacing(tokens::spacing::XS)
        ]
        .spacing(tokens::spacing::MD)
    };

    let retention_slider = {
        let retention_minutes = (cfg.trade_retention.as_secs_f32() / 60.0).max(1.0);

        let slider_ui = slider(1.0..=60.0, retention_minutes, move |new_minutes| {
            Message::VisualConfigChanged(
                pane,
                VisualConfig::Ladder(LadderConfig {
                    levels: cfg.levels,
                    show_spread: cfg.show_spread,
                    show_chase_tracker: cfg.show_chase_tracker,
                    trade_retention_secs: (new_minutes * 60.0) as u64,
                }),
                false,
            )
        })
        .step(1.0);

        classic_slider_row(
            text("Keep trades for"),
            slider_ui.into(),
            Some(label_text(format!(
                "≈ {} min",
                retention_minutes.round() as u64
            ))),
        )
    };

    let history_column =
        column![title("History"), retention_slider].spacing(tokens::spacing::MD);

    let content = split_column![
        display_options,
        history_column,
        row![
            space::horizontal(),
            sync_all_button(pane, VisualConfig::Ladder(LadderConfig {
                levels: cfg.levels,
                show_spread: cfg.show_spread,
                show_chase_tracker: cfg.show_chase_tracker,
                trade_retention_secs: cfg.trade_retention.as_secs(),
            }))
        ],
        ; spacing = tokens::spacing::LG, align_x = Alignment::Start
    ];

    cfg_view_container(320, content)
}
