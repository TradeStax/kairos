//! View methods for the drawing properties modal.

use data::SerializableColor;
use iced::{
    Alignment, Element, Length,
    widget::{
        button, center, column, container, mouse_area, opaque, pick_list, row, space, stack, text,
        text_input,
    },
};

use crate::components::form::form_section::FormSectionBuilder;
use crate::components::input::checkbox_field::CheckboxFieldBuilder;
use crate::components::input::slider_field::SliderFieldBuilder;
use crate::components::primitives::icon_button::icon_button;
use crate::components::primitives::icons::Icon;
use crate::style::{self, tokens};

use super::helpers::{color_swatch, hex_text_input, option_row, picker_popup};
use super::*;

impl DrawingPropertiesModal {
    pub fn view(&self) -> Element<'_, Message> {
        let header = self.header();

        let mut body = column![].spacing(tokens::spacing::LG);
        body = body.push(self.appearance_section());

        if self.has_fill() {
            body = body.push(self.fill_section());
        }

        if self.has_text() {
            body = body.push(self.text_section());
        }

        if self.has_fibonacci() {
            body = body.push(self.fibonacci_section());
        }

        body = body.push(self.options_section());

        let footer = self.footer();

        let inner = column![
            header,
            iced::widget::scrollable(body).style(style::scroll_bar),
            footer,
        ]
        .spacing(tokens::spacing::LG)
        .width(Length::Fill);

