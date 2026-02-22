use crate::components::form::form_section::FormSectionBuilder;
use crate::components::input::checkbox_field::CheckboxFieldBuilder;
use crate::components::input::slider_field::labeled_slider;
use crate::components::layout::button_group::ButtonGroupBuilder;
use crate::components::layout::modal_header::ModalHeaderBuilder;
use crate::screen::dashboard::pane::Message;
use crate::style::{self, tokens};

use data::state::pane::{
    ProfileConfig, ProfileDisplayType, ProfileExtendDirection,
    ProfileLengthUnit, ProfileLineStyle, ProfileNodeDetectionMethod,
    ProfilePeriod, VisualConfig,
};

use iced::{
    Alignment, Element, Length,
    widget::{
        column, container, pane_grid, pick_list, radio, row,
        scrollable, space,
    },
};
use iced::widget::scrollable::{Direction, Scrollbar};

use super::common::sync_all_button;

/// Five settings tabs.
const TAB_LABELS: &[&str] =
    &["Data", "Style", "POC", "Value Area", "Peak & Valley"];

pub fn profile_cfg_view<'a>(
    cfg: ProfileConfig,
    pane: pane_grid::Pane,
) -> Element<'a, Message> {
    let active_tab = cfg.settings_tab.min(4) as usize;

    // ── Header ───────────────────────────────────────────────────
    let header =
        ModalHeaderBuilder::new("Profile Settings").on_close(
            Message::PaneEvent(
                pane,
                crate::screen::dashboard::pane::Event::HideModal,
            ),
        );

    // ── Tab bar ──────────────────────────────────────────────────
    let tab_items: Vec<(String, Message)> = TAB_LABELS
        .iter()
        .enumerate()
        .map(|(i, label)| {
            let mut c = cfg.clone();
            c.settings_tab = i as u8;
            (
                label.to_string(),
                Message::VisualConfigChanged(
                    pane,
                    VisualConfig::Profile(c),
                    false,
                ),
            )
        })
        .collect();

    let tab_bar = container(
        ButtonGroupBuilder::new(tab_items, active_tab)
            .tab_style()
            .fill_width()
            .into_element(),
    )
    .padding(iced::Padding {
        top: tokens::spacing::SM,
        right: tokens::spacing::XL,
        bottom: 0.0,
        left: tokens::spacing::XL,
    });

    // ── Tab content ──────────────────────────────────────────────
    let tab_content: Element<'a, Message> = match active_tab {
        0 => data_tab(cfg.clone(), pane),
        1 => style_tab(cfg.clone(), pane),
        2 => poc_tab(cfg.clone(), pane),
        3 => value_area_tab(cfg.clone(), pane),
        4 => peak_valley_tab(cfg.clone(), pane),
        _ => column![].into(),
    };

    // ── Footer ───────────────────────────────────────────────────
    let footer = row![
        space::horizontal(),
        sync_all_button(pane, VisualConfig::Profile(cfg)),
    ]
    .spacing(tokens::spacing::SM)
    .align_y(Alignment::Center);

    // ── Assemble ─────────────────────────────────────────────────
    let body = column![tab_content, footer]
        .spacing(tokens::spacing::LG)
        .width(Length::Fill);

    let body_scrollable =
        scrollable::Scrollable::with_direction(
            body,
            Direction::Vertical(
                Scrollbar::new().width(4).scroller_width(4).spacing(2),
            ),
        )
        .style(style::scroll_bar);

    let inner = column![
        header,
        tab_bar,
        container(body_scrollable).padding(iced::Padding {
            top: tokens::spacing::MD,
            right: tokens::spacing::XL,
            bottom: tokens::spacing::XL,
            left: tokens::spacing::XL,
        }),
    ]
    .width(Length::Fill);

    container(inner)
        .max_width(480.0)
        .max_height(620.0)
        .style(style::dashboard_modal)
        .into()
}

// ── Helpers ─────────────────────────────────────────────────────────

/// Emit a config-changed message for the given pane.
fn cfg_msg(
    pane: pane_grid::Pane,
    cfg: ProfileConfig,
) -> Message {
    Message::VisualConfigChanged(
        pane,
        VisualConfig::Profile(cfg),
        false,
    )
}

// ── Tab 1: Data ─────────────────────────────────────────────────────

