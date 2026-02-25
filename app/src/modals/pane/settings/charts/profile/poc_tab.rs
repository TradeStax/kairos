//! Profile settings — POC tab.

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
    widget::{column, pane_grid, pick_list, row},
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

pub(super) fn poc_tab<'a>(
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
