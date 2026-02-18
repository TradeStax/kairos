mod comparison;
mod heatmap;
mod kline;
mod starter;

use crate::{
    component::primitives::{Icon, exchange_icon, icon_text, label::*},
    modal::{self, ModifierKind, pane::Modal},
    screen::dashboard::{panel, tickers_table::TickersTable},
    style::{self, palette, tokens},
    widget::{self, button_with_tooltip, link_group_button},
    window::{self, Window},
};
use data::{ChartBasis, ContentKind, Timeframe, UserTimezone};
use exchange::FuturesTickerInfo;
use iced::{
    Alignment, Element, Length, Renderer, Theme,
    alignment::Vertical,
    padding,
    widget::{button, center, column, container, pane_grid, row, text, tooltip},
};

use super::helpers::{basis_modifier, link_group_modal};
use super::{Content, Event, Message, State};

/// Alias for the optional compact-controls overlay element.
pub(crate) type CompactControls<'a> = Option<Element<'a, Message>>;

impl State {
    pub fn view<'a>(
        &'a self,
        id: pane_grid::Pane,
        panes: usize,
        is_focused: bool,
        maximized: bool,
        window: window::Id,
        main_window: &'a Window,
        timezone: UserTimezone,
        tickers_table: &'a TickersTable,
    ) -> pane_grid::Content<'a, Message, Theme, Renderer> {
        let mut stream_info_element = if Content::Starter == self.content {
            row![]
        } else {
            row![link_group_button(id, self.link_group, |id| {
                Message::PaneEvent(id, Event::ShowModal(Modal::LinkGroup))
            })]
        };

        // Show ticker info button if we have ticker info
        if let Some(ticker_info) = self.ticker_info {
            let exchange_icon = icon_text(exchange_icon(ticker_info.ticker.venue), 14);
            let symbol = ticker_info.ticker.as_str().to_string();

            let content = row![exchange_icon, title(symbol)]
                .align_y(Vertical::Center)
                .spacing(tokens::spacing::XS);

            let tickers_list_btn = button(content)
                .on_press(Message::PaneEvent(
                    id,
                    Event::ShowModal(Modal::MiniTickersList(
                        modal::pane::mini_tickers_list::MiniPanel::new(),
                    )),
                ))
                .style(|theme, status| {
                    style::button::modifier(
                        theme,
                        status,
                        !matches!(self.modal, Some(Modal::MiniTickersList(_))),
                    )
                })
                .padding([4, 10]);

            stream_info_element = stream_info_element.push(tickers_list_btn);
        } else if !matches!(self.content, Content::Starter) {
            // No ticker selected - show prompt
            let content = row![label_text("Choose a ticker")]
                .align_y(Alignment::Center)
                .spacing(tokens::spacing::XS);

            let tickers_list_btn = button(content)
                .on_press(Message::PaneEvent(
                    id,
                    Event::ShowModal(Modal::MiniTickersList(
                        modal::pane::mini_tickers_list::MiniPanel::new(),
                    )),
                ))
                .style(|theme, status| {
                    style::button::modifier(
                        theme,
                        status,
                        !matches!(self.modal, Some(Modal::MiniTickersList(_))),
                    )
                })
                .padding([4, 10]);

            stream_info_element = stream_info_element.push(tickers_list_btn);
        }

        let modifier: Option<modal::stream::Modifier> = self.modal.clone().and_then(|m| {
            if let Modal::StreamModifier(modifier) = m {
                Some(modifier)
            } else {
                None
            }
        });

        let compact_controls: CompactControls<'a> = if self.modal == Some(Modal::Controls) {
            Some(
                container(self.view_controls(id, panes, maximized, window != main_window.id))
                    .style(style::chart_modal)
                    .into(),
            )
        } else {
            None
        };

        let uninitialized_base = |kind: ContentKind| -> Element<'a, Message> {
            if self.loading_status.is_loading() {
                // Show detailed status instead of generic "Loading..."
                let status_text = match &self.loading_status {
                    data::LoadingStatus::Downloading {
                        schema,
                        days_complete,
                        days_total,
                        ..
                    } => {
                        format!("Downloading {} ({}/{})", schema, days_complete, days_total)
                    }
                    data::LoadingStatus::LoadingFromCache {
                        schema,
                        days_loaded,
                        days_total,
                        ..
                    } => {
                        format!("Loading {} ({}/{})", schema, days_loaded, days_total)
                    }
                    data::LoadingStatus::Building {
                        operation,
                        progress,
                    } => {
                        format!("{} ({:.0}%)", operation, progress * 100.0)
                    }
                    _ => "Loading\u{2026}".to_string(),
                };
                center(heading(status_text)).into()
            } else if let data::LoadingStatus::Error { message } = &self.loading_status {
                let content = column![heading(kind.to_string()), title(message)]
                    .spacing(tokens::spacing::MD)
                    .align_x(Alignment::Center);

                center(content).into()
            } else {
                let content = column![
                    heading(kind.to_string()),
                    title("No ticker selected")
                ]
                .spacing(tokens::spacing::MD)
                .align_x(Alignment::Center);

                center(content).into()
            }
        };

        let body = match &self.content {
            Content::Starter => self.view_starter_body(id, compact_controls, tickers_table),
            Content::Comparison(chart) => {
                let (body, extras) = self.view_comparison_body(
                    id,
                    chart,
                    modifier,
                    compact_controls,
                    uninitialized_base,
                    timezone,
                    tickers_table,
                );
                for e in extras {
                    stream_info_element = stream_info_element.push(e);
                }
                body
            }
            Content::TimeAndSales(panel) => {
                if let Some(panel) = panel {
                    let base = panel::view(panel, timezone).map(move |message| {
                        Message::PaneEvent(id, Event::PanelInteraction(message))
                    });

                    let settings_modal =
                        || modal::pane::settings::timesales_cfg_view(panel.config.clone(), id);

                    self.compose_stack_view(
                        base,
                        id,
                        None,
                        compact_controls,
                        settings_modal,
                        None,
                        tickers_table,
                    )
                } else {
                    let base = uninitialized_base(ContentKind::TimeAndSales);
                    self.compose_stack_view(
                        base,
                        id,
                        None,
                        compact_controls,
                        || column![].into(),
                        None,
                        tickers_table,
                    )
                }
            }
            Content::Ladder(panel) => {
                if let Some(panel) = panel {
                    let basis = self
                        .settings
                        .selected_basis
                        .unwrap_or(ChartBasis::Time(Timeframe::M5));

                    let kind = ModifierKind::Orderbook(basis);

                    // Tick multiplier removed - only for crypto
                    let modifiers = basis_modifier(id, basis, modifier, kind);

                    stream_info_element = stream_info_element.push(modifiers);

                    let base = panel::view(panel, timezone).map(move |message| {
                        Message::PaneEvent(id, Event::PanelInteraction(message))
                    });

                    let settings_modal =
                        || modal::pane::settings::ladder_cfg_view(panel.config.clone(), id);

                    self.compose_stack_view(
                        base,
                        id,
                        None,
                        compact_controls,
                        settings_modal,
                        None,
                        tickers_table,
                    )
                } else {
                    let base = uninitialized_base(ContentKind::Ladder);
                    self.compose_stack_view(
                        base,
                        id,
                        None,
                        compact_controls,
                        || column![].into(),
                        None,
                        tickers_table,
                    )
                }
            }
            Content::Heatmap {
                chart,
                indicators,
                studies,
                ..
            } => {
                let (body, extras) = self.view_heatmap_body(
                    id,
                    chart,
                    indicators,
                    modifier,
                    compact_controls,
                    uninitialized_base,
                    timezone,
                    tickers_table,
                );
                for e in extras {
                    stream_info_element = stream_info_element.push(e);
                }
                body
            }
            Content::Kline {
                chart,
                indicators,
                kind: chart_kind,
                ..
            } => {
                let (body, extras) = self.view_kline_body(
                    id,
                    chart,
                    indicators,
                    chart_kind,
                    modifier,
                    compact_controls,
                    uninitialized_base,
                    timezone,
                    tickers_table,
                );
                for e in extras {
                    stream_info_element = stream_info_element.push(e);
                }
                body
            }
        };

        // Show loading status in title bar
        match &self.loading_status {
            data::LoadingStatus::Downloading {
                schema,
                days_complete,
                days_total,
                ..
            } => {
                stream_info_element = stream_info_element.push(text(format!(
                    "Downloading {} ({}/{})",
                    schema, days_complete, days_total
                )));
            }
            data::LoadingStatus::LoadingFromCache {
                schema,
                days_loaded,
                ..
            } => {
                stream_info_element = stream_info_element
                    .push(text(format!("Loading {} ({} days)", schema, days_loaded)));
            }
            data::LoadingStatus::Building {
                operation,
                progress,
            } => {
                stream_info_element = stream_info_element.push(text(format!(
                    "{} ({:.0}%)",
                    operation,
                    progress * 100.0
                )));
            }
            data::LoadingStatus::Ready | data::LoadingStatus::Idle => {
                // Show disconnected indicator if chart has data but no feed
                if self.feed_id.is_none()
                    && self.ticker_info.is_some()
                    && self.content.initialized()
                {
                    stream_info_element = stream_info_element
                        .push(colored("Disconnected", palette::warning_color()));
                }
            }
            data::LoadingStatus::Error { message } => {
                stream_info_element = stream_info_element.push(text(format!("Error: {}", message)));
            }
        }

        let content = pane_grid::Content::new(body)
            .style(move |theme| style::pane_background(theme, is_focused));

        let controls = {
            let compact_control = container(
                button(label_text("...").align_y(Alignment::End))
                    .on_press(Message::PaneEvent(id, Event::ShowModal(Modal::Controls)))
                    .style(move |theme, status| {
                        style::button::transparent(
                            theme,
                            status,
                            self.modal == Some(Modal::Controls)
                                || self.modal == Some(Modal::Settings),
                        )
                    }),
            )
            .align_y(Alignment::Center)
            .height(Length::Fixed(tokens::layout::TITLE_BAR_HEIGHT))
            .padding(tokens::spacing::XS);

            if self.modal == Some(Modal::Controls) {
                pane_grid::Controls::new(compact_control)
            } else {
                pane_grid::Controls::dynamic(
                    self.view_controls(id, panes, maximized, window != main_window.id),
                    compact_control,
                )
            }
        };

        let title_bar = pane_grid::TitleBar::new(
            stream_info_element
                .padding(padding::left(tokens::spacing::XS).top(tokens::spacing::XXXS))
                .align_y(Vertical::Center)
                .spacing(tokens::spacing::MD)
                .height(Length::Fixed(tokens::layout::TITLE_BAR_HEIGHT)),
        )
        .controls(controls)
        .style(style::pane_title_bar);

        content.title_bar(if self.modal.is_none() {
            title_bar
        } else {
            title_bar.always_show_controls()
        })
    }

    pub(crate) fn view_controls(
        &'_ self,
        pane: pane_grid::Pane,
        total_panes: usize,
        is_maximized: bool,
        is_popout: bool,
    ) -> Element<'_, Message> {
        let modal_btn_style = |modal: Modal| {
            let is_active = self.modal == Some(modal);
            move |theme: &Theme, status: button::Status| {
                style::button::transparent(theme, status, is_active)
            }
        };

        let control_btn_style = |is_active: bool| {
            move |theme: &Theme, status: button::Status| {
                style::button::transparent(theme, status, is_active)
            }
        };

        let treat_as_starter =
            matches!(&self.content, Content::Starter) || !self.content.initialized();

        let tooltip_pos = tooltip::Position::Bottom;
        let mut buttons = row![];

        let show_modal = |modal: Modal| Message::PaneEvent(pane, Event::ShowModal(modal));

        if !treat_as_starter {
            // Settings button
            buttons = buttons.push(button_with_tooltip(
                icon_text(Icon::Cog, 12),
                show_modal(Modal::Settings),
                None,
                tooltip_pos,
                modal_btn_style(Modal::Settings),
            ));
        }
        if !treat_as_starter
            && matches!(
                &self.content,
                Content::Heatmap { .. } | Content::Kline { .. }
            )
        {
            buttons = buttons.push(button_with_tooltip(
                icon_text(Icon::ChartOutline, 12),
                show_modal(Modal::Indicators),
                Some("Indicators"),
                tooltip_pos,
                modal_btn_style(Modal::Indicators),
            ));
        }

        if is_popout {
            buttons = buttons.push(button_with_tooltip(
                icon_text(Icon::Popout, 12),
                Message::Merge,
                Some("Merge"),
                tooltip_pos,
                control_btn_style(is_popout),
            ));
        } else if total_panes > 1 {
            buttons = buttons.push(button_with_tooltip(
                icon_text(Icon::Popout, 12),
                Message::Popout,
                Some("Pop out"),
                tooltip_pos,
                control_btn_style(is_popout),
            ));
        }

        if total_panes > 1 {
            let (resize_icon, message) = if is_maximized {
                (Icon::ResizeSmall, Message::Restore)
            } else {
                (Icon::ResizeFull, Message::MaximizePane(pane))
            };

            buttons = buttons.push(button_with_tooltip(
                icon_text(resize_icon, 12),
                message,
                None,
                tooltip_pos,
                control_btn_style(is_maximized),
            ));

            buttons = buttons.push(button_with_tooltip(
                icon_text(Icon::Close, 12),
                Message::ClosePane(pane),
                None,
                tooltip_pos,
                control_btn_style(false),
            ));
        }

        buttons
            .padding(padding::right(tokens::spacing::XS).left(tokens::spacing::XS))
            .align_y(Vertical::Center)
            .height(Length::Fixed(tokens::layout::TITLE_BAR_HEIGHT))
            .into()
    }

    pub(crate) fn compose_stack_view<'a, F>(
        &'a self,
        base: Element<'a, Message>,
        pane: pane_grid::Pane,
        indicator_modal: Option<Element<'a, Message>>,
        compact_controls: Option<Element<'a, Message>>,
        settings_modal: F,
        selected_tickers: Option<&'a [FuturesTickerInfo]>,
        tickers_table: &'a TickersTable,
    ) -> Element<'a, Message>
    where
        F: FnOnce() -> Element<'a, Message>,
    {
        use modal::pane::stack_modal;

        let base =
            widget::toast::Manager::new(base, &self.notifications, Alignment::End, move |msg| {
                Message::PaneEvent(pane, Event::DeleteNotification(msg))
            })
            .into();

        let on_blur = Message::PaneEvent(pane, Event::HideModal);

        match &self.modal {
            Some(Modal::LinkGroup) => {
                let content = link_group_modal(pane, self.link_group);

                stack_modal(
                    base,
                    content,
                    on_blur,
                    padding::right(tokens::spacing::LG).left(tokens::spacing::XS),
                    Alignment::Start,
                )
            }
            Some(Modal::StreamModifier(modifier)) => stack_modal(
                base,
                modifier.view(self.ticker_info).map(move |message| {
                    Message::PaneEvent(pane, Event::StreamModifierChanged(message))
                }),
                Message::PaneEvent(pane, Event::HideModal),
                padding::right(tokens::spacing::LG).left(48),
                Alignment::Start,
            ),
            Some(Modal::MiniTickersList(panel)) => {
                let mini_list = panel
                    .view(tickers_table, selected_tickers, self.ticker_info)
                    .map(move |msg| {
                        Message::PaneEvent(pane, Event::MiniTickersListInteraction(msg))
                    });

                let content: Element<_> = container(mini_list)
                    .max_width(260)
                    .max_height(480)
                    .clip(true)
                    .padding(tokens::spacing::XL)
                    .style(style::chart_modal)
                    .into();

                stack_modal(
                    base,
                    content,
                    Message::PaneEvent(pane, Event::HideModal),
                    padding::left(tokens::spacing::LG),
                    Alignment::Start,
                )
            }
            Some(Modal::Settings) => stack_modal(
                base,
                settings_modal(),
                on_blur,
                padding::right(tokens::spacing::LG).left(tokens::spacing::LG),
                Alignment::End,
            ),
            Some(Modal::Indicators) => stack_modal(
                base,
                indicator_modal.unwrap_or_else(|| column![].into()),
                on_blur,
                padding::right(tokens::spacing::LG).left(tokens::spacing::LG),
                Alignment::End,
            ),
            Some(Modal::Controls) => stack_modal(
                base,
                if let Some(controls) = compact_controls {
                    controls
                } else {
                    column![].into()
                },
                on_blur,
                padding::left(tokens::spacing::LG),
                Alignment::End,
            ),
            Some(Modal::DataManagement(panel)) => {
                let pane_id = pane;
                stack_modal(
                    base,
                    panel.view().map(move |msg| {
                        Message::PaneEvent(pane_id, Event::DataManagementInteraction(msg))
                    }),
                    on_blur,
                    padding::all(tokens::spacing::LG),
                    Alignment::Center,
                )
            }
            None => base,
        }
    }
}