fn data_tab<'a>(
    cfg: ProfileConfig,
    pane: pane_grid::Pane,
) -> Element<'a, Message> {
    // Display type
    let display_section = {
        let make_radio =
            |label: &str, dt: ProfileDisplayType| {
                let c = cfg.clone();
                radio(
                    label,
                    dt,
                    Some(cfg.display_type),
                    move |value| {
                        let mut new = c.clone();
                        new.display_type = value;
                        cfg_msg(pane, new)
                    },
                )
                .spacing(tokens::spacing::XS)
            };

        FormSectionBuilder::new("Display Type")
            .push(make_radio(
                "Volume",
                ProfileDisplayType::Volume,
            ))
            .push(make_radio(
                "Bid/Ask Volume",
                ProfileDisplayType::BidAskVolume,
            ))
            .push(make_radio(
                "Delta",
                ProfileDisplayType::Delta,
            ))
            .push(make_radio(
                "Delta & Total",
                ProfileDisplayType::DeltaAndTotal,
            ))
            .push(make_radio(
                "Delta %",
                ProfileDisplayType::DeltaPercentage,
            ))
    };

    // Period
    let period_section = {
        let make_period_radio =
            |label: &str, p: ProfilePeriod| {
                let c = cfg.clone();
                radio(
                    label,
                    p,
                    Some(cfg.period),
                    move |value| {
                        let mut new = c.clone();
                        new.period = value;
                        cfg_msg(pane, new)
                    },
                )
                .spacing(tokens::spacing::XS)
            };

        let mut section = FormSectionBuilder::new("Period")
            .push(make_period_radio(
                "All Data",
                ProfilePeriod::AllData,
            ))
            .push(make_period_radio(
                "Length",
                ProfilePeriod::Length,
            ))
            .push(make_period_radio(
                "Custom Range",
                ProfilePeriod::Custom,
            ));

        if cfg.period == ProfilePeriod::Length {
            let make_unit =
                |label: &str, u: ProfileLengthUnit| {
                    let c = cfg.clone();
                    radio(
                        label,
                        u,
                        Some(cfg.length_unit),
                        move |value| {
                            let mut new = c.clone();
                            new.length_unit = value;
                            cfg_msg(pane, new)
                        },
                    )
                    .spacing(tokens::spacing::XS)
                };

            let units = row![
                make_unit("Days", ProfileLengthUnit::Days),
                make_unit("Min", ProfileLengthUnit::Minutes),
                make_unit(
                    "Contracts",
                    ProfileLengthUnit::Contracts
                ),
            ]
            .spacing(tokens::spacing::MD);

            let c = cfg.clone();
            let length_slider = labeled_slider(
                "Length value",
                1.0..=500.0,
                cfg.length_value as f32,
                move |value| {
                    let mut new = c.clone();
                    new.length_value = value.round() as i64;
                    cfg_msg(pane, new)
                },
                |value| format!("{}", value.round()),
                Some(1.0),
            );

            section = section.push(units).push(length_slider);
        }

        section
    };

    // Tick Grouping
    let grouping_section = {
        let c = cfg.clone();
        let auto_toggle = CheckboxFieldBuilder::new(
            "Automatic grouping",
            cfg.auto_grouping,
            move |value| {
                let mut new = c.clone();
                new.auto_grouping = value;
                cfg_msg(pane, new)
            },
        );

        let mut section = FormSectionBuilder::new("Tick Grouping")
            .push(auto_toggle);

        if cfg.auto_grouping {
            let c = cfg.clone();
            let factor_slider = labeled_slider(
                "Group factor",
                1.0..=50.0,
                cfg.auto_group_factor as f32,
                move |value| {
                    let mut new = c.clone();
                    new.auto_group_factor =
                        value.round() as i64;
                    cfg_msg(pane, new)
                },
                |value| format!("{}x", value.round()),
                Some(1.0),
            );
            section = section.push(factor_slider);
        } else {
            let c = cfg.clone();
            let ticks_slider = labeled_slider(
                "Manual ticks",
                1.0..=100.0,
                cfg.manual_ticks as f32,
                move |value| {
                    let mut new = c.clone();
                    new.manual_ticks = value.round() as i64;
                    cfg_msg(pane, new)
                },
                |value| format!("{} ticks", value.round()),
                Some(1.0),
            );
            section = section.push(ticks_slider);
        }

        section
    };

    // Value Area %
    let va_pct_section = {
        let c = cfg.clone();
        let va_slider = labeled_slider(
            "Value Area %",
            0.5..=0.95,
            cfg.value_area_pct,
            move |value| {
                let mut new = c.clone();
                new.value_area_pct = value;
                cfg_msg(pane, new)
            },
            |value| format!("{:.0}%", value * 100.0),
            Some(0.05),
        );

        FormSectionBuilder::new("Value Area").push(va_slider)
    };

    column![
        display_section,
        period_section,
        grouping_section,
        va_pct_section,
    ]
    .spacing(tokens::spacing::XL)
    .into()
}

