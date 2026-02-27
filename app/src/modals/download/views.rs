//! Shared view functions for the download subsystem.
//!
//! Reusable UI elements used by both DataManagementPanel and HistoricalDownloadModal.

use super::{CacheStatus, DownloadConfig, DownloadProgress};
use crate::style::{self, tokens};
use iced::{
    Alignment, Color, Element, Length,
    widget::{
        button, center, column, container, mouse_area, opaque, pick_list, progress_bar, row, space,
        stack, text,
    },
};

/// Ticker dropdown with label
pub fn ticker_dropdown<'a, Message: Clone + 'a>(
    selected_idx: usize,
    on_select: impl Fn(usize) -> Message + 'a,
) -> Element<'a, Message> {
    let ticker_options = DownloadConfig::ticker_options();
    let selected = DownloadConfig::ticker_display(selected_idx);

    column![
        text("Ticker").size(tokens::text::LABEL),
        pick_list(ticker_options, Some(selected), move |selected: String| {
            on_select(DownloadConfig::find_ticker_idx(&selected))
        })
        .width(Length::Fill),
    ]
    .spacing(tokens::spacing::XS)
    .into()
}

/// Schema dropdown with label
pub fn schema_dropdown<'a, Message: Clone + 'a>(
    selected_idx: usize,
    on_select: impl Fn(usize) -> Message + 'a,
) -> Element<'a, Message> {
    let schema_options = DownloadConfig::schema_options();
    let selected = DownloadConfig::schema_display(selected_idx);

    column![
        text("Schema").size(tokens::text::LABEL),
        pick_list(schema_options, Some(selected), move |selected: String| {
            on_select(DownloadConfig::find_schema_idx(&selected))
        })
        .width(Length::Fill),
    ]
    .spacing(tokens::spacing::XS)
    .into()
}

/// Cache status display line
pub fn cache_status_display<'a, Message: 'a>(
    progress: &DownloadProgress,
    cache_status: Option<&CacheStatus>,
) -> Element<'a, Message> {
    if matches!(progress, DownloadProgress::CheckingCost) {
        text("Checking cost...")
            .size(tokens::text::SMALL)
            .style(|theme: &iced::Theme| iced::widget::text::Style {
                color: Some(theme.extended_palette().primary.base.color),
            })
            .into()
    } else if let Some(status) = cache_status {
        let total = status.total_days;
        let cached = status.cached_days;
        let uncached = status.uncached_days;

        let cache_line = if cached == total {
            format!("All {} days already downloaded", total)
        } else if cached > 0 {
            format!(
                "{}/{} days cached ({} to download)",
                cached, total, uncached
            )
        } else {
            format!("Need to download all {} days", total)
        };

        let cost_suffix = match status.estimated_cost_usd {
            Some(cost) if cost < 0.01 => " \u{2014} Est. cost: <$0.01".to_string(),
            Some(cost) => format!(" \u{2014} Est. cost: ${:.2}", cost),
            None => String::new(),
        };

        let display = format!("{}{}", cache_line, cost_suffix);

        let text_style: fn(&iced::Theme) -> iced::widget::text::Style =
            if cached == total {
                |theme: &iced::Theme| iced::widget::text::Style {
                    color: Some(
                        theme.extended_palette().success.base.color,
                    ),
                }
            } else if cached > 0 {
                |theme: &iced::Theme| iced::widget::text::Style {
                    color: Some(
                        theme.extended_palette().primary.base.color,
                    ),
                }
            } else {
                |theme: &iced::Theme| iced::widget::text::Style {
                    color: Some(
                        theme.extended_palette().secondary.base.color,
                    ),
                }
            };

        text(display)
            .size(tokens::text::BODY)
            .style(text_style)
            .into()
    } else {
        text("Select date range to see cache status")
            .size(tokens::text::SMALL)
            .into()
    }
}

/// Download progress section (progress bar + labels)
pub fn download_progress_section<'a, Message: 'a>(
    progress: &DownloadProgress,
) -> Option<Element<'a, Message>> {
    match progress {
        DownloadProgress::Downloading {
            current_day,
            total_days,
        } => {
            let pct = if *total_days > 0 {
                (*current_day as f32 / *total_days as f32) * 100.0
            } else {
                0.0
            };

            Some(
                container(
                    column![
                        row![
                            text("Downloading...").size(tokens::text::BODY),
                            space::horizontal().width(Length::Fill),
                            text(format!(
                                "{}/{} days ({}%)",
                                current_day, total_days, pct as u32
                            ))
                            .size(tokens::text::SMALL),
                        ]
                        .align_y(Alignment::Center),
                        progress_bar(0.0..=100.0, pct)
                            .girth(6.0)
                            .style(style::progress_bar),
                    ]
                    .spacing(tokens::spacing::SM),
                )
                .padding(tokens::spacing::LG)
                .style(style::modal_container)
                .into(),
            )
        }
        DownloadProgress::Complete { days_downloaded } => Some(
            container(
                text(format!("Download complete - {} days", days_downloaded))
                    .size(tokens::text::BODY)
                    .style(|theme: &iced::Theme| iced::widget::text::Style {
                        color: Some(theme.extended_palette().success.base.color),
                    }),
            )
            .padding(tokens::spacing::LG)
            .style(style::modal_container)
            .into(),
        ),
        DownloadProgress::Error(err) => Some(
            container(
                text(format!("Error: {}", err))
                    .size(tokens::text::BODY)
                    .style(|theme: &iced::Theme| iced::widget::text::Style {
                        color: Some(theme.extended_palette().danger.base.color),
                    }),
            )
            .padding(tokens::spacing::LG)
            .style(style::modal_container)
            .into(),
        ),
        _ => None,
    }
}

