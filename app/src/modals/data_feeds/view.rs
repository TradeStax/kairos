//! View methods for the Data Feeds modal
//!
//! Contains view(), view_left_panel(), view_feed_item(), view_right_panel(),
//! view_edit_form(), view_databento_fields(), view_rithmic_fields(), and
//! the section_header helper.

use crate::components;
use crate::components::overlay::modal_header::ModalHeaderBuilder;
use crate::style;
use crate::style::{palette, tokens};
use data::feed::{DataFeed, FeedKind, FeedProvider, FeedStatus, RithmicEnvironment, RithmicServer};

/// Maps a feed connection status to a display color.
fn feed_status_color(theme: &iced::Theme, status: &FeedStatus) -> iced::Color {
    match status {
        FeedStatus::Connected => palette::success_color(theme),
        FeedStatus::Connecting => theme.extended_palette().warning.strong.color,
        FeedStatus::Downloading { .. } => palette::info_color(theme),
        FeedStatus::Error(_) => palette::error_color(theme),
        FeedStatus::Disconnected => palette::neutral_color(theme),
    }
}
use iced::{
    Alignment, Element, Length, padding,
    widget::{
        button, column, container, mouse_area, pick_list, row, rule, scrollable, space, stack,
        text, text_input,
    },
};

use super::{DataFeedsMessage, DataFeedsModal};

/// Fallback Rithmic system names (used when server probe hasn't completed)
const RITHMIC_SYSTEM_NAMES_FALLBACK: &[&str] =
    &["Rithmic Paper Trading", "Rithmic 01", "Rithmic Test"];

/// Tickers available for Rithmic subscription
const RITHMIC_TICKERS: &[(&str, &str)] = &[
    ("ES", "E-mini S&P 500"),
    ("NQ", "E-mini Nasdaq-100"),
    ("YM", "E-mini Dow"),
    ("RTY", "E-mini Russell 2000"),
    ("CL", "Crude Oil"),
    ("GC", "Gold"),
    ("SI", "Silver"),
    ("ZN", "10-Year T-Note"),
    ("ZB", "30-Year T-Bond"),
    ("ZF", "5-Year T-Note"),
    ("NG", "Natural Gas"),
    ("HG", "Copper"),
];

impl DataFeedsModal {
    pub fn view(&self) -> Element<'_, DataFeedsMessage> {
        let header =
            ModalHeaderBuilder::new("Manage Connections").on_close(DataFeedsMessage::Close);

        let left_panel = self.view_left_panel();
        let right_panel = self.view_right_panel();

        let body = row![
            left_panel,
            rule::vertical(1).style(style::split_ruler),
            right_panel,
        ]
        .height(420);

        let content = column![header, body,];

