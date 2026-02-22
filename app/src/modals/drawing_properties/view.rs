//! View methods for the drawing properties modal.

use data::{CalcMode, LineStyle, SerializableColor};
use iced::{
    Alignment, Element, Length,
    widget::{
        button, center, column, container, mouse_area, opaque, pick_list, row, space,
        stack, text, text_input,
    },
};

use crate::components::form::form_section::FormSectionBuilder;
use crate::components::input::checkbox_field::CheckboxFieldBuilder;
use crate::components::input::slider_field::SliderFieldBuilder;
use crate::components::input::stepper::StepperBuilder;
use crate::components::layout::button_group::ButtonGroupBuilder;
use crate::components::primitives::icon_button::icon_button;
use crate::components::primitives::icons::Icon;
use crate::style::{self, tokens};

use super::helpers::{color_swatch, hex_text_input, option_row, picker_popup};
use super::{PickerKind, Tab, *};

impl DrawingPropertiesModal {
    pub fn view(&self) -> Element<'_, Message> {
        let header = self.header();
        let tabs = self.available_tabs();
        let tab_bar = self.tab_bar(&tabs);
        let tab_content = self.tab_content();
        let footer = self.footer();

        let inner = column![
            header,
            tab_bar,
            iced::widget::scrollable(tab_content).style(style::scroll_bar),
            footer,
        ]
        .spacing(tokens::spacing::LG)
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
            .padding(tokens::spacing::XL)
            .max_width(440.0)
            .max_height(620.0)
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
            text(format!("{} Properties", self.tool))
                .size(tokens::text::HEADING),
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

    /// Tab bar using ButtonGroupBuilder.
    fn tab_bar(&self, tabs: &[Tab]) -> Element<'_, Message> {
        let selected_idx = tabs
            .iter()
            .position(|t| *t == self.active_tab)
            .unwrap_or(0);

        let items: Vec<(String, Message)> = tabs
            .iter()
            .map(|t| (tab_label(*t).to_string(), Message::SwitchTab(*t)))
            .collect();

        ButtonGroupBuilder::new(items, selected_idx)
            .tab_style()
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

        FormSectionBuilder::new(title)
            .push(color_style_row)
            .push(width_slider)
            .into()
    }

    /// Unified stroke color row: swatch + hex + style dropdown.
    fn stroke_color_row(&self) -> Element<'_, Message> {
        let stroke_iced: iced::Color =
            crate::style::theme_bridge::rgba_to_iced_color(self.stroke_color);
        let hex_stroke = self
            .hex_input_stroke
            .as_deref()
            .unwrap_or(
                data::config::theme::rgba_to_hex_string(self.stroke_color)
                    .as_str(),
            )
            .to_string();
        let is_hex_valid = self.hex_input_stroke.is_none()
            || self
                .hex_input_stroke
                .as_deref()
                .and_then(data::config::theme::hex_to_rgba_safe)
                .is_some();

        row![
            text("Color").size(tokens::text::LABEL),
            color_swatch(
                stroke_iced,
                self.active_picker == Some(PickerKind::LineColor),
                Message::ToggleStrokePicker,
            ),
            hex_text_input(
                &hex_stroke,
                is_hex_valid,
                Message::StrokeHexInput,
            ),
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
            let fill_iced: iced::Color =
                crate::style::theme_bridge::rgba_to_iced_color(fill_c);
            let hex_fill = self
                .hex_input_fill
                .as_deref()
                .unwrap_or(
                    data::config::theme::rgba_to_hex_string(fill_c).as_str(),
                )
                .to_string();
            let is_hex_valid = self.hex_input_fill.is_none()
                || self
                    .hex_input_fill
                    .as_deref()
                    .and_then(data::config::theme::hex_to_rgba_safe)
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
                hex_text_input(
                    &hex_fill,
                    is_hex_valid,
                    Message::FillHexInput,
                ),
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
                iced::widget::checkbox(level_visible).on_toggle(move |v| {
                    Message::FibLevelVisibilityToggled(idx, v)
                }),
                text(level_label).size(tokens::text::BODY).width(50),
                container(
                    iced::widget::Space::new().width(14).height(14)
                )
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

    // ── Calculator sections ───────────────────────────────────────────

    /// Quantity stepper + contract info.
    fn quantity_section(&self) -> Element<'_, Message> {
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

    /// Take Profit — mode dropdown, value input, color, opacity.
    fn take_profit_section(&self) -> Element<'_, Message> {
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

    /// Stop Loss — mode dropdown, value input, color, opacity.
    fn stop_loss_section(&self) -> Element<'_, Message> {
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
    fn risk_level_color_row(
        &self,
        color: data::SerializableColor,
        hex_input: &Option<String>,
        kind: PickerKind,
        toggle_msg: Message,
        hex_msg: impl Fn(String) -> Message + 'static,
    ) -> Element<'_, Message> {
        let iced_color = crate::style::theme_bridge::rgba_to_iced_color(color);
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
    fn calc_labels_section(&self) -> Element<'_, Message> {
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
    fn display_options_section(&self) -> Element<'_, Message> {
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

    /// Options section for generic drawings.
    fn options_section(&self) -> Element<'_, Message> {
        let mut section = FormSectionBuilder::new("Options");

        if self.has_labels() {
            section = section.push(option_row(
                "Show Labels",
                iced::widget::checkbox(self.show_labels)
                    .on_toggle(Message::ShowLabelsToggled),
            ));
        }

        section = section.push(option_row(
            "Visible",
            iced::widget::checkbox(self.visible)
                .on_toggle(Message::VisibleToggled),
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

    fn target_color_picker_popup(&self) -> Element<'_, Message> {
        let calc = self
            .position_calc
            .as_ref()
            .cloned()
            .unwrap_or_default();
        let hsva = self.editing_target_color.unwrap_or_else(|| {
            data::config::theme::rgba_to_hsva(calc.target_color)
        });
        picker_popup(hsva, Message::CalcTargetColorChanged)
    }

    fn stop_color_picker_popup(&self) -> Element<'_, Message> {
        let calc = self
            .position_calc
            .as_ref()
            .cloned()
            .unwrap_or_default();
        let hsva = self.editing_stop_color.unwrap_or_else(|| {
            data::config::theme::rgba_to_hsva(calc.stop_color)
        });
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
    }
}

/// Format a calc value for display, removing trailing zeros.
fn format_calc_value(v: f64) -> String {
    if v == v.floor() {
        format!("{:.0}", v)
    } else {
        format!("{}", v)
    }
}
