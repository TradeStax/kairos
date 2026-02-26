//! Profile settings — Peak & Valley tab.

use crate::components::form::form_section::FormSectionBuilder;
use crate::components::input::checkbox_field::CheckboxFieldBuilder;
use crate::components::input::slider_field::labeled_slider;
use crate::screen::dashboard::pane::Message;
use crate::style::tokens;

use crate::screen::dashboard::pane::config::{
    ProfileConfig, ProfileLineStyle, ProfileNodeDetectionMethod, VisualConfig,
};

use iced::{
    Element, Length,
    widget::{column, pane_grid, pick_list},
};

fn cfg_msg(pane: pane_grid::Pane, cfg: ProfileConfig) -> Message {
    Message::VisualConfigChanged(pane, VisualConfig::Profile(Box::new(cfg)), false)
}

pub(super) fn peak_valley_tab<'a>(
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
    let show_hvn_zones =
        CheckboxFieldBuilder::new("Show HVN zones", cfg.show_hvn_zones, move |value| {
            let mut new = c.clone();
            new.show_hvn_zones = value;
            cfg_msg(pane, new)
        });

    let mut hvn_section = FormSectionBuilder::new("High Volume Nodes")
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
    let show_peak = CheckboxFieldBuilder::new("Show peak line", cfg.show_peak_line, move |value| {
        let mut new = c.clone();
        new.show_peak_line = value;
        cfg_msg(pane, new)
    });

    let mut peak_section = FormSectionBuilder::new("Peak Line").push(show_peak);

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
        let peak_label =
            CheckboxFieldBuilder::new("Show price label", cfg.show_peak_label, move |value| {
                let mut new = c.clone();
                new.show_peak_label = value;
                cfg_msg(pane, new)
            });

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
    let show_lvn_zones =
        CheckboxFieldBuilder::new("Show LVN zones", cfg.show_lvn_zones, move |value| {
            let mut new = c.clone();
            new.show_lvn_zones = value;
            cfg_msg(pane, new)
        });

    let mut lvn_section = FormSectionBuilder::new("Low Volume Nodes")
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
    let show_valley =
        CheckboxFieldBuilder::new("Show valley line", cfg.show_valley_line, move |value| {
            let mut new = c.clone();
            new.show_valley_line = value;
            cfg_msg(pane, new)
        });

    let mut valley_section = FormSectionBuilder::new("Valley Line").push(show_valley);

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
        let valley_label =
            CheckboxFieldBuilder::new("Show price label", cfg.show_valley_label, move |value| {
                let mut new = c.clone();
                new.show_valley_label = value;
                cfg_msg(pane, new)
            });

        valley_section = valley_section
            .push(valley_width)
            .push(valley_style)
            .push(valley_label);
    }

    column![hvn_section, peak_section, lvn_section, valley_section,]
        .spacing(tokens::spacing::XL)
        .into()
}
