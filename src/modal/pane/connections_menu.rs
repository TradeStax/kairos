//! Connections Quick-Menu
//!
//! Lightweight sidebar popover showing up to 5 connections (live first,
//! then recent historical). Each row shows status, name, provider, and
//! a connect/disconnect button. "Manage Connections" opens the full dialog.

use crate::style;
use data::feed::{DataFeed, DataFeedManager, FeedId, FeedStatus};
use iced::{
    Alignment, Element, Length,
    widget::{button, column, container, row, rule, space, text},
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

    pub fn update(
        &mut self,
        message: ConnectionsMenuMessage,
    ) -> Option<Action> {
        match message {
            ConnectionsMenuMessage::ConnectFeed(id) => {
                Some(Action::ConnectFeed(id))
            }
            ConnectionsMenuMessage::DisconnectFeed(id) => {
                Some(Action::DisconnectFeed(id))
            }
            ConnectionsMenuMessage::OpenManageDialog => {
                Some(Action::OpenManageDialog)
            }
        }
    }

    pub fn view(&self) -> Element<'_, ConnectionsMenuMessage> {
        let feeds = &self.feeds_snapshot;

        let header = row![
            text("Connections").size(14),
            space::horizontal().width(Length::Fill),
            text(format!("{}/{}", feeds.active_count(), feeds.total_count()))
                .size(11),
        ]
        .align_y(Alignment::Center);

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

        let mut feed_list = column![].spacing(4);

        if display_feeds.is_empty() {
            feed_list = feed_list.push(
                text("No feeds configured").size(12),
            );
        } else {
            for feed in &display_feeds {
                feed_list = feed_list.push(self.view_connection_row(feed));
            }
        }

        let manage_button = button(
            text("Manage Connections")
                .size(12)
                .align_x(Alignment::Center),
        )
        .width(Length::Fill)
        .on_press(ConnectionsMenuMessage::OpenManageDialog)
        .padding([4, 8]);

        let content = column![
            header,
            rule::horizontal(1).style(style::split_ruler),
            feed_list,
            rule::horizontal(1).style(style::split_ruler),
            manage_button,
        ]
        .spacing(8);

        container(content)
            .max_width(220)
            .padding(16)
            .style(style::dashboard_modal)
            .into()
    }

    fn view_connection_row<'a>(
        &self,
        feed: &'a DataFeed,
    ) -> Element<'a, ConnectionsMenuMessage> {
        let status_color = match &feed.status {
            FeedStatus::Connected => iced::Color::from_rgb(0.2, 0.8, 0.2),
            FeedStatus::Connecting => iced::Color::from_rgb(0.9, 0.7, 0.1),
            FeedStatus::Downloading { .. } => {
                iced::Color::from_rgb(0.3, 0.6, 1.0)
            }
            FeedStatus::Error(_) => iced::Color::from_rgb(0.9, 0.2, 0.2),
            FeedStatus::Disconnected => {
                iced::Color::from_rgb(0.5, 0.5, 0.5)
            }
        };

        let status_dot =
            container(space::horizontal().width(8).height(8)).style(
                move |_theme: &iced::Theme| container::Style {
                    background: Some(iced::Background::Color(status_color)),
                    border: iced::Border {
                        radius: 4.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            );

        let feed_id = feed.id;
        let is_connected = feed.status.is_connected();
        let is_connecting = matches!(feed.status, FeedStatus::Connecting);

        let provider_label = text(feed.provider.display_name())
            .size(10)
            .style(|theme: &iced::Theme| iced::widget::text::Style {
                color: Some(
                    theme.extended_palette().secondary.weak.color,
                ),
            });

        let name_label = container(text(&feed.name).size(12))
            .width(Length::Fill)
            .clip(true);

        let row_content = row![
            status_dot,
            name_label,
            provider_label,
        ]
        .spacing(8)
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
