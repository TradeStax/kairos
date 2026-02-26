//! Profile settings — Data tab.

use crate::components::form::form_section::FormSectionBuilder;
use crate::components::input::checkbox_field::CheckboxFieldBuilder;
use crate::components::input::slider_field::labeled_slider;
use crate::screen::dashboard::pane::Message;
use crate::style::tokens;

use crate::screen::dashboard::pane::config::{ProfileConfig, ProfileDisplayType, ProfileSplitUnit};

use iced::{
    Alignment, Element, Length,
    widget::{column, pane_grid, pick_list, radio, row},
};

fn cfg_msg(pane: pane_grid::Pane, cfg: ProfileConfig) -> Message {
    use crate::screen::dashboard::pane::config::VisualConfig;
    Message::VisualConfigChanged(pane, VisualConfig::Profile(Box::new(cfg)), false)
}

pub(super) fn data_tab<'a>(cfg: ProfileConfig, pane: pane_grid::Pane) -> Element<'a, Message> {
    // Display type
    let display_section = {
        let make_radio = |label: &str, dt: ProfileDisplayType| {
            let c = cfg.clone();
            radio(label, dt, Some(cfg.display_type), move |value| {
                let mut new = c.clone();
                new.display_type = value;
                cfg_msg(pane, new)
            })
            .spacing(tokens::spacing::XS)
        };

        FormSectionBuilder::new("Display Type")
            .push(make_radio("Volume", ProfileDisplayType::Volume))
            .push(make_radio(
                "Bid/Ask Volume",
                ProfileDisplayType::BidAskVolume,
            ))
            .push(make_radio("Delta", ProfileDisplayType::Delta))
            .push(make_radio(
                "Delta & Total",
                ProfileDisplayType::DeltaAndTotal,
            ))
            .push(make_radio("Delta %", ProfileDisplayType::DeltaPercentage))
    };

    // Split interval
    let split_section = {
        let c = cfg.clone();
        let unit_picker = pick_list(ProfileSplitUnit::ALL, Some(cfg.split_unit), move |value| {
            let mut new = c.clone();
            new.split_unit = value;
            cfg_msg(pane, new)
        })
        .width(Length::Fixed(120.0));

        let c2 = cfg.clone();
        let value_slider = labeled_slider(
            "Split value",
            1.0..=100.0,
            cfg.split_value as f32,
            move |value| {
                let mut new = c2.clone();
                new.split_value = value.round() as i64;
                cfg_msg(pane, new)
            },
            |value| format!("{}", value.round()),
            Some(1.0),
        );

        FormSectionBuilder::new("Split Interval").push(
            row![unit_picker, value_slider]
                .spacing(tokens::spacing::MD)
                .align_y(Alignment::End),
        )
    };

    // Max profiles
    let max_profiles_section = {
        let c = cfg.clone();
        let slider = labeled_slider(
            "Max profiles",
            1.0..=50.0,
            cfg.max_profiles as f32,
            move |value| {
                let mut new = c.clone();
                new.max_profiles = value.round() as i64;
                cfg_msg(pane, new)
            },
            |value| format!("{}", value.round()),
            Some(1.0),
        );

        FormSectionBuilder::new("Max Profiles").push(slider)
    };

    // Tick Grouping
    let grouping_section = {
        let c = cfg.clone();
        let auto_toggle =
            CheckboxFieldBuilder::new("Automatic grouping", cfg.auto_grouping, move |value| {
                let mut new = c.clone();
                new.auto_grouping = value;
                cfg_msg(pane, new)
            });

        let mut section = FormSectionBuilder::new("Tick Grouping").push(auto_toggle);

        if cfg.auto_grouping {
            let c = cfg.clone();
            let factor_slider = labeled_slider(
                "Group factor",
                1.0..=50.0,
                cfg.auto_group_factor as f32,
                move |value| {
                    let mut new = c.clone();
                    new.auto_group_factor = value.round() as i64;
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
        split_section,
        max_profiles_section,
        grouping_section,
        va_pct_section,
    ]
    .spacing(tokens::spacing::XL)
    .into()
}
