//! Profile settings — Value Area tab.

use crate::components::form::form_section::FormSectionBuilder;
use crate::components::input::checkbox_field::CheckboxFieldBuilder;
use crate::components::input::slider_field::labeled_slider;
use crate::screen::dashboard::pane::Message;
use crate::style::tokens;

use data::state::pane::{
    ProfileConfig, ProfileExtendDirection, ProfileLineStyle,
    VisualConfig,
};

use iced::{
    Element, Length,
    widget::{column, pane_grid, pick_list},
};

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

pub(super) fn value_area_tab<'a>(
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
