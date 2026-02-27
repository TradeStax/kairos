//! View methods for the drawing properties modal.

use crate::drawing::{DrawingTool, LineStyle};
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
use crate::components::layout::button_group::ButtonGroupBuilder;
use crate::components::overlay::modal_header::ModalHeaderBuilder;
use crate::components::primitives::icon_button::icon_button;
use crate::components::primitives::icons::Icon;
use crate::style::{self, tokens};

use super::helpers::{color_swatch, hex_text_input, option_row, picker_popup};
use super::{PickerKind, Tab, *};

impl DrawingPropertiesModal {
    pub fn view(&self) -> Element<'_, Message> {
        let lock_icon = if self.locked {
            Icon::Locked
        } else {
            Icon::Unlocked
        };
        let header = ModalHeaderBuilder::new(format!("{} Properties", self.tool))
            .push_control(
                icon_button(lock_icon)
                    .size(14.0)
                    .padding(tokens::spacing::XS)
                    .on_press(Message::LockedToggled(!self.locked)),
            )
            .on_close(Message::Close);

        let tabs = self.available_tabs();
        let tab_bar = self.tab_bar(&tabs);
        let tab_content = self.tab_content();
        let footer = self.footer();

        let body = column![tab_content, footer]
            .spacing(tokens::spacing::LG)
            .width(Length::Fill);

        let body_scrollable = iced::widget::scrollable::Scrollable::with_direction(
            body,
            iced::widget::scrollable::Direction::Vertical(
                iced::widget::scrollable::Scrollbar::new()
                    .width(4)
                    .scroller_width(4)
                    .spacing(2),
            ),
        )
        .style(style::scroll_bar);

        let inner = column![
            header,
            tab_bar,
            container(body_scrollable).padding(iced::Padding {
                top: tokens::spacing::MD,
                right: tokens::spacing::XL,
                bottom: tokens::spacing::XL,
                left: tokens::spacing::XL,
            }),
        ]
        .width(Length::Fill);