// ── Tab 2: Style ────────────────────────────────────────────────────

fn style_tab<'a>(
    cfg: ProfileConfig,
    pane: pane_grid::Pane,
) -> Element<'a, Message> {
    // Opacity
    let c = cfg.clone();
    let opacity_slider = labeled_slider(
        "Opacity",
        0.1..=1.0,
        cfg.opacity,
        move |value| {
            let mut new = c.clone();
            new.opacity = value;
            cfg_msg(pane, new)
        },
        |value| format!("{:.0}%", value * 100.0),
        Some(0.05),
    );

    let section = FormSectionBuilder::new("Appearance")
        .push(opacity_slider);

    // VA highlight toggle (dim outside)
    let c = cfg.clone();
    let va_highlight = CheckboxFieldBuilder::new(
        "Dim outside Value Area",
        cfg.show_va_highlight,
        move |value| {
            let mut new = c.clone();
            new.show_va_highlight = value;
            cfg_msg(pane, new)
        },
    );

    column![section, va_highlight]
        .spacing(tokens::spacing::XL)
        .into()
}

// ── Tab 3: POC ──────────────────────────────────────────────────────

fn poc_tab<'a>(
    cfg: ProfileConfig,
    pane: pane_grid::Pane,
) -> Element<'a, Message> {
    let c = cfg.clone();
    let show_poc = CheckboxFieldBuilder::new(
        "Show POC line",
        cfg.show_poc,
        move |value| {
            let mut new = c.clone();
            new.show_poc = value;
            cfg_msg(pane, new)
        },
    );

    let mut section = FormSectionBuilder::new("Point of Control")
        .push(show_poc);

    if cfg.show_poc {
        let c = cfg.clone();
        let width_slider = labeled_slider(
            "Line width",
            0.5..=4.0,
            cfg.poc_line_width,
            move |value| {
                let mut new = c.clone();
                new.poc_line_width = value;
                cfg_msg(pane, new)
            },
            |value| format!("{:.1}px", value),
            Some(0.5),
        );
        section = section.push(width_slider);

        let c = cfg.clone();
        let line_style = pick_list(
            &ProfileLineStyle::ALL[..],
            Some(cfg.poc_line_style),
            move |value| {
                let mut new = c.clone();
                new.poc_line_style = value;
                cfg_msg(pane, new)
            },
        )
        .width(Length::Fixed(120.0));

        let c2 = cfg.clone();
        let extend = pick_list(
            &ProfileExtendDirection::ALL[..],
            Some(cfg.poc_extend),
            move |value| {
                let mut new = c2.clone();
                new.poc_extend = value;
                cfg_msg(pane, new)
            },
        )
        .width(Length::Fixed(120.0));

        let style_row = row![
            column![
                iced::widget::text("Style").size(tokens::text::LABEL),
                line_style,
            ]
            .spacing(tokens::spacing::XS),
            column![
                iced::widget::text("Extend").size(tokens::text::LABEL),
                extend,
            ]
            .spacing(tokens::spacing::XS),
        ]
        .spacing(tokens::spacing::MD);

        section = section.push(style_row);

        let c = cfg.clone();
        let show_label = CheckboxFieldBuilder::new(
            "Show price label",
            cfg.show_poc_label,
            move |value| {
                let mut new = c.clone();
                new.show_poc_label = value;
                cfg_msg(pane, new)
            },
        );
        section = section.push(show_label);
    }

    section.into_element()
}

// ── Tab 4: Value Area ───────────────────────────────────────────────

