//! Profile settings modal — tab dispatcher.

mod data_tab;
mod style_tab;
mod poc_tab;
mod value_area_tab;
mod peak_valley_tab;

use data_tab::data_tab;
use style_tab::style_tab;
use poc_tab::poc_tab;
use value_area_tab::value_area_tab;
use peak_valley_tab::peak_valley_tab;

use crate::components::layout::button_group::ButtonGroupBuilder;
use crate::components::overlay::modal_header::ModalHeaderBuilder;
use crate::screen::dashboard::pane::Message;
use crate::style::{self, tokens};

use data::state::pane::{ProfileConfig, VisualConfig};

use iced::{
    Alignment, Element, Length,
    widget::{column, container, pane_grid, row, scrollable, space},
};
use iced::widget::scrollable::{Direction, Scrollbar};

use super::super::common::sync_all_button;

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