        // Color picker popup overlay
        let content: Element<'_, Message> = if self.show_stroke_picker || self.show_fill_picker {
            let popup = self.color_picker_popup();
            stack![
                mouse_area(inner).on_press(Message::DismissColorPicker),
                center(opaque(popup)),
            ]
            .into()
        } else {
            inner.into()
        };

        container(content)
            .padding(tokens::spacing::XL)
            .max_width(440.0)
            .max_height(560.0)
            .style(style::dashboard_modal)
            .into()
    }

    /// Title bar with drawing name, lock toggle, and close button.
    fn header(&self) -> Element<'_, Message> {
        let lock_icon = if self.locked {
            Icon::Locked
        } else {
            Icon::Unlocked
        };
        row![
            text(format!("{} Properties", self.tool)).size(tokens::text::HEADING),
            space::horizontal(),
            icon_button(lock_icon)
                .size(14)
                .padding(tokens::spacing::XS)
                .on_press(Message::LockedToggled(!self.locked)),
            icon_button(Icon::Close)
                .size(14)
                .padding(tokens::spacing::XS)
                .on_press(Message::Close),
        ]
        .spacing(tokens::spacing::XS)
        .align_y(Alignment::Center)
        .width(Length::Fill)
        .into()
    }

    /// Color swatch + hex + style dropdown, width slider.
    fn appearance_section(&self) -> Element<'_, Message> {
        let stroke_iced: iced::Color =
            crate::style::theme_bridge::rgba_to_iced_color(self.stroke_color);
        let hex_stroke = self
            .hex_input_stroke
            .as_deref()
            .unwrap_or(data::config::theme::rgba_to_hex_string(self.stroke_color).as_str())
            .to_string();
        let is_hex_valid = self.hex_input_stroke.is_none()
            || self
                .hex_input_stroke
                .as_deref()
                .and_then(data::config::theme::hex_to_rgba_safe)
                .is_some();

        // Color swatch + hex input + style dropdown in one row
        let color_style_row: Element<'_, Message> = row![
            text("Color").size(tokens::text::LABEL),
            color_swatch(
                stroke_iced,
                self.show_stroke_picker,
                Message::ToggleStrokePicker,
            ),
            hex_text_input(&hex_stroke, is_hex_valid, Message::StrokeHexInput,),
            space::horizontal(),
            text("Style").size(tokens::text::LABEL),
            pick_list(
                LineStyle::ALL.to_vec(),
                Some(self.line_style),
                Message::LineStyleChanged,
            )
            .width(100),
        ]
        .spacing(tokens::spacing::MD)
        .align_y(Alignment::Center)
        .into();

        let width_slider = SliderFieldBuilder::new(
            "Width",
            0.5..=5.0,
            self.stroke_width,
            Message::StrokeWidthChanged,
        )
        .step(0.5)
        .format(|v| format!("{v:.1}px"));

        FormSectionBuilder::new("Appearance")
            .push(color_style_row)
            .push(width_slider)
            .into()
    }

    /// Fill toggle, color swatch, opacity (shapes only).
    fn fill_section(&self) -> Element<'_, Message> {
        let fill_enabled = self.fill_color.is_some();

        let mut section = FormSectionBuilder::new("Fill").with_top_divider(true);

        if !fill_enabled {
            section = section.push(CheckboxFieldBuilder::new(
                "Enable Fill",
                false,
                Message::FillEnabled,
            ));
        } else {
            let fill_c = self
                .fill_color
                .unwrap_or(SerializableColor::new(0.3, 0.6, 1.0, 1.0));
            let fill_iced: iced::Color = crate::style::theme_bridge::rgba_to_iced_color(fill_c);
            let hex_fill = self
                .hex_input_fill
                .as_deref()
                .unwrap_or(data::config::theme::rgba_to_hex_string(fill_c).as_str())
                .to_string();
            let is_hex_valid = self.hex_input_fill.is_none()
                || self
                    .hex_input_fill
                    .as_deref()
                    .and_then(data::config::theme::hex_to_rgba_safe)
                    .is_some();

            // Enable toggle + color swatch + hex
            let fill_row: Element<'_, Message> = row![
                iced::widget::checkbox(fill_enabled)
                    .label("Fill")
                    .on_toggle(Message::FillEnabled),
                space::horizontal(),
                color_swatch(fill_iced, self.show_fill_picker, Message::ToggleFillPicker,),
                hex_text_input(&hex_fill, is_hex_valid, Message::FillHexInput,),
            ]
            .spacing(tokens::spacing::MD)
            .align_y(Alignment::Center)
            .into();

            section = section.push(fill_row);

            section = section.push(
                SliderFieldBuilder::new(
                    "Opacity",
                    0.0..=1.0f32,
                    self.fill_opacity,
                    Message::FillOpacityChanged,
                )
                .step(0.05)
                .format(|v| format!("{:.0}%", v * 100.0)),
            );
        }

        section.into()
    }

    fn text_section(&self) -> Element<'_, Message> {
        let current_text = self.text.as_deref().unwrap_or("");

        FormSectionBuilder::new("Text")
            .with_top_divider(true)
            .push(
                text_input("Enter text...", current_text)
                    .on_input(Message::TextChanged)
                    .width(Length::Fill),
            )
            .into()
    }

    /// Fibonacci options + two-column level grid.
    fn fibonacci_section(&self) -> Element<'_, Message> {
        let fib = self.fibonacci.as_ref().cloned().unwrap_or_default();

        // Two-column option toggles
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
        let mid = (levels.len() + 1) / 2;
        let mut left_col = column![].spacing(tokens::spacing::XS);
        let mut right_col = column![].spacing(tokens::spacing::XS);

        for (idx, level) in levels.iter().enumerate() {
            let level_color: iced::Color =
                crate::style::theme_bridge::rgba_to_iced_color(level.color);
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
            .with_top_divider(true)
            .push(options_row)
            .push(extend_row)
            .push(levels_grid)
            .into()
    }

    /// Options list -- label on left, control on right.
    fn options_section(&self) -> Element<'_, Message> {
        let mut section = FormSectionBuilder::new("Options");

        if self.has_labels() {
            section = section.push(option_row(
                "Show Labels",
                iced::widget::checkbox(self.show_labels).on_toggle(Message::ShowLabelsToggled),
            ));
        }

        section = section.push(option_row(
            "Visible",
            iced::widget::checkbox(self.visible).on_toggle(Message::VisibleToggled),
        ));

        if self.has_label_input() {
            // Two-column label row: text input + alignment dropdown
            let label_value = self.label.as_deref().unwrap_or("");
            let label_row: Element<'_, Message> = row![
                text("Label").size(tokens::text::BODY).width(50),
                text_input("Optional label...", label_value)
                    .on_input(Message::LabelChanged)
                    .width(Length::Fill),
                pick_list(
                    LabelAlignment::ALL.to_vec(),
                    Some(self.label_alignment),
                    Message::LabelAlignmentChanged,
                )
                .width(80),
            ]
            .spacing(tokens::spacing::MD)
            .align_y(Alignment::Center)
            .width(Length::Fill)
            .into();

            section = section.push(label_row);
        }

        section.into()
    }

    fn footer(&self) -> Element<'_, Message> {
        row![
            space::horizontal(),
            button(text("Cancel").size(tokens::text::BODY))
                .on_press(Message::Close)
                .padding([tokens::spacing::SM, tokens::spacing::XL])
                .style(style::button::secondary),
            button(text("Apply").size(tokens::text::BODY))
                .on_press(Message::Apply)
                .padding([tokens::spacing::SM, tokens::spacing::XL])
                .style(style::button::primary),
        ]
        .spacing(tokens::spacing::MD)
        .align_y(Alignment::Center)
        .into()
    }

    /// Floating color picker popup overlay.
    fn color_picker_popup(&self) -> Element<'_, Message> {
        if self.show_stroke_picker {
            self.stroke_picker_popup()
        } else {
            self.fill_picker_popup()
        }
    }

    fn stroke_picker_popup(&self) -> Element<'_, Message> {
        let hsva = self.editing_stroke_color.unwrap_or_else(|| {
            data::config::theme::rgba_to_hsva(self.stroke_color)
        });

        picker_popup(hsva, Message::StrokeColorChanged)
    }

    fn fill_picker_popup(&self) -> Element<'_, Message> {
        let fill_c = self
            .fill_color
            .unwrap_or(SerializableColor::new(0.3, 0.6, 1.0, 1.0));
        let hsva = self.editing_fill_color.unwrap_or_else(|| {
            data::config::theme::rgba_to_hsva(fill_c)
        });

        picker_popup(hsva, Message::FillColorChanged)
    }
}