fn value_area_tab<'a>(
    cfg: ProfileConfig,
    pane: pane_grid::Pane,
) -> Element<'a, Message> {
    // VA fill
    let c = cfg.clone();
    let show_fill = CheckboxFieldBuilder::new(
        "Show VA fill",
        cfg.show_va_fill,
        move |value| {
            let mut new = c.clone();
            new.show_va_fill = value;
            cfg_msg(pane, new)
        },
    );

    let mut fill_section =
        FormSectionBuilder::new("VA Fill").push(show_fill);

    if cfg.show_va_fill {
        let c = cfg.clone();
        let opacity_slider = labeled_slider(
            "Fill opacity",
            0.01..=0.3,
            cfg.va_fill_opacity,
            move |value| {
                let mut new = c.clone();
                new.va_fill_opacity = value;
                cfg_msg(pane, new)
            },
            |value| format!("{:.0}%", value * 100.0),
            Some(0.01),
        );
        fill_section = fill_section.push(opacity_slider);
    }

    // VAH line
    let vah_section = {
        let c = cfg.clone();
        let width_slider = labeled_slider(
            "VAH line width",
            0.5..=4.0,
            cfg.vah_line_width,
            move |value| {
                let mut new = c.clone();
                new.vah_line_width = value;
                cfg_msg(pane, new)
            },
            |value| format!("{:.1}px", value),
            Some(0.5),
        );

        let c = cfg.clone();
        let line_style = pick_list(
            &ProfileLineStyle::ALL[..],
            Some(cfg.vah_line_style),
            move |value| {
                let mut new = c.clone();
                new.vah_line_style = value;
                cfg_msg(pane, new)
            },
        )
        .width(Length::Fixed(120.0));

        FormSectionBuilder::new("VAH Line")
            .push(width_slider)
            .push(line_style)
    };

    // VAL line
    let val_section = {
        let c = cfg.clone();
        let width_slider = labeled_slider(
            "VAL line width",
            0.5..=4.0,
            cfg.val_line_width,
            move |value| {
                let mut new = c.clone();
                new.val_line_width = value;
                cfg_msg(pane, new)
            },
            |value| format!("{:.1}px", value),
            Some(0.5),
        );

        let c = cfg.clone();
        let line_style = pick_list(
            &ProfileLineStyle::ALL[..],
            Some(cfg.val_line_style),
            move |value| {
                let mut new = c.clone();
                new.val_line_style = value;
                cfg_msg(pane, new)
            },
        )
        .width(Length::Fixed(120.0));

        FormSectionBuilder::new("VAL Line")
            .push(width_slider)
            .push(line_style)
    };

    // Extend + labels
    let c = cfg.clone();
    let extend = pick_list(
        &ProfileExtendDirection::ALL[..],
        Some(cfg.va_extend),
        move |value| {
            let mut new = c.clone();
            new.va_extend = value;
            cfg_msg(pane, new)
        },
    )
    .width(Length::Fixed(120.0));

    let c = cfg.clone();
    let show_labels = CheckboxFieldBuilder::new(
        "Show price labels",
        cfg.show_va_labels,
        move |value| {
            let mut new = c.clone();
            new.show_va_labels = value;
            cfg_msg(pane, new)
        },
    );

    let extend_section = FormSectionBuilder::new("Extension")
        .push(extend)
        .push(show_labels);

    column![fill_section, vah_section, val_section, extend_section]
        .spacing(tokens::spacing::XL)
        .into()
}

// ── Tab 5: Peak & Valley ────────────────────────────────────────────