        let content: Element<'_, Message> = if self.active_picker.is_some() {
            let popup = self.active_color_picker_popup();
            stack![
                mouse_area(inner).on_press(Message::DismissColorPicker),
                center(opaque(popup)),
            ]
            .into()
        } else {
            inner.into()
        };

        container(content)
            .max_width(440.0)
            .max_height(620.0)
            .style(style::dashboard_modal)
            .into()
    }

    /// Tab bar using ButtonGroupBuilder.
    fn tab_bar(&self, tabs: &[Tab]) -> Element<'_, Message> {
        let selected_idx = tabs.iter().position(|t| *t == self.active_tab).unwrap_or(0);

        let items: Vec<(String, Message)> = tabs
            .iter()
            .map(|t| (tab_label(*t).to_string(), Message::SwitchTab(*t)))
            .collect();

        container(
            ButtonGroupBuilder::new(items, selected_idx)
                .tab_style()
                .fill_width()
                .into_element(),
        )
        .padding(iced::Padding {
            top: tokens::spacing::SM,
            right: tokens::spacing::XL,
            bottom: 0.0,
            left: tokens::spacing::XL,
        })
        .into()
    }

    /// Dispatch to the active tab's content.
    fn tab_content(&self) -> Element<'_, Message> {
        match self.active_tab {
            Tab::Style => self.style_tab(),
            Tab::Levels => self.levels_tab(),
            Tab::Position => self.position_tab(),
            Tab::Labels => self.labels_tab(),
            Tab::Display => self.display_tab(),
            Tab::Vbp(param_tab) => self.vbp_tab_content(param_tab),
        }
    }

    // ── Style tab ─────────────────────────────────────────────────────

    fn style_tab(&self) -> Element<'_, Message> {
        let mut body = column![].spacing(tokens::spacing::LG);
        body = body.push(self.stroke_section());

        if self.has_fill() {
            body = body.push(self.fill_section());
        }

        if self.has_text() {
            body = body.push(self.text_section());
        }

        body.into()
    }

    // ── Levels tab (Fibonacci only) ───────────────────────────────────

    fn levels_tab(&self) -> Element<'_, Message> {
        self.fibonacci_section()
    }

    // ── Position tab (Calculator only) ────────────────────────────────

    fn position_tab(&self) -> Element<'_, Message> {
        column![]
            .spacing(tokens::spacing::LG)
            .push(self.quantity_section())
            .push(self.take_profit_section())
            .push(self.stop_loss_section())
            .into()
    }

    // ── Labels tab (Calculator only) ──────────────────────────────────

    fn labels_tab(&self) -> Element<'_, Message> {
        let mut body = column![].spacing(tokens::spacing::LG);
        body = body.push(self.calc_labels_section());
        body = body.push(self.display_options_section());
        body.into()
    }

    // ── Display tab (generic + fibonacci) ─────────────────────────────

    fn display_tab(&self) -> Element<'_, Message> {
        self.options_section()
    }

    // ── Shared sections ───────────────────────────────────────────────

    /// Stroke color + hex + line style + width slider.
    fn stroke_section(&self) -> Element<'_, Message> {
        let color_style_row = self.stroke_color_row();

        let width_slider = SliderFieldBuilder::new(
            "Width",
            0.5..=5.0,
            self.stroke_width,
            Message::StrokeWidthChanged,
        )
        .step(0.5)
        .format(|v| format!("{v:.1}px"));

        let title = if self.has_position_calc() {
            "Entry Line"
        } else {
            "Appearance"
        };

        let mut section = FormSectionBuilder::new(title)
            .push(color_style_row)
            .push(width_slider);

        if self.tool == DrawingTool::Arrow {
            let arrowhead_row: Element<'_, Message> = row![
                container(CheckboxFieldBuilder::new(
                    "Head at Start",
                    self.arrow_head_start,
                    Message::ArrowHeadStartToggled,
                ))
                .width(Length::FillPortion(1)),
                container(CheckboxFieldBuilder::new(
                    "Head at End",
                    self.arrow_head_end,
                    Message::ArrowHeadEndToggled,
                ))
                .width(Length::FillPortion(1)),
            ]
            .spacing(tokens::spacing::LG)
            .into();
            section = section.push(arrowhead_row);
        }

        section.into()
    }

    /// Unified stroke color row: swatch + hex + style dropdown.
    fn stroke_color_row(&self) -> Element<'_, Message> {
        let stroke_iced: iced::Color = crate::style::theme::rgba_to_iced_color(self.stroke_color);
        let hex_stroke = self
            .hex_input_stroke
            .as_deref()
            .unwrap_or(crate::config::theme::rgba_to_hex_string(self.stroke_color).as_str())
            .to_string();
        let is_hex_valid = self.hex_input_stroke.is_none()
            || self
                .hex_input_stroke
                .as_deref()
                .and_then(crate::config::theme::hex_to_rgba_safe)
                .is_some();

        row![
            text("Color").size(tokens::text::LABEL),
            color_swatch(
                stroke_iced,
                self.active_picker == Some(PickerKind::LineColor),
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
        .into()
    }

    /// Fill toggle, color swatch, opacity (shapes only).
    fn fill_section(&self) -> Element<'_, Message> {
        let fill_enabled = self.fill_color.is_some();

        let mut section = FormSectionBuilder::new("Fill");

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
            let fill_iced: iced::Color = crate::style::theme::rgba_to_iced_color(fill_c);
            let hex_fill = self
                .hex_input_fill
                .as_deref()
                .unwrap_or(crate::config::theme::rgba_to_hex_string(fill_c).as_str())
                .to_string();
            let is_hex_valid = self.hex_input_fill.is_none()
                || self
                    .hex_input_fill
                    .as_deref()
                    .and_then(crate::config::theme::hex_to_rgba_safe)
                    .is_some();

            let fill_row: Element<'_, Message> = row![
                iced::widget::checkbox(fill_enabled)
                    .label("Fill")
                    .on_toggle(Message::FillEnabled),
                space::horizontal(),
                color_swatch(
                    fill_iced,
                    self.active_picker == Some(PickerKind::FillColor),
                    Message::ToggleFillPicker,
                ),
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

        let mut section = FormSectionBuilder::new("Text").push(
            text_input("Enter text...", current_text)
                .on_input(Message::TextChanged)
                .width(Length::Fill),
        );

        if self.tool == DrawingTool::TextLabel {
            section = section.push(
                SliderFieldBuilder::new(
                    "Font Size",
                    8.0..=28.0f32,
                    self.text_font_size,
                    Message::TextFontSizeChanged,
                )
                .step(1.0)
                .format(|v| format!("{v:.0}px")),
            );
        }

        section.into()
    }

    /// Options section for generic drawings.
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

    // ── Color picker popups ──────────────────────────────────────────

    fn active_color_picker_popup(&self) -> Element<'_, Message> {
        match self.active_picker {
            Some(PickerKind::LineColor) => self.stroke_picker_popup(),
            Some(PickerKind::FillColor) => self.fill_picker_popup(),
            Some(PickerKind::TpColor) => self.target_color_picker_popup(),
            Some(PickerKind::SlColor) | None => self.stop_color_picker_popup(),
        }
    }

    fn stroke_picker_popup(&self) -> Element<'_, Message> {
        let hsva = self
            .editing_stroke_color
            .unwrap_or_else(|| crate::config::theme::rgba_to_hsva(self.stroke_color));
        picker_popup(hsva, Message::StrokeColorChanged)
    }

    fn fill_picker_popup(&self) -> Element<'_, Message> {
        let fill_c = self
            .fill_color
            .unwrap_or(SerializableColor::new(0.3, 0.6, 1.0, 1.0));
        let hsva = self
            .editing_fill_color
            .unwrap_or_else(|| crate::config::theme::rgba_to_hsva(fill_c));
        picker_popup(hsva, Message::FillColorChanged)
    }

    fn target_color_picker_popup(&self) -> Element<'_, Message> {
        let calc = self.position_calc.as_ref().cloned().unwrap_or_default();
        let hsva = self
            .editing_target_color
            .unwrap_or_else(|| crate::config::theme::rgba_to_hsva(calc.target_color));
        picker_popup(hsva, Message::CalcTargetColorChanged)
    }

    fn stop_color_picker_popup(&self) -> Element<'_, Message> {
        let calc = self.position_calc.as_ref().cloned().unwrap_or_default();
        let hsva = self
            .editing_stop_color
            .unwrap_or_else(|| crate::config::theme::rgba_to_hsva(calc.stop_color));
        picker_popup(hsva, Message::CalcStopColorChanged)
    }
}

/// Display label for each tab.
fn tab_label(tab: Tab) -> &'static str {
    match tab {
        Tab::Style => "Style",
        Tab::Levels => "Levels",
        Tab::Position => "Position",
        Tab::Labels => "Labels",
        Tab::Display => "Display",
        Tab::Vbp(param_tab) => vbp_tab_default_label(param_tab),
    }
}

/// Default display label for a VBP `ParameterTab`.
pub(super) fn vbp_tab_default_label(tab: study::ParameterTab) -> &'static str {
    // VBP study provides custom labels via tab_labels(); these are
    // the fallback defaults matching the study's static LABELS.
    match tab {
        study::ParameterTab::Parameters => "Data",
        study::ParameterTab::Style => "Style",
        study::ParameterTab::PocSettings => "POC",
        study::ParameterTab::ValueArea => "Value Area",
        study::ParameterTab::Nodes => "Peak & Valley",
        study::ParameterTab::Vwap => "VWAP",
        study::ParameterTab::Display => "Display",
        study::ParameterTab::Absorption => "Absorption",
    }
}