        container(content)
            .width(650)
            .style(style::dashboard_modal)
            .into()
    }

    fn view_left_panel(&self) -> Element<'_, DataFeedsMessage> {
        let feeds = &self.feeds_snapshot;

        let historical = feeds.historical_feeds();
        let realtime = feeds.realtime_feeds();

        let mut feed_list = column![].spacing(tokens::spacing::XXS);

        // Datasets section
        if !historical.is_empty() {
            feed_list = feed_list.push(section_header("Datasets"));
            for feed in &historical {
                let is_selected = self.selected_feed == Some(feed.id);
                feed_list = feed_list.push(self.view_feed_item(feed, is_selected));
            }
        }

        // Connections section
        if !realtime.is_empty() {
            feed_list = feed_list.push(section_header("Connections"));
            for feed in feeds.feeds_by_priority() {
                if feed.is_realtime() {
                    let is_selected = self.selected_feed == Some(feed.id);
                    feed_list = feed_list.push(self.view_feed_item(feed, is_selected));
                }
            }
        }

        if feeds.total_count() == 0 && !self.is_creating {
            feed_list = feed_list.push(
                container(components::primitives::body("No connections"))
                    .padding([tokens::spacing::XL, tokens::spacing::LG])
                    .width(Length::Fill)
                    .align_x(Alignment::Center),
            );
        }

        // "+" and "-" buttons
        let add_button = button(components::primitives::title("+").align_x(Alignment::Center))
            .width(28)
            .height(28)
            .on_press(DataFeedsMessage::ToggleAddPopup);

        let remove_button: Option<Element<'_, DataFeedsMessage>> = self.selected_feed.map(|id| {
            button(components::primitives::title("\u{2212}").align_x(Alignment::Center))
                .width(28)
                .height(28)
                .on_press(DataFeedsMessage::RemoveFeed(id))
                .style(style::button::secondary)
                .into()
        });

        let mut button_row = row![add_button].spacing(tokens::spacing::XS);
        if let Some(rm) = remove_button {
            button_row = button_row.push(rm);
        }

        let add_area: Element<'_, DataFeedsMessage> = if self.add_popup_open {
            let popup = container(
                column![
                    button(components::primitives::body("Historical"))
                        .width(Length::Fill)
                        .on_press(DataFeedsMessage::OpenHistoricalDownload,)
                        .padding([tokens::spacing::XS, tokens::spacing::LG]),
                    button(components::primitives::body("Realtime"))
                        .width(Length::Fill)
                        .on_press(DataFeedsMessage::AddRealtime)
                        .padding([tokens::spacing::XS, tokens::spacing::LG]),
                ]
                .spacing(tokens::spacing::XXS),
            )
            .padding(tokens::spacing::XS)
            .style(style::dashboard_modal);

            stack![
                mouse_area(
                    container(space::horizontal())
                        .width(200)
                        .height(Length::Fill)
                )
                .on_press(DataFeedsMessage::CloseAddPopup),
                column![
                    space::vertical().height(Length::Fill),
                    popup,
                    container(button_row).padding([tokens::spacing::SM, tokens::spacing::MD]),
                ],
            ]
            .height(Length::Fill)
            .into()
        } else {
            column![
                space::vertical().height(Length::Fill),
                rule::horizontal(1).style(style::split_ruler),
                container(button_row).padding([tokens::spacing::SM, tokens::spacing::MD]),
            ]
            .into()
        };

        column![
            mouse_area(
                scrollable(feed_list.padding([tokens::spacing::XS, 0.0])).height(Length::Fill),
            )
            .on_press(DataFeedsMessage::DeselectFeed),
            add_area,
        ]
        .width(200)
        .into()
    }

    fn view_feed_item<'a>(
        &self,
        feed: &'a DataFeed,
        is_selected: bool,
    ) -> Element<'a, DataFeedsMessage> {
        let indicator: Element<'a, DataFeedsMessage> = if feed.is_historical() {
            // Small "DB" label for datasets
            container(text("DB").size(8).align_x(Alignment::Center))
                .width(18)
                .height(18)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .style(|theme: &iced::Theme| container::Style {
                    background: Some(iced::Background::Color(
                        palette::info_color(theme).scale_alpha(tokens::alpha::SUBTLE),
                    )),
                    border: iced::Border {
                        radius: 3.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .into()
        } else {
            // Status dot for connections
            let feed_status = feed.status.clone();
            components::display::status_dot_themed(move |theme| {
                feed_status_color(theme, &feed_status)
            })
        };

        let info = column![
            components::primitives::label_text(&feed.name),
            components::primitives::tiny(feed.provider.display_name()),
        ]
        .spacing(tokens::spacing::XXXS);

        let item_content = row![indicator, info]
            .spacing(tokens::spacing::MD)
            .align_y(Alignment::Center);

        let feed_id = feed.id;
        let btn = button(item_content)
            .width(Length::Fill)
            .on_press(DataFeedsMessage::SelectFeed(feed_id))
            .padding([tokens::spacing::SM, tokens::spacing::LG]);

        if is_selected {
            btn.style(style::button::primary).into()
        } else {
            btn.style(style::button::list_item).into()
        }
    }

    fn view_right_panel(&self) -> Element<'_, DataFeedsMessage> {
        let feeds = &self.feeds_snapshot;

        match self.selected_feed {
            None => container(components::primitives::label_text(
                "Select a connection or click + to add one",
            ))
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .into(),

            Some(id) => {
                if let Some(feed) = feeds.get(id) {
                    match &feed.kind {
                        FeedKind::Historical(info) => self.view_historical_panel(feed, info),
                        FeedKind::Realtime => self.view_edit_form(feed),
                    }
                } else {
                    container(components::primitives::label_text("Feed not found"))
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .align_x(Alignment::Center)
                        .align_y(Alignment::Center)
                        .into()
                }
            }
        }
    }

    fn view_edit_form<'a>(&'a self, feed: &'a DataFeed) -> Element<'a, DataFeedsMessage> {
        // Name + Type on the same row (3/4 name, 1/4 type)
        let type_col: Element<'_, DataFeedsMessage> = if self.is_creating {
            column![
                components::primitives::body("Type"),
                pick_list(
                    FeedProvider::ALL,
                    self.edit_form.provider,
                    DataFeedsMessage::SetProvider,
                )
                .text_size(tokens::text::LABEL),
            ]
            .spacing(tokens::spacing::XS)
            .width(Length::FillPortion(1))
            .into()
        } else {
            column![
                components::primitives::body("Type"),
                text_input("", feed.provider.display_name()).size(tokens::text::LABEL),
            ]
            .spacing(tokens::spacing::XS)
            .width(Length::FillPortion(1))
            .into()
        };

        let name_type_row = row![
            column![
                components::primitives::body("Name"),
                text_input("Connection name", &self.edit_form.name)
                    .on_input(DataFeedsMessage::SetName)
                    .size(tokens::text::LABEL),
            ]
            .spacing(tokens::spacing::XS)
            .width(Length::FillPortion(3)),
            type_col,
        ]
        .spacing(tokens::spacing::MD);

        // Auto-connect on startup toggle
        let auto_connect_toggle = components::input::toggle_switch::toggle_switch(
            "Connect on startup",
            self.edit_form.auto_connect,
            DataFeedsMessage::SetAutoConnect,
        );

        // Provider-specific fields
        let provider_fields: Element<'_, DataFeedsMessage> = match self.edit_form.provider {
            Some(FeedProvider::Databento) => self.view_databento_fields(),
            Some(FeedProvider::Rithmic) => self.view_rithmic_fields(feed.id),
            None => space::vertical().height(0).into(),
        };

        let form_content = column![
            name_type_row,
            auto_connect_toggle,
            rule::horizontal(1).style(style::split_ruler),
            provider_fields,
        ]
        .spacing(tokens::spacing::LG)
        .padding([tokens::spacing::LG, tokens::spacing::XL]);

        // Footer
        let footer = container(
            row![
                space::horizontal().width(Length::Fill),
                button(components::primitives::label_text("Cancel"))
                    .on_press(DataFeedsMessage::CancelEdit)
                    .padding([tokens::spacing::XS, tokens::spacing::LG])
                    .style(style::button::secondary),
                button(components::primitives::label_text("Save"))
                    .on_press(DataFeedsMessage::SaveFeed)
                    .padding([tokens::spacing::XS, tokens::spacing::LG])
                    .style(style::button::primary),
            ]
            .spacing(tokens::spacing::MD),
        )
        .padding([tokens::spacing::MD, tokens::spacing::XL]);

        column![
            scrollable(form_content).height(Length::Fill),
            rule::horizontal(1).style(style::split_ruler),
            footer,
        ]
        .width(Length::Fill)
        .into()
    }

    fn view_databento_fields(&self) -> Element<'_, DataFeedsMessage> {
        let has_saved_key =
            crate::infra::secrets::SecretsManager::new().has_api_key(data::ApiProvider::Databento);
        let key_placeholder = if has_saved_key {
            "API key saved (leave blank to keep)"
        } else {
            "Enter Databento API key"
        };

        let api_key_field: Element<'_, DataFeedsMessage> =
            components::input::secure_field::SecureFieldBuilder::new(
                "API Key",
                key_placeholder,
                &self.edit_form.api_key,
                DataFeedsMessage::SetApiKey,
            )
            .show_set_indicator(has_saved_key)
            .into();

        let cache_toggle = components::input::toggle_switch::toggle_switch(
            "Enable caching",
            self.edit_form.cache_enabled,
            DataFeedsMessage::SetCacheEnabled,
        );

        let cache_days = column![
            components::primitives::body("Cache max days"),
            text_input("90", &self.edit_form.cache_max_days)
                .on_input(DataFeedsMessage::SetCacheMaxDays)
                .size(tokens::text::LABEL),
        ]
        .spacing(tokens::spacing::XS);

        column![
            components::primitives::title("Databento Settings"),
            api_key_field,
            cache_toggle,
            cache_days,
        ]
        .spacing(tokens::spacing::MD)
        .into()
    }

    fn view_rithmic_fields(&self, feed_id: data::FeedId) -> Element<'_, DataFeedsMessage> {
        // Environment
        let env_options: Vec<String> = RithmicEnvironment::ALL
            .iter()
            .map(|e| e.to_string())
            .collect();
        let selected_env = Some(self.edit_form.environment.to_string());

        // Server dropdown
        let server_options: Vec<String> =
            RithmicServer::ALL.iter().map(|s| s.to_string()).collect();
        let selected_server = Some(self.edit_form.server.to_string());

        let env_server_row = row![
            column![
                components::primitives::body("Environment"),
                pick_list(env_options, selected_env, |selected| {
                    let env = RithmicEnvironment::ALL
                        .iter()
                        .find(|e| e.to_string() == selected)
                        .copied()
                        .unwrap_or(RithmicEnvironment::Demo);
                    DataFeedsMessage::SetEnvironment(env)
                })
                .text_size(tokens::text::BODY),
            ]
            .spacing(tokens::spacing::XS)
            .width(Length::FillPortion(1)),
            column![
                components::primitives::body("Server"),
                pick_list(server_options, selected_server, |selected| {
                    let server = RithmicServer::ALL
                        .iter()
                        .find(|s| s.to_string() == selected)
                        .copied()
                        .unwrap_or(RithmicServer::Chicago);
                    DataFeedsMessage::SetServer(server)
                },)
                .text_size(tokens::text::BODY),
            ]
            .spacing(tokens::spacing::XS)
            .width(Length::FillPortion(2)),
        ]
        .spacing(tokens::spacing::MD);

        // System name — dynamic from probe, with fallback
        let system_name_options: Vec<String> = if !self.edit_form.available_system_names.is_empty()
        {
            self.edit_form.available_system_names.clone()
        } else {
            RITHMIC_SYSTEM_NAMES_FALLBACK
                .iter()
                .map(|s| s.to_string())
                .collect()
        };
        let selected_system = if self.edit_form.system_name.is_empty() {
            None
        } else {
            Some(self.edit_form.system_name.clone())
        };

        let system_name_label: Element<'_, DataFeedsMessage> =
            if self.edit_form.system_names_loading {
                components::primitives::body("System Name (loading...)").into()
            } else {
                components::primitives::body("System Name").into()
            };

        let system_name_field = column![
            system_name_label,
            pick_list(
                system_name_options,
                selected_system,
                DataFeedsMessage::SetSystemName,
            )
            .text_size(tokens::text::BODY),
        ]
        .spacing(tokens::spacing::XS);

        // Account ID (full width, standalone)
        let account_id = column![
            components::primitives::body("Account ID"),
            text_input("", &self.edit_form.account_id)
                .on_input(DataFeedsMessage::SetAccountId)
                .size(tokens::text::LABEL),
        ]
        .spacing(tokens::spacing::XS);

        let user_id = column![
            components::primitives::body("User ID"),
            text_input("Your Rithmic user ID", &self.edit_form.user_id,)
                .on_input(DataFeedsMessage::SetUserId)
                .size(tokens::text::LABEL),
        ]
        .spacing(tokens::spacing::XS);

        let password_field = {
            let has_saved = crate::infra::secrets::SecretsManager::new()
                .has_feed_password(&feed_id.to_string());
            let placeholder = if has_saved {
                "Password saved (leave blank to keep)"
            } else {
                "Enter password"
            };

            components::input::secure_field::SecureFieldBuilder::new(
                "Password",
                placeholder,
                &self.edit_form.password,
                DataFeedsMessage::SetPassword,
            )
            .show_set_indicator(has_saved)
            .into_element()
        };

        // Subscribed tickers — use dynamic list if available
        let ticker_source: Vec<(&str, &str)> = if !self.edit_form.available_tickers.is_empty() {
            self.edit_form
                .available_tickers
                .iter()
                .map(|s| (s.as_str(), s.as_str()))
                .collect()
        } else {
            RITHMIC_TICKERS.to_vec()
        };

        let selected_tickers = &self.edit_form.subscribed_tickers;
        let mut tickers_col = column![text("Subscribed Tickers").size(tokens::text::LABEL),]
            .spacing(tokens::spacing::XS);

        for (symbol, _label) in &ticker_source {
            let is_checked = selected_tickers.iter().any(|t| t == *symbol);
            let sym = symbol.to_string();
            let cb = iced::widget::checkbox(is_checked)
                .label(*symbol)
                .on_toggle(move |_| DataFeedsMessage::ToggleTicker(sym.clone()))
                .text_size(tokens::text::BODY)
                .spacing(tokens::spacing::XS);
            tickers_col = tickers_col.push(cb);
        }
        let tickers_field = tickers_col;

        let reconnect_toggle = components::input::toggle_switch::toggle_switch(
            "Auto-reconnect",
            self.edit_form.auto_reconnect,
            DataFeedsMessage::SetAutoReconnect,
        );

        column![
            components::primitives::title("Rithmic Settings"),
            env_server_row,
            system_name_field,
            account_id,
            user_id,
            password_field,
            tickers_field,
            reconnect_toggle,
        ]
        .spacing(tokens::spacing::MD)
        .into()
    }
}

fn section_header(label: &str) -> Element<'_, DataFeedsMessage> {
    container(components::primitives::small(label))
        .padding(
            padding::top(tokens::spacing::SM)
                .right(tokens::spacing::LG)
                .bottom(tokens::spacing::XXS)
                .left(tokens::spacing::LG),
        )
        .width(Length::Fill)
        .into()
}
