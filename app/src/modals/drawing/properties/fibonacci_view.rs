//! Fibonacci level views for the drawing properties modal.

use iced::{
    Alignment, Element, Length,
    widget::{column, container, row, text},
};

use crate::components::form::form_section::FormSectionBuilder;
use crate::components::input::checkbox_field::CheckboxFieldBuilder;
use crate::style::tokens;

use super::{DrawingPropertiesModal, Message};

impl DrawingPropertiesModal {
    /// Fibonacci options + two-column level grid.
    pub(super) fn fibonacci_section(&self) -> Element<'_, Message> {
        let fib = self.fibonacci.as_ref().cloned().unwrap_or_default();

        let options_row: Element<'_, Message> = row![
            container(CheckboxFieldBuilder::new(
                "Show Prices",
                fib.show_prices,
                Message::FibShowPricesToggled,
            ))
            .width(Length::FillPortion(1)),
            container(CheckboxFieldBuilder::new(
                "Show %",
                fib.show_percentages,
                Message::FibShowPercentagesToggled,
            ))
            .width(Length::FillPortion(1)),
        ]
        .spacing(tokens::spacing::LG)
        .into();

        let extend_row = CheckboxFieldBuilder::new(
            "Extend Lines",
            fib.extend_lines,
            Message::FibExtendLinesToggled,
        );

        // Two-column level grid
        let levels = &fib.levels;
        let mid = levels.len().div_ceil(2);
        let mut left_col = column![].spacing(tokens::spacing::XS);
        let mut right_col = column![].spacing(tokens::spacing::XS);

        for (idx, level) in levels.iter().enumerate() {
            let level_color: iced::Color = crate::style::theme::rgba_to_iced_color(level.color);
            let level_label = level.label.clone();
            let level_visible = level.visible;

            let level_row: Element<'_, Message> = row![
                iced::widget::checkbox(level_visible)
                    .on_toggle(move |v| { Message::FibLevelVisibilityToggled(idx, v) }),
                text(level_label).size(tokens::text::BODY).width(50),
                container(iced::widget::Space::new().width(14).height(14))
                    .style(move |_theme: &iced::Theme| {
                        container::Style {
                            background: Some(level_color.into()),
                            border: iced::Border {
                                radius: tokens::radius::SM.into(),
                                width: tokens::border::THIN,
                                color: iced::Color::WHITE.scale_alpha(0.2),
                            },
                            ..container::Style::default()
                        }
                    })
                    .width(14)
                    .height(14),
            ]
            .spacing(tokens::spacing::SM)
            .align_y(Alignment::Center)
            .into();

            if idx < mid {
                left_col = left_col.push(level_row);
            } else {
                right_col = right_col.push(level_row);
            }
        }

        let levels_grid: Element<'_, Message> = column![
            text("Levels").size(tokens::text::LABEL),
            row![
                left_col.width(Length::FillPortion(1)),
                right_col.width(Length::FillPortion(1)),
            ]
            .spacing(tokens::spacing::MD),
        ]
        .spacing(tokens::spacing::SM)
        .into();

        FormSectionBuilder::new("Fibonacci")
            .push(options_row)
            .push(extend_row)
            .push(levels_grid)
            .into()
    }
}
