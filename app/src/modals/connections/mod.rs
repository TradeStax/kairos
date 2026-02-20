//! Connections Quick-Menu
//!
//! Lightweight sidebar popover showing up to 5 connections (live first,
//! then recent historical). Each row shows status, name, provider, and
//! a connect/disconnect button. "Manage Connections" opens the full dialog.

use crate::components;
use crate::components::primitives::label::{body, small};
use crate::style;
use crate::style::{palette, tokens};
use data::feed::{DataFeed, DataFeedManager, FeedId, FeedStatus};
use iced::{
    Alignment, Element, Length,
    widget::{button, column, container, row, rule, text},
};

const MAX_VISIBLE_FEEDS: usize = 5;

#[derive(Debug, Clone)]
pub enum ConnectionsMenuMessage {
    ConnectFeed(FeedId),
    DisconnectFeed(FeedId),
    OpenManageDialog,
}

pub enum Action {
    ConnectFeed(FeedId),
    DisconnectFeed(FeedId),
    OpenManageDialog,
}

/// Snapshot-based view of connections for the sidebar popover
pub struct ConnectionsMenu {
    feeds_snapshot: DataFeedManager,
}

impl ConnectionsMenu {
    pub fn new() -> Self {
        Self {
            feeds_snapshot: DataFeedManager::default(),
        }
    }

    pub fn sync_snapshot(&mut self, manager: &DataFeedManager) {
        self.feeds_snapshot = manager.clone();
    }

    pub fn update(&mut self, message: ConnectionsMenuMessage) -> Option<Action> {
        match message {
            ConnectionsMenuMessage::ConnectFeed(id) => Some(Action::ConnectFeed(id)),
            ConnectionsMenuMessage::DisconnectFeed(id) => Some(Action::DisconnectFeed(id)),
            ConnectionsMenuMessage::OpenManageDialog => Some(Action::OpenManageDialog),
        }
    }

    pub fn view(&self) -> Element<'_, ConnectionsMenuMessage> {
        let feeds = &self.feeds_snapshot;

        let header: Element<'_, ConnectionsMenuMessage> =
            components::layout::section_header::SectionHeaderBuilder::new("Connections")
                .trailing(small(format!(
                    "{}/{}",
                    feeds.active_count(),
                    feeds.total_count()
                )))
                .into();

        // Sort: connected feeds first, then by priority
        let mut display_feeds: Vec<&DataFeed> = feeds.feeds().iter().collect();
        display_feeds.sort_by(|a, b| {
            let a_connected = a.status.is_connected() as u8;
            let b_connected = b.status.is_connected() as u8;
            b_connected
                .cmp(&a_connected)
                .then(a.priority.cmp(&b.priority))
        });
        display_feeds.truncate(MAX_VISIBLE_FEEDS);

        let mut feed_list = column![].spacing(tokens::spacing::XS);

        if display_feeds.is_empty() {
            feed_list = feed_list.push(body("No feeds configured"));
        } else {
            for feed in &display_feeds {
                feed_list = feed_list.push(self.view_connection_row(feed));
            }
        }

        let manage_button = button(
            text("Manage Connections")
                .size(tokens::text::BODY)
                .align_x(Alignment::Center),
        )
        .width(Length::Fill)
        .on_press(ConnectionsMenuMessage::OpenManageDialog)
        .padding([tokens::spacing::XS, tokens::spacing::MD]);

        let content = column![
            header,
            rule::horizontal(1).style(style::split_ruler),
            feed_list,
            rule::horizontal(1).style(style::split_ruler),
            manage_button,
        ]
        .spacing(tokens::spacing::MD);

        container(content)
            .max_width(tokens::layout::MODAL_WIDTH_SM)
            .padding(tokens::spacing::XL)
            .style(style::dashboard_modal)
            .into()
    }

    fn view_connection_row<'a>(&self, feed: &'a DataFeed) -> Element<'a, ConnectionsMenuMessage> {
        let status_color = palette::status_color(&feed.status);

        let status_dot = components::display::status_dot(status_color);

        let feed_id = feed.id;
        let is_connected = feed.status.is_connected();
        let is_connecting = matches!(feed.status, FeedStatus::Connecting);

        let provider_label = text(feed.provider.display_name())
            .size(tokens::text::TINY)
            .style(|theme: &iced::Theme| iced::widget::text::Style {
                color: Some(theme.extended_palette().secondary.weak.color),
            });

        let name_label = container(
            text(&feed.name)
                .size(tokens::text::BODY)
                .wrapping(iced::widget::text::Wrapping::None),
        )
        .width(Length::Fill)
        .clip(true);

        let row_content = row![status_dot, name_label, provider_label,]
            .spacing(tokens::spacing::MD)
            .align_y(Alignment::Center);

        let msg = if is_connected {
            Some(ConnectionsMenuMessage::DisconnectFeed(feed_id))
        } else if is_connecting {
            None // disable while connecting
        } else {
            Some(ConnectionsMenuMessage::ConnectFeed(feed_id))
        };

        let btn = button(row_content)
            .width(Length::Fill)
            .padding([6, 10])
            .style(style::button::list_item);

        if let Some(msg) = msg {
            btn.on_press(msg).into()
        } else {
            btn.into()
        }
    }
}
