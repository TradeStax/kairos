//! Profile settings — Style tab.

use crate::components::form::form_section::FormSectionBuilder;
use crate::components::input::checkbox_field::CheckboxFieldBuilder;
use crate::components::input::slider_field::labeled_slider;
use crate::screen::dashboard::pane::Message;
use crate::style::tokens;

use data::state::pane::{ProfileConfig, VisualConfig};

use iced::{Element, widget::{column, pane_grid}};

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

pub(super) fn style_tab<'a>(
    cfg: ProfileConfig,
    pane: pane_grid::Pane,
) -> Element<'a, Message> {
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
