mod comparison;
mod controls;
mod heatmap;
pub(crate) mod helpers;
mod kline;
mod modal_stack;
mod starter;

pub(crate) use modal_stack::CompactControls;

use crate::{
    component::primitives::{Icon, exchange_icon, icon_text, label::*},
    component::input::link_group_button::link_group_button,
    modal::{self, pane::Modal},
    screen::dashboard::{panel, tickers_table::TickersTable},
    style::{self, palette, tokens},
    window::{self, Window},
};
use data::{ContentKind, UserTimezone};
use iced::{
    Alignment, Element, Length, Renderer, Theme,
    alignment::Vertical,
    padding,
    widget::{button, center, column, container, pane_grid, row, text},
};

use super::{Content, Event, Message, State};

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
                        modal::pane::tickers::MiniPanel::new(),
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
            let content = row![label_text("Choose a ticker")]
                .align_y(Alignment::Center)
                .spacing(tokens::spacing::XS);

            let tickers_list_btn = button(content)
                .on_press(Message::PaneEvent(
                    id,
                    Event::ShowModal(Modal::MiniTickersList(
                        modal::pane::tickers::MiniPanel::new(),
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
                        .unwrap_or(data::ChartBasis::Time(data::Timeframe::M5));

                    let kind = modal::ModifierKind::Orderbook(basis);

                    let modifiers = helpers::basis_modifier(id, basis, modifier, kind);

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
}
