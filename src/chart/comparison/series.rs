//! Series editor for comparison chart
//!
//! This module provides the UI for editing series properties
//! such as color and custom label names.

use crate::style;
use crate::widget::chart::Series;
use crate::widget::color_picker::color_picker;
use exchange::FuturesTickerInfo;
use iced::widget::{button, column, container, row, text};
use iced::{Element, Length};
use palette::Hsva;

use super::{Action, old_format_to_ticker_info};

const MAX_LABEL_CHARS: usize = 24;

#[derive(Debug, Clone)]
pub enum Message {
    ToggleEditFor {
        ticker: FuturesTickerInfo,
        applied_color: iced::Color,
        applied_name: Option<String>,
    },
    ColorChangedHsva(Hsva),
    NameChanged(String),
}

#[derive(Default)]
pub struct TickerSeriesEditor {
    pub show_config_for: Option<FuturesTickerInfo>,
    pub editing_color: Option<Hsva>,
    pub editing_name: Option<String>,
}

impl TickerSeriesEditor {
    pub fn update(&mut self, msg: Message) -> Option<Action> {
        match msg {
            Message::ToggleEditFor {
                ticker,
                applied_color,
                applied_name,
            } => {
                if let Some(current) = self.show_config_for
                    && current == ticker
                {
                    self.show_config_for = None;
                    self.editing_color = None;
                    self.editing_name = None;
                    return None;
                }
                self.show_config_for = Some(ticker);
                self.editing_color = Some(data::config::theme::to_hsva(applied_color));
                self.editing_name = applied_name;
                None
            }
            Message::ColorChangedHsva(hsva) => {
                self.editing_color = Some(hsva);
                if let Some(t) = self.show_config_for {
                    return Some(Action::SeriesColorChanged(
                        t,
                        data::config::theme::from_hsva(hsva),
                    ));
                }
                None
            }
            Message::NameChanged(new_name) => {
                let trimmed = new_name.trim();
                let limited = Self::clamp(trimmed);
                self.editing_name = Some(limited.clone());
                if let Some(t) = self.show_config_for {
                    return Some(Action::SeriesNameChanged(t, limited));
                }
                None
            }
        }
    }

    pub fn view<'a>(&'a self, series: &'a [Series]) -> Element<'a, Message> {
        let mut content = column![].spacing(6);

        for s in series {
            let applied = s.color;
            let futures_info = old_format_to_ticker_info(&s.ticker_info);
            let is_open = self.show_config_for.is_some_and(|t| t == futures_info);

            let header = button(
                row![
                    container("").width(14).height(14).style(move |theme| {
                        style::colored_circle_container(theme, applied)
                    }),
                    text(futures_info.ticker.as_str().to_string()).size(13),
                ]
                .width(Length::Fill)
                .spacing(8)
                .align_y(iced::Alignment::Center),
            )
            .on_press(Message::ToggleEditFor {
                ticker: futures_info,
                applied_color: applied,
                applied_name: s.name.clone(),
            })
            .style(move |theme, status| style::button::transparent(theme, status, is_open))
            .width(Length::Fill);

            let mut col = column![header].padding(4);
            let mut inner_col = column![];

            if is_open {
                let hsva_in = self
                    .editing_color
                    .unwrap_or_else(|| data::config::theme::to_hsva(applied));
                inner_col = inner_col.push(color_picker(hsva_in, Message::ColorChangedHsva));

                let label_name = self
                    .editing_name
                    .clone()
                    .unwrap_or_else(|| s.name.clone().unwrap_or_default());
                inner_col = inner_col.push(
                    iced::widget::text_input("Set a custom label name", &label_name)
                        .on_input(Message::NameChanged)
                        .size(14)
                        .padding(4)
                        .width(Length::Fill),
                );

                col = col.push(inner_col.spacing(12).padding(4)).spacing(4);
            }

            content = content.push(container(col).style(style::modal_container));
        }

        content.into()
    }

    fn clamp(s: &str) -> String {
        s.chars().take(MAX_LABEL_CHARS).collect()
    }
}
