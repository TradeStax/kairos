//! Left panel: selectable level list with filters and manual add/remove.

use iced::{
    Alignment, Element, Length,
    widget::{button, column, container, pick_list, row, rule, scrollable, text_input},
};

use study::orderflow::level_analyzer::types::{LevelSource, MonitoredLevel};

use crate::components;
use crate::components::layout::interactive_card::InteractiveCardBuilder;
use crate::components::primitives;
use crate::style;
use crate::style::tokens;

use super::{
    LevelDetailModal, Message, SessionFilter, SourceFilter, StatusFilter,
    status_color, status_label,
};

impl LevelDetailModal {
    pub(super) fn view_left_panel<'a>(
        &'a self,
        filtered: &[&'a MonitoredLevel],
    ) -> Element<'a, Message> {
        // Filter bar
        let source_options = vec![
            SourceFilter::All,
            SourceFilter::Profile,
            SourceFilter::Session,
            SourceFilter::PriorDay,
            SourceFilter::OpeningRange,
            SourceFilter::Delta,
            SourceFilter::Manual,
        ];

        let status_options = vec![
            StatusFilter::All,
            StatusFilter::Untested,
            StatusFilter::Holding,
            StatusFilter::BeingTested,
            StatusFilter::Weakening,
            StatusFilter::Broken,
        ];

        let session_options = vec![
            SessionFilter::All,
            SessionFilter::CurrentSession,
            SessionFilter::RthOnly,
            SessionFilter::EthOnly,
        ];

        let filter_bar = column![
            pick_list(
                source_options,
                Some(self.source_filter),
                Message::FilterSource,
            )
            .width(Length::Fill)
            .text_size(tokens::text::TINY),
            pick_list(
                status_options,
                Some(self.status_filter),
                Message::FilterStatus,
            )
            .width(Length::Fill)
            .text_size(tokens::text::TINY),
            pick_list(
                session_options,
                Some(self.session_filter.clone()),
                Message::FilterSession,
            )
            .width(Length::Fill)
            .text_size(tokens::text::TINY),
        ]
        .spacing(tokens::spacing::XXS)
        .padding([tokens::spacing::XS, tokens::spacing::SM]);

        // Level items
        let level_list: Element<'_, Message> = if filtered.is_empty() {
            container(primitives::small("No levels match filters"))
                .padding(tokens::spacing::LG)
                .width(Length::Fill)
                .align_x(Alignment::Center)
                .into()
        } else {
            let items = filtered.iter().enumerate().fold(
                column![].spacing(tokens::spacing::XXXS),
                |col, (idx, level)| col.push(level_item(level, idx, self.selected_index)),
            );
            scrollable(items.padding([tokens::spacing::XXS, 0.0]))
                .height(Length::Fill)
                .into()
        };

        // Footer: manual price input + add/remove buttons
        let mut footer_row = row![
            text_input("Price...", &self.manual_price_input)
                .on_input(Message::ManualPriceChanged)
                .on_submit(Message::AddManualLevel)
                .size(tokens::text::SMALL)
                .width(Length::Fill),
            button(primitives::small("+ Add"))
                .on_press(Message::AddManualLevel)
                .padding([tokens::spacing::XXS, tokens::spacing::SM]),
        ]
        .spacing(tokens::spacing::XS)
        .align_y(Alignment::Center);

        // Only show Remove for selected Manual levels
        if let Some(idx) = self.selected_index {
            let levels = self.filtered_sorted_levels();
            if let Some(level) = levels.get(idx) {
                if level.source == LevelSource::Manual {
                    footer_row = footer_row.push(
                        button(primitives::small("- Remove"))
                            .on_press(Message::RemoveSelected)
                            .padding([tokens::spacing::XXS, tokens::spacing::SM])
                            .style(style::button::secondary),
                    );
                }
            }
        }

        let footer = container(footer_row)
            .padding([tokens::spacing::SM, tokens::spacing::SM]);

        column![
            filter_bar,
            level_list,
            rule::horizontal(1).style(style::split_ruler),
            footer,
        ]
        .width(200)
        .into()
    }
}

/// Render a single level list item.
fn level_item<'a>(
    level: &MonitoredLevel,
    idx: usize,
    selected: Option<usize>,
) -> Element<'a, Message> {
    let is_selected = selected == Some(idx);
    let status = level.status;

    let dot = components::display::status_dot_themed(status_color(status));

    let session_tag = level.session_key.short_tag();
    let label = if session_tag.is_empty() {
        level.source.label().to_string()
    } else {
        format!("{} {}", session_tag, level.source.label())
    };

    let content = row![
        dot,
        primitives::mono(format!("{:.2}", level.price)),
        primitives::tiny(label),
        primitives::tiny(status_label(status)),
    ]
    .spacing(tokens::spacing::XS)
    .align_y(Alignment::Center);

    InteractiveCardBuilder::new(content, Message::SelectLevel(idx))
        .selected(is_selected)
        .accent_bar(true)
        .padding([tokens::spacing::XS, tokens::spacing::SM])
        .width(Length::Fill)
        .into()
}