fn peak_valley_tab<'a>(
    cfg: ProfileConfig,
    pane: pane_grid::Pane,
) -> Element<'a, Message> {
    // ── HVN section ──────────────────────────────────────────────
    let c = cfg.clone();
    let hvn_method = pick_list(
        &ProfileNodeDetectionMethod::ALL[..],
        Some(cfg.hvn_method),
        move |value| {
            let mut new = c.clone();
            new.hvn_method = value;
            cfg_msg(pane, new)
        },
    )
    .width(Length::Fixed(120.0));

    let c = cfg.clone();
    let hvn_threshold = labeled_slider(
        "HVN threshold",
        0.1..=1.0,
        cfg.hvn_threshold,
        move |value| {
            let mut new = c.clone();
            new.hvn_threshold = value;
            cfg_msg(pane, new)
        },
        |value| format!("{:.0}%", value * 100.0),
        Some(0.05),
    );

    let c = cfg.clone();
    let show_hvn_zones = CheckboxFieldBuilder::new(
        "Show HVN zones",
        cfg.show_hvn_zones,
        move |value| {
            let mut new = c.clone();
            new.show_hvn_zones = value;
            cfg_msg(pane, new)
        },
    );

    let mut hvn_section =
        FormSectionBuilder::new("High Volume Nodes")
            .push(hvn_method)
            .push(hvn_threshold)
            .push(show_hvn_zones);

    if cfg.show_hvn_zones {
        let c = cfg.clone();
        let zone_opacity = labeled_slider(
            "Zone opacity",
            0.01..=0.5,
            cfg.hvn_zone_opacity,
            move |value| {
                let mut new = c.clone();
                new.hvn_zone_opacity = value;
                cfg_msg(pane, new)
            },
            |value| format!("{:.0}%", value * 100.0),
            Some(0.01),
        );
        hvn_section = hvn_section.push(zone_opacity);
    }

    // Peak line
    let c = cfg.clone();
    let show_peak = CheckboxFieldBuilder::new(
        "Show peak line",
        cfg.show_peak_line,
        move |value| {
            let mut new = c.clone();
            new.show_peak_line = value;
            cfg_msg(pane, new)
        },
    );

    let mut peak_section =
        FormSectionBuilder::new("Peak Line").push(show_peak);

    if cfg.show_peak_line {
        let c = cfg.clone();
        let peak_width = labeled_slider(
            "Line width",
            0.5..=4.0,
            cfg.peak_line_width,
            move |value| {
                let mut new = c.clone();
                new.peak_line_width = value;
                cfg_msg(pane, new)
            },
            |value| format!("{:.1}px", value),
            Some(0.5),
        );

        let c = cfg.clone();
        let peak_style = pick_list(
            &ProfileLineStyle::ALL[..],
            Some(cfg.peak_line_style),
            move |value| {
                let mut new = c.clone();
                new.peak_line_style = value;
                cfg_msg(pane, new)
            },
        )
        .width(Length::Fixed(120.0));

        let c = cfg.clone();
        let peak_label = CheckboxFieldBuilder::new(
            "Show price label",
            cfg.show_peak_label,
            move |value| {
                let mut new = c.clone();
                new.show_peak_label = value;
                cfg_msg(pane, new)
            },
        );

        peak_section = peak_section
            .push(peak_width)
            .push(peak_style)
            .push(peak_label);
    }

    // ── LVN section ──────────────────────────────────────────────
    let c = cfg.clone();
    let lvn_method = pick_list(
        &ProfileNodeDetectionMethod::ALL[..],
        Some(cfg.lvn_method),
        move |value| {
            let mut new = c.clone();
            new.lvn_method = value;
            cfg_msg(pane, new)
        },
    )
    .width(Length::Fixed(120.0));

    let c = cfg.clone();
    let lvn_threshold = labeled_slider(
        "LVN threshold",
        0.01..=0.5,
        cfg.lvn_threshold,
        move |value| {
            let mut new = c.clone();
            new.lvn_threshold = value;
            cfg_msg(pane, new)
        },
        |value| format!("{:.0}%", value * 100.0),
        Some(0.01),
    );

    let c = cfg.clone();
    let show_lvn_zones = CheckboxFieldBuilder::new(
        "Show LVN zones",
        cfg.show_lvn_zones,
        move |value| {
            let mut new = c.clone();
            new.show_lvn_zones = value;
            cfg_msg(pane, new)
        },
    );

    let mut lvn_section =
        FormSectionBuilder::new("Low Volume Nodes")
            .push(lvn_method)
            .push(lvn_threshold)
            .push(show_lvn_zones);

    if cfg.show_lvn_zones {
        let c = cfg.clone();
        let zone_opacity = labeled_slider(
            "Zone opacity",
            0.01..=0.5,
            cfg.lvn_zone_opacity,
            move |value| {
                let mut new = c.clone();
                new.lvn_zone_opacity = value;
                cfg_msg(pane, new)
            },
            |value| format!("{:.0}%", value * 100.0),
            Some(0.01),
        );
        lvn_section = lvn_section.push(zone_opacity);
    }

    // Valley line
    let c = cfg.clone();
    let show_valley = CheckboxFieldBuilder::new(
        "Show valley line",
        cfg.show_valley_line,
        move |value| {
            let mut new = c.clone();
            new.show_valley_line = value;
            cfg_msg(pane, new)
        },
    );

    let mut valley_section =
        FormSectionBuilder::new("Valley Line").push(show_valley);

    if cfg.show_valley_line {
        let c = cfg.clone();
        let valley_width = labeled_slider(
            "Line width",
            0.5..=4.0,
            cfg.valley_line_width,
            move |value| {
                let mut new = c.clone();
                new.valley_line_width = value;
                cfg_msg(pane, new)
            },
            |value| format!("{:.1}px", value),
            Some(0.5),
        );

        let c = cfg.clone();
        let valley_style = pick_list(
            &ProfileLineStyle::ALL[..],
            Some(cfg.valley_line_style),
            move |value| {
                let mut new = c.clone();
                new.valley_line_style = value;
                cfg_msg(pane, new)
            },
        )
        .width(Length::Fixed(120.0));

        let c = cfg.clone();
        let valley_label = CheckboxFieldBuilder::new(
            "Show price label",
            cfg.show_valley_label,
            move |value| {
                let mut new = c.clone();
                new.show_valley_label = value;
                cfg_msg(pane, new)
            },
        );

        valley_section = valley_section
            .push(valley_width)
            .push(valley_style)
            .push(valley_label);
    }

    column![
        hvn_section,
        peak_section,
        lvn_section,
        valley_section,
    ]
    .spacing(tokens::spacing::XL)
    .into()
}
