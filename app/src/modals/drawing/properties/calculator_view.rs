//! Position calculator tab views for the drawing properties modal.
//!
//! Contains: quantity section, take profit, stop loss, labels, and
//! display options for Buy/Sell calculator drawing tools.

use data::CalcMode;
use iced::{
    Alignment, Element, Length,
    widget::{
        button, column, container, pick_list, row, text, text_input,
    },
};

use crate::components::form::form_section::FormSectionBuilder;
use crate::components::input::slider_field::SliderFieldBuilder;
use crate::components::input::stepper::StepperBuilder;
use crate::style::{self, tokens};

use super::helpers::{color_swatch, hex_text_input, option_row};
use super::{DrawingPropertiesModal, Message, PickerKind};

impl DrawingPropertiesModal {
    /// Quantity stepper + contract info.
    pub(super) fn quantity_section(&self) -> Element<'_, Message> {
        let calc = self
            .position_calc
            .as_ref()
            .cloned()
            .unwrap_or_default();

        let stepper: Element<'_, Message> = StepperBuilder::new(
            calc.quantity,
            1u32,
            999u32,
            1u32,
            Message::CalcQuantityChanged,
        )
        .label("Quantity")
        .into();

        let mut section = FormSectionBuilder::new("Position").push(stepper);

        if let Some(info) = self.ticker_info {
            let tick_value = info.tick_size * info.contract_size;
            let info_text: Element<'_, Message> = text(format!(
                "Tick: {} | Value: ${:.2} | Size: {}",
                info.tick_size, tick_value, info.contract_size as u32
            ))
            .size(tokens::text::SMALL)
            .style(|theme: &iced::Theme| {
                let palette = theme.extended_palette();
                text::Style {
                    color: Some(palette.background.weak.text),
                }
            })
            .into();
            section = section.push(info_text);
        }

        section.into()
    }

    /// Take Profit -- mode dropdown, value input, color, opacity.
    pub(super) fn take_profit_section(&self) -> Element<'_, Message> {
        let calc = self
            .position_calc
            .as_ref()
            .cloned()
            .unwrap_or_default();

        let mut section = FormSectionBuilder::new("Take Profit");

        let mut mode_row = row![]
            .spacing(tokens::spacing::MD)
            .align_y(Alignment::Center);

        mode_row = mode_row.push(text("Mode").size(tokens::text::LABEL));
        mode_row = mode_row.push(
            pick_list(
                CalcMode::ALL.to_vec(),
                Some(calc.target_mode),
                Message::CalcTargetModeChanged,
            )
            .width(90),
        );

        if calc.target_mode != CalcMode::Free {
            mode_row = mode_row.push(
                text_input("0", &format_calc_value(calc.target_value))
                    .on_input(Message::CalcTargetValueChanged)
                    .width(80),
            );
            let unit = match calc.target_mode {
                CalcMode::Ticks => "ticks",
                CalcMode::Money => "$",
                CalcMode::Free => "",
            };
            mode_row = mode_row.push(text(unit).size(tokens::text::SMALL));
        }

        let mode_row_el: Element<'_, Message> = mode_row.into();
        section = section.push(mode_row_el);

        let color_row = self.risk_level_color_row(
            calc.target_color,
            &self.hex_input_target,
            PickerKind::TpColor,
            Message::ToggleTargetColorPicker,
            Message::CalcTargetHexInput,
        );
        section = section.push(color_row);

        section = section.push(
            SliderFieldBuilder::new(
                "Opacity",
                0.0..=1.0f32,
                calc.target_opacity,
                Message::CalcTargetOpacityChanged,
            )
            .step(0.05)
            .format(|v| format!("{:.0}%", v * 100.0)),
        );

        section.into()
    }

    /// Stop Loss -- mode dropdown, value input, color, opacity.
    pub(super) fn stop_loss_section(&self) -> Element<'_, Message> {
        let calc = self
            .position_calc
            .as_ref()
            .cloned()
            .unwrap_or_default();

        let mut section = FormSectionBuilder::new("Stop Loss");

        let mut mode_row = row![]
            .spacing(tokens::spacing::MD)
            .align_y(Alignment::Center);

        mode_row = mode_row.push(text("Mode").size(tokens::text::LABEL));
        mode_row = mode_row.push(
            pick_list(
                CalcMode::ALL.to_vec(),
                Some(calc.stop_mode),
                Message::CalcStopModeChanged,
            )
            .width(90),
        );

        if calc.stop_mode != CalcMode::Free {
            mode_row = mode_row.push(
                text_input("0", &format_calc_value(calc.stop_value))
                    .on_input(Message::CalcStopValueChanged)
                    .width(80),
            );
            let unit = match calc.stop_mode {
                CalcMode::Ticks => "ticks",
                CalcMode::Money => "$",
                CalcMode::Free => "",
            };
            mode_row = mode_row.push(text(unit).size(tokens::text::SMALL));
        }

        let mode_row_el: Element<'_, Message> = mode_row.into();
        section = section.push(mode_row_el);

        let color_row = self.risk_level_color_row(
            calc.stop_color,
            &self.hex_input_stop,
            PickerKind::SlColor,
            Message::ToggleStopColorPicker,
            Message::CalcStopHexInput,
        );
        section = section.push(color_row);

        section = section.push(
            SliderFieldBuilder::new(
                "Opacity",
                0.0..=1.0f32,
                calc.stop_opacity,
                Message::CalcStopOpacityChanged,
            )
            .step(0.05)
            .format(|v| format!("{:.0}%", v * 100.0)),
        );

        section.into()
    }

    /// Shared color row for risk-level sections (take profit / stop loss).
    pub(super) fn risk_level_color_row(
        &self,
        color: data::SerializableColor,
        hex_input: &Option<String>,
        kind: PickerKind,
        toggle_msg: Message,
        hex_msg: impl Fn(String) -> Message + 'static,
    ) -> Element<'_, Message> {
        let iced_color = crate::style::theme::rgba_to_iced_color(color);
        let hex = hex_input
            .as_deref()
            .unwrap_or(
                data::config::theme::rgba_to_hex_string(color).as_str(),
            )
            .to_string();
        let is_hex_valid = hex_input.is_none()
            || hex_input
                .as_deref()
                .and_then(data::config::theme::hex_to_rgba_safe)
                .is_some();
        let picker_open = self.active_picker.as_ref() == Some(&kind);

        row![
            text("Color").size(tokens::text::LABEL),
            color_swatch(iced_color, picker_open, toggle_msg),
            hex_text_input(&hex, is_hex_valid, hex_msg),
        ]
        .spacing(tokens::spacing::MD)
        .align_y(Alignment::Center)
        .into()
    }

    /// Calculator label toggles + font size.
    pub(super) fn calc_labels_section(&self) -> Element<'_, Message> {
        let calc = self
            .position_calc
            .as_ref()
            .cloned()
            .unwrap_or_default();

        let label_toggles: Element<'_, Message> = column![
            row![
                container(
                    iced::widget::checkbox(calc.show_target_label)
                        .label("Target")
                        .on_toggle(Message::CalcShowTargetLabelToggled)
                )
                .width(Length::FillPortion(1)),
                container(
                    iced::widget::checkbox(calc.show_entry_label)
                        .label("Entry")
                        .on_toggle(Message::CalcShowEntryLabelToggled)
                )
                .width(Length::FillPortion(1)),
            ]
            .spacing(tokens::spacing::LG),
            row![
                container(
                    iced::widget::checkbox(calc.show_stop_label)
                        .label("Stop")
                        .on_toggle(Message::CalcShowStopLabelToggled)
                )
                .width(Length::FillPortion(1)),
                container(
                    iced::widget::checkbox(calc.show_pnl)
                        .label("P&L")
                        .on_toggle(Message::CalcShowPnlToggled)
                )
                .width(Length::FillPortion(1)),
            ]
            .spacing(tokens::spacing::LG),
            iced::widget::checkbox(calc.show_ticks)
                .label("Ticks")
                .on_toggle(Message::CalcShowTicksToggled),
        ]
        .spacing(tokens::spacing::SM)
        .into();

        let font_slider = SliderFieldBuilder::new(
            "Font Size",
            8.0..=16.0f32,
            calc.label_font_size,
            Message::CalcLabelFontSizeChanged,
        )
        .step(1.0)
        .format(|v| format!("{v:.0}px"));

        FormSectionBuilder::new("Labels")
            .push(label_toggles)
            .push(font_slider)
            .into()
    }

    /// Display options for calculator (visibility, reset).
    pub(super) fn display_options_section(&self) -> Element<'_, Message> {
        let mut section = FormSectionBuilder::new("Display");

        section = section.push(option_row(
            "Show Labels",
            iced::widget::checkbox(self.show_labels)
                .on_toggle(Message::ShowLabelsToggled),
        ));

        section = section.push(option_row(
            "Visible",
            iced::widget::checkbox(self.visible)
                .on_toggle(Message::VisibleToggled),
        ));

        let reset_btn: Element<'_, Message> = button(
            text("Reset Colors to Default").size(tokens::text::BODY),
        )
        .on_press(Message::CalcResetColorsToDefault)
        .padding([tokens::spacing::SM, tokens::spacing::XL])
        .style(style::button::secondary)
        .into();

        section = section.push(reset_btn);
        section.into()
    }
}

/// Format a calc value for display, removing trailing zeros.
pub(super) fn format_calc_value(v: f64) -> String {
    if v == v.floor() {
        format!("{:.0}", v)
    } else {
        format!("{}", v)
    }
}
