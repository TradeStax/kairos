use crate::components::input::slider_field::labeled_slider;
use crate::components::primitives::label::title;
use crate::screen::dashboard::pane::Message;
use crate::split_column;
use crate::style::tokens;

use data::state::pane::{
    ProfileConfig, ProfileDisplayType, ProfileLengthUnit, ProfilePeriod, VisualConfig,
};

use iced::{
    Alignment, Element,
    widget::{checkbox, column, pane_grid, radio, row, space},
};

use super::common::{cfg_view_container, sync_all_button};

pub fn profile_cfg_view<'a>(
    cfg: ProfileConfig,
    pane: pane_grid::Pane,
) -> Element<'a, Message> {
    // ── Display type ──────────────────────────────────────────────
    let display_section = {
        let make_radio = |label: &str, dt: ProfileDisplayType| {
            let c = cfg.clone();
            radio(label, dt, Some(cfg.display_type), move |value| {
                let mut new = c.clone();
                new.display_type = value;
                Message::VisualConfigChanged(
                    pane,
                    VisualConfig::Profile(new),
                    false,
                )
            })
            .spacing(tokens::spacing::XS)
        };

        column![
            title("Display"),
            make_radio("Volume", ProfileDisplayType::Volume),
            make_radio("Bid/Ask Volume", ProfileDisplayType::BidAskVolume),
            make_radio("Delta", ProfileDisplayType::Delta),
            make_radio("Delta & Total", ProfileDisplayType::DeltaAndTotal),
            make_radio("Delta %", ProfileDisplayType::DeltaPercentage),
        ]
        .spacing(tokens::spacing::SM)
    };

    // ── Period ─────────────────────────────────────────────────────
    let period_section = {
        let make_period_radio = |label: &str, p: ProfilePeriod| {
            let c = cfg.clone();
            radio(label, p, Some(cfg.period), move |value| {
                let mut new = c.clone();
                new.period = value;
                Message::VisualConfigChanged(
                    pane,
                    VisualConfig::Profile(new),
                    false,
                )
            })
            .spacing(tokens::spacing::XS)
        };

        let mut col = column![
            title("Period"),
            make_period_radio("All Data", ProfilePeriod::AllData),
            make_period_radio("Length", ProfilePeriod::Length),
            make_period_radio("Custom Range", ProfilePeriod::Custom),
        ]
        .spacing(tokens::spacing::SM);

        if cfg.period == ProfilePeriod::Length {
            let make_unit = |label: &str, u: ProfileLengthUnit| {
                let c = cfg.clone();
                radio(label, u, Some(cfg.length_unit), move |value| {
                    let mut new = c.clone();
                    new.length_unit = value;
                    Message::VisualConfigChanged(
                        pane,
                        VisualConfig::Profile(new),
                        false,
                    )
                })
                .spacing(tokens::spacing::XS)
            };

            let units = row![
                make_unit("Days", ProfileLengthUnit::Days),
                make_unit("Min", ProfileLengthUnit::Minutes),
                make_unit("Contracts", ProfileLengthUnit::Contracts),
            ]
            .spacing(tokens::spacing::MD);

            let length_value = cfg.length_value;
            let c = cfg.clone();
            let length_slider = labeled_slider(
                "Length value",
                1.0..=500.0,
                length_value as f32,
                move |value| {
                    let mut new = c.clone();
                    new.length_value = value.round() as i64;
                    Message::VisualConfigChanged(
                        pane,
                        VisualConfig::Profile(new),
                        false,
                    )
                },
                |value| format!("{}", value.round()),
                Some(1.0),
            );

            col = col.push(units).push(length_slider);
        }

        col
    };

    // ── Tick Grouping ──────────────────────────────────────────────
    let grouping_section = {
        let auto = cfg.auto_grouping;
        let c = cfg.clone();
        let auto_checkbox = checkbox(auto)
            .label("Automatic grouping")
            .on_toggle(move |value| {
                let mut new = c.clone();
                new.auto_grouping = value;
                Message::VisualConfigChanged(
                    pane,
                    VisualConfig::Profile(new),
                    false,
                )
            });

        let mut col = column![title("Tick Grouping"), auto_checkbox]
            .spacing(tokens::spacing::SM);

        if auto {
            let factor = cfg.auto_group_factor;
            let c = cfg.clone();
            let factor_slider = labeled_slider(
                "Group factor",
                1.0..=50.0,
                factor as f32,
                move |value| {
                    let mut new = c.clone();
                    new.auto_group_factor = value.round() as i64;
                    Message::VisualConfigChanged(
                        pane,
                        VisualConfig::Profile(new),
                        false,
                    )
                },
                |value| format!("{}x", value.round()),
                Some(1.0),
            );
            col = col.push(factor_slider);
        } else {
            let ticks = cfg.manual_ticks;
            let c = cfg.clone();
            let ticks_slider = labeled_slider(
                "Manual ticks",
                1.0..=100.0,
                ticks as f32,
                move |value| {
                    let mut new = c.clone();
                    new.manual_ticks = value.round() as i64;
                    Message::VisualConfigChanged(
                        pane,
                        VisualConfig::Profile(new),
                        false,
                    )
                },
                |value| format!("{} ticks", value.round()),
                Some(1.0),
            );
            col = col.push(ticks_slider);
        }

        col
    };

    // ── Value Area ─────────────────────────────────────────────────
    let va_section = {
        let c = cfg.clone();
        let va_slider = labeled_slider(
            "Value Area %",
            0.5..=0.95,
            cfg.value_area_pct,
            move |value| {
                let mut new = c.clone();
                new.value_area_pct = value;
                Message::VisualConfigChanged(
                    pane,
                    VisualConfig::Profile(new),
                    false,
                )
            },
            |value| format!("{:.0}%", value * 100.0),
            Some(0.05),
        );

        let c = cfg.clone();
        let va_highlight = checkbox(cfg.show_va_highlight)
            .label("Dim outside VA")
            .on_toggle(move |value| {
                let mut new = c.clone();
                new.show_va_highlight = value;
                Message::VisualConfigChanged(
                    pane,
                    VisualConfig::Profile(new),
                    false,
                )
            });

        column![title("Value Area"), va_slider, va_highlight]
            .spacing(tokens::spacing::SM)
    };

    // ── POC ────────────────────────────────────────────────────────
    let poc_section = {
        let c = cfg.clone();
        let poc_toggle = checkbox(cfg.show_poc)
            .label("Show POC line")
            .on_toggle(move |value| {
                let mut new = c.clone();
                new.show_poc = value;
                Message::VisualConfigChanged(
                    pane,
                    VisualConfig::Profile(new),
                    false,
                )
            });

        let c = cfg.clone();
        let poc_width_slider = labeled_slider(
            "POC line width",
            0.5..=4.0,
            cfg.poc_line_width,
            move |value| {
                let mut new = c.clone();
                new.poc_line_width = value;
                Message::VisualConfigChanged(
                    pane,
                    VisualConfig::Profile(new),
                    false,
                )
            },
            |value| format!("{:.1}px", value),
            Some(0.5),
        );

        column![title("Point of Control"), poc_toggle, poc_width_slider]
            .spacing(tokens::spacing::SM)
    };

    // ── Volume Nodes ───────────────────────────────────────────────
    let nodes_section = {
        let c = cfg.clone();
        let hvn_toggle = checkbox(cfg.show_hvn)
            .label("Show HVN")
            .on_toggle(move |value| {
                let mut new = c.clone();
                new.show_hvn = value;
                Message::VisualConfigChanged(
                    pane,
                    VisualConfig::Profile(new),
                    false,
                )
            });

        let c = cfg.clone();
        let lvn_toggle = checkbox(cfg.show_lvn)
            .label("Show LVN")
            .on_toggle(move |value| {
                let mut new = c.clone();
                new.show_lvn = value;
                Message::VisualConfigChanged(
                    pane,
                    VisualConfig::Profile(new),
                    false,
                )
            });

        let mut col = column![title("Volume Nodes"), hvn_toggle, lvn_toggle]
            .spacing(tokens::spacing::SM);

        if cfg.show_hvn {
            let c = cfg.clone();
            let hvn_slider = labeled_slider(
                "HVN threshold",
                0.1..=1.0,
                cfg.hvn_threshold,
                move |value| {
                    let mut new = c.clone();
                    new.hvn_threshold = value;
                    Message::VisualConfigChanged(
                        pane,
                        VisualConfig::Profile(new),
                        false,
                    )
                },
                |value| format!("{:.0}%", value * 100.0),
                Some(0.05),
            );
            col = col.push(hvn_slider);
        }

        if cfg.show_lvn {
            let c = cfg.clone();
            let lvn_slider = labeled_slider(
                "LVN threshold",
                0.01..=0.5,
                cfg.lvn_threshold,
                move |value| {
                    let mut new = c.clone();
                    new.lvn_threshold = value;
                    Message::VisualConfigChanged(
                        pane,
                        VisualConfig::Profile(new),
                        false,
                    )
                },
                |value| format!("{:.0}%", value * 100.0),
                Some(0.01),
            );
            col = col.push(lvn_slider);
        }

        col
    };

    // ── Opacity ────────────────────────────────────────────────────
    let opacity_section = {
        let c = cfg.clone();
        let opacity_slider = labeled_slider(
            "Opacity",
            0.1..=1.0,
            cfg.opacity,
            move |value| {
                let mut new = c.clone();
                new.opacity = value;
                Message::VisualConfigChanged(
                    pane,
                    VisualConfig::Profile(new),
                    false,
                )
            },
            |value| format!("{:.0}%", value * 100.0),
            Some(0.05),
        );

        column![title("Appearance"), opacity_slider]
            .spacing(tokens::spacing::SM)
    };

    // ── Buttons ────────────────────────────────────────────────────
    let buttons_row = row![
        space::horizontal(),
        sync_all_button(pane, VisualConfig::Profile(cfg)),
    ]
    .spacing(tokens::spacing::SM)
    .align_y(Alignment::Center);

    let content = split_column![
        display_section,
        period_section,
        grouping_section,
        va_section,
        poc_section,
        nodes_section,
        opacity_section,
        buttons_row
        ; spacing = tokens::spacing::LG, align_x = Alignment::Start
    ];

    cfg_view_container(340, content)
}
