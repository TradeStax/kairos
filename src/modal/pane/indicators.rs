use crate::screen::dashboard::pane::{self, Message};
use crate::style::{self, Icon, icon_text};
use crate::widget::{column_drag, dragger_row};

use data::chart::indicator::{Indicator, UiIndicator};
use iced::{
    Element, Length, padding,
    widget::{button, column, container, pane_grid, row, space, text},
};

fn build_indicator_row<'a, I>(
    pane: pane_grid::Pane,
    indicator: &I,
    is_selected: bool,
) -> Element<'a, Message>
where
    I: Indicator + Copy + Into<UiIndicator> + std::fmt::Display + PartialEq,
{
    let content = if is_selected {
        row![
            text(indicator.to_string()),
            space::horizontal(),
            container(icon_text(Icon::Checkmark, 12)),
        ]
        .width(Length::Fill)
    } else {
        row![text(indicator.to_string())].width(Length::Fill)
    };

    button(content)
        .on_press(Message::PaneEvent(
            pane,
            pane::Event::ToggleIndicator((*indicator).into()),
        ))
        .width(Length::Fill)
        .style(move |theme, status| style::button::modifier(theme, status, is_selected))
        .into()
}

fn selected_list<'a, I>(
    pane: pane_grid::Pane,
    selected: &[I],
    reorderable: bool,
) -> Element<'a, Message>
where
    I: Indicator + Copy + Into<UiIndicator> + std::fmt::Display + PartialEq,
{
    let elements: Vec<Element<_>> = selected
        .iter()
        .map(|indicator| {
            let base = build_indicator_row(pane, indicator, true);
            dragger_row(base, reorderable)
        })
        .collect();

    if reorderable {
        let mut draggable_column = column_drag::Column::new()
            .on_drag(move |event| Message::PaneEvent(pane, pane::Event::ReorderIndicator(event)))
            .spacing(4);
        for element in elements {
            draggable_column = draggable_column.push(element);
        }
        draggable_column.into()
    } else {
        iced::widget::Column::with_children(elements)
            .spacing(4)
            .into()
    }
}

fn available_list<'a, I>(pane: pane_grid::Pane, available: &[I]) -> Element<'a, Message>
where
    I: Indicator + Copy + Into<UiIndicator> + std::fmt::Display + PartialEq,
{
    let elements: Vec<Element<_>> = available
        .iter()
        .map(|indicator| {
            let base = build_indicator_row(pane, indicator, false);
            dragger_row(base, false)
        })
        .collect();

    iced::widget::Column::with_children(elements)
        .spacing(4)
        .into()
}

pub fn content_row_kline<'a>(
    pane: pane_grid::Pane,
    selected: &[data::KlineIndicator],
    allows_drag: bool,
) -> Element<'a, Message> {
    let reorderable = allows_drag && selected.len() >= 2;

    let selected_list = if !selected.is_empty() {
        Some(selected_list(pane, selected, reorderable))
    } else {
        None
    };

    // Get all kline indicators
    let available: Vec<data::KlineIndicator> = data::KlineIndicator::all_indicators()
        .into_iter()
        .filter(|indicator| !selected.contains(indicator))
        .collect();

    let available_list = if !available.is_empty() {
        Some(available_list(pane, &available))
    } else {
        None
    };

    let mut col = iced::widget::Column::new();
    if let Some(sel) = selected_list {
        col = col.push(sel);
    }
    if let Some(avail) = available_list {
        col = col.push(avail);
    }

    column![
        container(text("Indicators").size(14)).padding(padding::bottom(8)),
        col.spacing(4)
    ]
    .spacing(4)
    .into()
}

pub fn content_row_heatmap<'a>(
    pane: pane_grid::Pane,
    selected: &[data::HeatmapIndicator],
    allows_drag: bool,
) -> Element<'a, Message> {
    let reorderable = allows_drag && selected.len() >= 2;

    let selected_list = if !selected.is_empty() {
        Some(selected_list(pane, selected, reorderable))
    } else {
        None
    };

    // Get all heatmap indicators
    let available: Vec<data::HeatmapIndicator> = data::HeatmapIndicator::all_indicators()
        .into_iter()
        .filter(|indicator| !selected.contains(indicator))
        .collect();

    let available_list = if !available.is_empty() {
        Some(available_list(pane, &available))
    } else {
        None
    };

    let mut col = iced::widget::Column::new();
    if let Some(sel) = selected_list {
        col = col.push(sel);
    }
    if let Some(avail) = available_list {
        col = col.push(avail);
    }

    column![
        container(text("Indicators").size(14)).padding(padding::bottom(8)),
        col.spacing(4)
    ]
    .spacing(4)
    .into()
}
