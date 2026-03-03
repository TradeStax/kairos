use crate::components::display::tooltip::tooltip;
use crate::components::input::slider_field::classic_slider_row;
use crate::components::primitives::label::{label_text, title};
use crate::screen::dashboard::pane::Message;
use crate::split_column;
use crate::style;
use crate::style::tokens;

use crate::screen::dashboard::ladder;
use crate::screen::dashboard::pane::config::{LadderConfig, VisualConfig};

use iced::{
    Alignment, Element,
    widget::{
        button, checkbox, column, pane_grid, row, slider, space, text,
        tooltip::Position as TooltipPosition,
    },
};

use super::common::{cfg_view_container, sync_all_button};

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

    let history_column = column![title("History"), retention_slider].spacing(tokens::spacing::MD);

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
