mod comparison;
mod controls;
mod heatmap;
pub(crate) mod helpers;
mod kline;
mod modal_stack;
mod profile;
mod starter;

pub(crate) use modal_stack::CompactControls;

use crate::{
    components::display::empty_state::EmptyStateBuilder,
    components::display::progress_bar::ProgressBarBuilder,
    components::input::link_group_button::link_group_button,
    components::primitives::{exchange_icon, icon_text, label::*, Icon},
    modals::{self, pane::Modal},
    screen::dashboard::panel,
    style::{self, palette, tokens},
    window::{self, Window},
};
use data::{ContentKind, UserTimezone};
use exchange::{FuturesTicker, FuturesTickerInfo};
use iced::{
    Alignment, Element, Length, Renderer, Theme,
    alignment::Vertical,
    padding,
    widget::{button, center, column, container, pane_grid, row, text},
};
use rustc_hash::FxHashMap;

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
        tickers_info: &'a FxHashMap<FuturesTicker, FuturesTickerInfo>,
        ticker_ranges: &'a std::collections::HashMap<String, String>,
    ) -> pane_grid::Content<'a, Message, Theme, Renderer> {
        let mut stream_info_element = if matches!(
            self.content,
            Content::Starter
        ) {
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
                        modals::pane::tickers::MiniPanel::new(),
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
            // Visual separator between ticker group and basis modifier
            stream_info_element = stream_info_element
                .push(crate::components::primitives::separator::vertical_divider());
        } else if !matches!(self.content, Content::Starter) {
            let content = row![label_text("Choose a ticker")]
                .align_y(Alignment::Center)
                .spacing(tokens::spacing::XS);

            let tickers_list_btn = button(content)
                .on_press(Message::PaneEvent(
                    id,
                    Event::ShowModal(Modal::MiniTickersList(
                        modals::pane::tickers::MiniPanel::new(),
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

        let modifier: Option<modals::stream::Modifier> = self.modal.clone().and_then(|m| {
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
                let (status_text, progress_value, progress_max) = match &self.loading_status {
                    data::LoadingStatus::Downloading {
                        schema,
                        days_complete,
                        days_total,
                        ..
                    } => (
                        format!("Downloading {} ({}/{})", schema, days_complete, days_total),
                        Some(*days_complete as f32),
                        Some(*days_total as f32),
                    ),
                    data::LoadingStatus::LoadingFromCache {
                        schema,
                        days_loaded,
                        days_total,
                        ..
                    } => (
                        format!("Loading {} ({}/{})", schema, days_loaded, days_total),
                        Some(*days_loaded as f32),
                        Some(*days_total as f32),
                    ),
                    data::LoadingStatus::Building {
                        operation,
                        progress,
                    } => (
                        format!("{} ({:.0}%)", operation, progress * 100.0),
                        Some(*progress * 100.0),
                        Some(100.0_f32),
                    ),
                    _ => ("Loading\u{2026}".to_string(), None, None),
                };

                let content: Element<'a, Message> = if let (Some(val), Some(max)) =
                    (progress_value, progress_max)
                {
                    column![
                        text(status_text.clone()).size(tokens::text::SMALL),
                        ProgressBarBuilder::<Message>::new(val, max)
                            .show_percentage(false)
                            .into_element()
                    ]
                    .spacing(tokens::spacing::XS)
                    .width(Length::Fixed(240.0))
                    .align_x(Alignment::Center)
                    .into()
                } else {
                    heading(status_text).into()
                };
                center(content).into()
            } else if let data::LoadingStatus::Error { message } = &self.loading_status {
                let content = column![
                    text(char::from(Icon::Close).to_string())
                        .font(crate::components::primitives::ICONS_FONT)
                        .size(tokens::chart::EMPTY_STATE_ICON_SIZE)
                        .style(palette::error_text),
                    heading(kind.to_string()),
                    title(message)
                ]
                .spacing(tokens::spacing::MD)
                .align_x(Alignment::Center);

                center(content).into()
            } else {
                center(
                    EmptyStateBuilder::new("Select a ticker to start charting")
                        .icon(Icon::ChartOutline)
                        .into_element(),
                )
                .into()
            }
        };

        let body = match &self.content {
            Content::Starter => {
                self.view_starter_body(id, compact_controls, tickers_info, ticker_ranges)
            }
            Content::Comparison(chart) => {
                let (body, extras) = self.view_comparison_body(
                    id,
                    chart,
                    modifier,
                    compact_controls,
                    uninitialized_base,
                    timezone,
                    tickers_info,
                    ticker_ranges,
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
                        || modals::pane::settings::timesales_cfg_view(panel.config.clone(), id);

                    self.compose_stack_view(
                        base,
                        id,
                        compact_controls,
                        settings_modal,
                        None,
                        tickers_info,
                        ticker_ranges,
                    )
                } else {
                    let base = uninitialized_base(ContentKind::TimeAndSales);
                    self.compose_stack_view(
                        base,
                        id,
                        compact_controls,
                        || column![].into(),
                        None,
                        tickers_info,
                        ticker_ranges,
                    )
                }
            }
            Content::Ladder(panel) => {
                if let Some(panel) = panel {
                    let basis = self
                        .settings
                        .selected_basis
                        .unwrap_or(data::ChartBasis::Time(data::Timeframe::M5));

                    let kind = modals::ModifierKind::Orderbook(basis);

                    let modifiers = helpers::basis_modifier(id, basis, modifier, kind);

                    stream_info_element = stream_info_element.push(modifiers);

                    let base = panel::view(panel, timezone).map(move |message| {
                        Message::PaneEvent(id, Event::PanelInteraction(message))
                    });

                    let settings_modal =
                        || modals::pane::settings::ladder_cfg_view(panel.config.clone(), id);

                    self.compose_stack_view(
                        base,
                        id,
                        compact_controls,
                        settings_modal,
                        None,
                        tickers_info,
                        ticker_ranges,
                    )
                } else {
                    let base = uninitialized_base(ContentKind::Ladder);
                    self.compose_stack_view(
                        base,
                        id,
                        compact_controls,
                        || column![].into(),
                        None,
                        tickers_info,
                        ticker_ranges,
                    )
                }
            }
            Content::Heatmap {
                chart,
                indicators,
                studies: _,
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
                    tickers_info,
                    ticker_ranges,
                );
                for e in extras {
                    stream_info_element = stream_info_element.push(e);
                }
                body
            }
            Content::Kline { chart, .. } => {
                let (body, extras) = self.view_kline_body(
                    id,
                    chart,
                    modifier,
                    compact_controls,
                    uninitialized_base,
                    timezone,
                    tickers_info,
                    ticker_ranges,
                );
                for e in extras {
                    stream_info_element = stream_info_element.push(e);
                }
                body
            }
            Content::Profile { chart, .. } => {
                let (body, extras) = self.view_profile_body(
                    id,
                    chart,
                    modifier,
                    compact_controls,
                    uninitialized_base,
                    timezone,
                    tickers_info,
                    ticker_ranges,
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
                stream_info_element = stream_info_element.push(
                    text(format!(
                        "Downloading {} ({}/{})",
                        schema, days_complete, days_total
                    ))
                    .size(tokens::text::SMALL)
                    .style(palette::info_text),
                );
            }
            data::LoadingStatus::LoadingFromCache {
                schema,
                days_loaded,
                ..
            } => {
                stream_info_element = stream_info_element.push(
                    text(format!("Loading {} ({} days)", schema, days_loaded))
                        .size(tokens::text::SMALL)
                        .style(palette::info_text),
                );
            }
            data::LoadingStatus::Building {
                operation,
                progress,
            } => {
                stream_info_element = stream_info_element.push(
                    text(format!("{} ({:.0}%)", operation, progress * 100.0))
                        .size(tokens::text::SMALL)
                        .style(palette::info_text),
                );
            }
            data::LoadingStatus::Ready | data::LoadingStatus::Idle => {
                if self.feed_id.is_some()
                    && self.ticker_info.is_some()
                    && self.content.initialized()
                {
                    stream_info_element = stream_info_element.push(
                        crate::components::display::status_dot::status_badge_themed(
                            |theme| {
                                let p = theme.extended_palette();
                                p.success.base.color
                            },
                            "LIVE",
                        ),
                    );
                } else if self.feed_id.is_none()
                    && self.ticker_info.is_some()
                    && self.content.initialized()
                {
                    stream_info_element = stream_info_element.push(
                        crate::components::display::status_dot::status_badge_themed(
                            |theme| {
                                let p = theme.extended_palette();
                                p.danger.base.color
                            },
                            "Disconnected",
                        ),
                    );
                }
            }
            data::LoadingStatus::Error { message } => {
                stream_info_element = stream_info_element.push(
                    text(format!("Error: {}", message))
                        .size(tokens::text::SMALL)
                        .style(palette::error_text),
                );
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