/// Confirmation overlay for download actions
pub fn download_confirm_overlay<'a, Message: Clone + 'a>(
    base: Element<'a, Message>,
    ticker_idx: usize,
    schema_idx: usize,
    start_date: chrono::NaiveDate,
    end_date: chrono::NaiveDate,
    cache_status: Option<&CacheStatus>,
    on_cancel: Message,
    on_confirm: Message,
) -> Element<'a, Message> {
    let (symbol, name) = super::FUTURES_PRODUCTS[ticker_idx];
    let (_, schema_name, _) = super::SCHEMAS[schema_idx];
    let total_days = (end_date - start_date).num_days().max(0) + 1;
    let cached_days = cache_status
        .map(|s| s.cached_days.min(total_days as usize))
        .unwrap_or(0);
    let uncached_days = (total_days as usize).saturating_sub(cached_days);

    let cost_label = cache_status
        .and_then(|s| s.estimated_cost_usd)
        .map(|c| {
            if c < 0.01 {
                "Estimated cost: <$0.01".to_string()
            } else {
                format!("Estimated cost: ${:.2}", c)
            }
        });

    let status_text = if cached_days == total_days as usize {
        text("All data already cached — no download needed")
            .size(tokens::text::TITLE)
            .style(|theme: &iced::Theme| iced::widget::text::Style {
                color: Some(theme.extended_palette().success.base.color),
            })
    } else if uncached_days > 0 {
        text(format!("{} days will be downloaded", uncached_days))
            .size(tokens::text::TITLE)
    } else {
        text(format!("{} days total", total_days)).size(tokens::text::TITLE)
    };

    let mut confirm_items = column![
        text("Confirm Download").size(tokens::text::HEADING),
        space::vertical().height(Length::Fixed(12.0)),
        text(format!("{} - {}", symbol, name)).size(tokens::text::TITLE),
        text(format!("Schema: {}", schema_name)).size(tokens::text::LABEL),
        text(format!(
            "Date Range: {} to {}",
            start_date.format("%b %d, %Y"),
            end_date.format("%b %d, %Y")
        ))
        .size(tokens::text::LABEL),
        space::vertical().height(Length::Fixed(8.0)),
        text(format!(
            "{} days total ({} cached, {} to download)",
            total_days, cached_days, uncached_days
        ))
        .size(tokens::text::BODY),
    ]
    .spacing(tokens::spacing::SM)
    .align_x(Alignment::Center);

    if let Some(label) = cost_label {
        confirm_items = confirm_items.push(
            text(label)
                .size(tokens::text::BODY)
                .style(|theme: &iced::Theme| iced::widget::text::Style {
                    color: Some(
                        theme.extended_palette().primary.base.color,
                    ),
                }),
        );
    }

    confirm_items = confirm_items
        .push(space::vertical().height(Length::Fixed(12.0)))
        .push(status_text)
        .push(space::vertical().height(Length::Fixed(16.0)));

    let confirm_content = container(
        confirm_items
            .push(
                row![
                    button(
                        text("Cancel")
                            .size(tokens::text::LABEL)
                            .align_x(Alignment::Center)
                    )
                    .on_press(on_cancel.clone())
                    .width(Length::Fill)
                    .padding([tokens::spacing::MD, tokens::spacing::XL])
                    .style(style::button::secondary),
                    button(
                        text("Confirm")
                            .size(tokens::text::LABEL)
                            .align_x(Alignment::Center)
                    )
                    .on_press(on_confirm)
                    .width(Length::Fill)
                    .padding([tokens::spacing::MD, tokens::spacing::XL])
                    .style(style::button::primary),
                ]
                .spacing(tokens::spacing::MD),
            )
            .padding(tokens::spacing::XXL),
    )
    .width(Length::Fixed(tokens::layout::CONFIRM_DIALOG_WIDTH))
    .style(style::confirm_modal);

    stack![
        base,
        opaque(
            container(mouse_area(center(opaque(confirm_content))).on_press(on_cancel))
                .width(Length::Fill)
                .height(Length::Fill)
                .style(|_theme| {
                    container::Style {
                        background: Some(
                            Color {
                                a: crate::style::tokens::alpha::BACKDROP,
                                ..Color::BLACK
                            }
                            .into(),
                        ),
                        ..container::Style::default()
                    }
                })
        )
    ]
    .into()
}
