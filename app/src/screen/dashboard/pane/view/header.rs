//! Stream-info header construction for the pane title bar.

use crate::config::UserTimezone;
use crate::{
    components::input::link_group_button::link_group_button,
    components::primitives::{
        Icon,
        badge::{BadgeKind, badge},
        exchange_icon, icon_text,
        label::*,
        separator::vertical_divider,
    },
    modals::{self, pane::Modal},
    style::{self, tokens},
};
use data::{FuturesTicker, FuturesTickerInfo};
use iced::widget::pane_grid;
use iced::{
    Alignment, Length,
    alignment::Vertical,
    padding,
    widget::{button, row},
};
use rustc_hash::FxHashMap;

use super::super::{Content, Event, Message, State};
use super::assistant;

/// Build the stream-info header row for the pane title bar.
///
/// Returns the assembled row and whether this is an AI pane
/// (so the caller knows to skip the live/disconnected badge).
pub(super) fn build_stream_info_row<'a>(
    state: &'a State,
    id: pane_grid::Pane,
    _tickers_info: &'a FxHashMap<FuturesTicker, FuturesTickerInfo>,
    _ticker_ranges: &'a std::collections::HashMap<String, String>,
    _timezone: UserTimezone,
) -> (iced::widget::Row<'a, Message>, bool) {
    let mut stream_info_element = if matches!(state.content, Content::Starter) {
        row![]
    } else {
        row![link_group_button(
            state.link_group.as_ref().map(|g| g.to_string()),
            state.link_group.is_some(),
            Message::PaneEvent(id, Box::new(Event::ShowModal(Modal::LinkGroup))),
        )]
    };

    // AI assistant pane — icon + label + model badge
    if let Content::AiAssistant(ai_state) = &state.content {
        let icon = iced::widget::container(icon_text(Icon::MessageSquare, 13))
            .width(Length::Fixed(13.0))
            .height(Length::Fixed(13.0))
            .align_x(Alignment::Center)
            .align_y(Alignment::Center);

        let label_group = iced::widget::container(
            row![icon, label_text("AI Analyst")]
                .spacing(tokens::spacing::XS)
                .align_y(Vertical::Center),
        )
        .align_y(Alignment::Center)
        .height(Length::Fill)
        .padding(padding::left(tokens::spacing::SM));

        stream_info_element = stream_info_element
            .push(label_group)
            .push(vertical_divider())
            .push(badge(
                assistant::model_short_name(&ai_state.model),
                BadgeKind::Default,
            ));
    }

    let is_ai_pane = matches!(state.content, Content::AiAssistant(_));

    if let Some(ticker_info) = state.ticker_info {
        if is_ai_pane {
            // AI assistant: plain ticker label, no dropdown
            let symbol = ticker_info.ticker.as_str().to_string();
            stream_info_element = stream_info_element.push(
                iced::widget::container(title(symbol))
                    .align_y(Alignment::Center)
                    .height(Length::Fill)
                    .padding(padding::left(tokens::spacing::XS)),
            );
        } else {
            let exchange_icon = icon_text(exchange_icon(ticker_info.ticker.venue), 14);
            let symbol = ticker_info.ticker.as_str().to_string();

            let content = row![exchange_icon, title(symbol)]
                .align_y(Vertical::Center)
                .spacing(tokens::spacing::XS);

            let tickers_list_btn = button(content)
                .on_press(Message::PaneEvent(
                    id,
                    Box::new(Event::ShowModal(Modal::MiniTickersList(
                        modals::pane::tickers::MiniPanel::new(),
                    ))),
                ))
                .style(|theme, status| {
                    style::button::modifier(
                        theme,
                        status,
                        !matches!(state.modal, Some(Modal::MiniTickersList(_))),
                    )
                })
                .padding([4, 10]);

            stream_info_element = stream_info_element.push(tickers_list_btn);
        }
    } else if !matches!(state.content, Content::Starter | Content::AiAssistant(_)) {
        let content = row![label_text("Choose a ticker")]
            .align_y(Alignment::Center)
            .spacing(tokens::spacing::XS);

        let tickers_list_btn = button(content)
            .on_press(Message::PaneEvent(
                id,
                Box::new(Event::ShowModal(Modal::MiniTickersList(
                    modals::pane::tickers::MiniPanel::new(),
                ))),
            ))
            .style(|theme, status| {
                style::button::modifier(
                    theme,
                    status,
                    !matches!(state.modal, Some(Modal::MiniTickersList(_))),
                )
            })
            .padding([4, 10]);

        stream_info_element = stream_info_element.push(tickers_list_btn);
    }

    (stream_info_element, is_ai_pane)
}

/// Append the connection-status badge to the stream-info row.
pub(super) fn append_loading_badge<'a>(
    mut row: iced::widget::Row<'a, Message>,
    state: &'a State,
    is_ai_pane: bool,
) -> iced::widget::Row<'a, Message> {
    if !is_ai_pane
        && matches!(
            state.loading_status,
            data::LoadingStatus::Ready | data::LoadingStatus::Idle
        )
    {
        if state.feed_id.is_some()
            && state.ticker_info.is_some()
            && state.content.initialized()
        {
            row = row.push(crate::components::display::status_dot::status_badge_themed(
                |theme| {
                    let p = theme.extended_palette();
                    p.success.base.color
                },
                "LIVE",
            ));
        } else if state.feed_id.is_none()
            && state.ticker_info.is_some()
            && state.content.initialized()
        {
            row = row.push(crate::components::display::status_dot::status_badge_themed(
                |theme| {
                    let p = theme.extended_palette();
                    p.danger.base.color
                },
                "Disconnected",
            ));
        }
    }
    row
}
