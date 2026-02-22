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
use crate::components::layout::modal_header::ModalHeaderBuilder;
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
        let header = ModalHeaderBuilder::new(format!(
            "{} Properties",
            self.tool
        ))
        .push_control(
            icon_button(lock_icon)
                .size(14)
                .padding(tokens::spacing::XS)
                .on_press(Message::LockedToggled(!self.locked)),
        )
        .on_close(Message::Close);

        let tabs = self.available_tabs();
        let tab_bar = self.tab_bar(&tabs);
        let tab_content = self.tab_content();
        let footer = self.footer();

        let body = column![tab_bar, tab_content, footer,]
            .spacing(tokens::spacing::LG)
            .width(Length::Fill);

        let body_scrollable =
            iced::widget::scrollable(body).style(style::scroll_bar);

        let inner = column![
            header,
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

    // ── VBP tab content (data-driven) ───────────────────────────────

    /// Render a VBP tab's content with full section grouping, all 6
    /// parameter kinds (Boolean, Choice, Float, Integer, Color,
    /// LineStyle), and conditional visibility.
    fn vbp_tab_content(
        &self,
        tab: study::ParameterTab,
    ) -> Element<'_, Message> {
        let Some(ref config) = self.vbp_config else {
            return text("No VBP config").into();
        };
        let Some(ref params) = self.vbp_params else {
            return text("No VBP params").into();
        };

        // Collect visible params for this tab
        let visible: Vec<&study::ParameterDef> = params
            .iter()
            .filter(|p| p.tab == tab)
            .filter(|p| {
                !super::HIDDEN_KEYS
                    .contains(&p.key.as_str())
            })
            .filter(|p| p.visible_when.is_visible(config))
            .collect();

        if visible.is_empty() {
            return container(
                text("No configurable parameters")
                    .size(tokens::text::BODY)
                    .style(|theme: &iced::Theme| text::Style {
                        color: Some(
                            theme
                                .extended_palette()
                                .background
                                .weak
                                .text,
                        ),
                    }),
            )
            .padding(tokens::spacing::LG)
            .into();
        }

        // Group by section, preserving order
        let mut section_order: Vec<
            Option<&study::config::ParameterSection>,
        > = Vec::new();
        for p in &visible {
            let key = p.section.as_ref().map(|s| s.label);
            if !section_order.iter().any(|existing| {
                existing.map(|s| s.label) == key
            }) {
                section_order.push(p.section.as_ref());
            }
        }
        section_order.sort_by_key(|s| match s {
            Some(sec) => (0, sec.order, ""),
            None => (1, 0, ""),
        });

        let tab_label = vbp_tab_default_label(tab);

        let mut content_col: Vec<Element<'_, Message>> =
            Vec::new();
        for section_def in &section_order {
            let section_label =
                section_def.map(|s| s.label);
            let mut section_params: Vec<&study::ParameterDef> =
                visible
                    .iter()
                    .filter(|p| {
                        p.section.as_ref().map(|s| s.label)
                            == section_label
                    })
                    .copied()
                    .collect();
            section_params.sort_by_key(|p| p.order);

            let title =
                section_label.unwrap_or(tab_label);
            let mut form =
                FormSectionBuilder::new(title)
                    .spacing(tokens::spacing::LG);

            for param in section_params {
                let widget =
                    self.vbp_param_widget(param, config);
                form = form.push(widget);
            }

            content_col.push(form.into());
        }

        column(content_col)
            .spacing(
                if section_order.len() > 1 {
                    tokens::spacing::XL
                } else {
                    0.0
                },
            )
            .into()
    }

    /// Render a single VBP parameter as a widget.
    fn vbp_param_widget<'a>(
        &'a self,
        param: &'a study::ParameterDef,
        config: &study::StudyConfig,
    ) -> Element<'a, Message> {
        use study::config::ParameterKind;

        let key = param.key.to_string();

        match &param.kind {
            ParameterKind::Integer { min, max } => {
                let current = config.get_int(
                    &param.key,
                    match &param.default {
                        study::ParameterValue::Integer(v) => *v,
                        _ => *min,
                    },
                );
                let min_f = *min as f32;
                let max_f = *max as f32;
                let current_f = current as f32;
                let fmt = param.format;
                // Skip large ranges (e.g. timestamps)
                if max_f > 10000.0 {
                    return space::horizontal().into();
                }
                let key = key.clone();
                SliderFieldBuilder::new(
                    &param.label,
                    min_f..=max_f,
                    current_f,
                    move |v| {
                        Message::VbpParamChanged(
                            key.clone(),
                            study::ParameterValue::Integer(
                                v as i64,
                            ),
                        )
                    },
                )
                .step(1.0)
                .format(move |v| {
                    format_integer(*v as i64, fmt)
                })
                .into()
            }
            ParameterKind::Float {
                min, max, step, ..
            } => {
                let current = config.get_float(
                    &param.key,
                    match &param.default {
                        study::ParameterValue::Float(v) => *v,
                        _ => *min,
                    },
                );
                let min_f = *min as f32;
                let max_f = *max as f32;
                let step_f = *step as f32;
                let current_f = current as f32;
                let fmt = param.format;
                let key = key.clone();
                SliderFieldBuilder::new(
                    &param.label,
                    min_f..=max_f,
                    current_f,
                    move |v| {
                        Message::VbpParamChanged(
                            key.clone(),
                            study::ParameterValue::Float(
                                v as f64,
                            ),
                        )
                    },
                )
                .step(step_f)
                .format(move |v| format_float(*v, fmt))
                .into()
            }
            ParameterKind::Color => {
                let current = config.get_color(
                    &param.key,
                    match &param.default {
                        study::ParameterValue::Color(c) => *c,
                        _ => data::SerializableColor::new(
                            1.0, 1.0, 1.0, 1.0,
                        ),
                    },
                );
                let iced_color =
                    crate::style::theme_bridge::rgba_to_iced_color(
                        current,
                    );
                let is_editing =
                    self.editing_vbp_color_key.as_deref()
                        == Some(param.key.as_str());
                let key_for_press = key.clone();
                let key_for_picker = key.clone();

                let swatch = button(
                    space::horizontal().width(22).height(22),
                )
                .style(
                    move |_theme, _status| button::Style {
                        background: Some(iced_color.into()),
                        border: iced::border::rounded(3)
                            .width(if is_editing {
                                2.0
                            } else {
                                1.0
                            })
                            .color(if is_editing {
                                iced::Color::WHITE
                            } else {
                                iced::Color::from_rgba(
                                    1.0, 1.0, 1.0, 0.3,
                                )
                            }),
                        ..button::Style::default()
                    },
                )
                .padding(0)
                .on_press(Message::VbpEditColor(
                    key_for_press,
                ));

                let mut col = column![
                    row![
                        text(&param.label)
                            .size(tokens::text::BODY),
                        space::horizontal(),
                        swatch,
                    ]
                    .align_y(Alignment::Center)
                    .width(Length::Fill),
                ]
                .spacing(tokens::spacing::SM);

                if is_editing {
                    let hsva = self
                        .editing_vbp_color_hsva
                        .unwrap_or_else(|| {
                            data::config::theme::rgba_to_hsva(
                                crate::style::theme_bridge::iced_color_to_rgba(
                                    iced_color,
                                ),
                            )
                        });
                    col = col.push(
                        container(
                            crate::components::input::color_picker::color_picker(
                                hsva,
                                move |h| {
                                    Message::VbpColorChanged(
                                        key_for_picker.clone(),
                                        h,
                                    )
                                },
                                180.0,
                            ),
                        )
                        .padding(tokens::spacing::SM)
                        .style(style::dropdown_container),
                    );
                }

                col.into()
            }
            ParameterKind::Boolean => {
                let current = config.get_bool(
                    &param.key,
                    match &param.default {
                        study::ParameterValue::Boolean(v) => {
                            *v
                        }
                        _ => false,
                    },
                );
                let key = key.clone();
                crate::components::input::toggle_switch::toggle_switch(
                    &param.label,
                    current,
                    move |v| {
                        Message::VbpParamChanged(
                            key.clone(),
                            study::ParameterValue::Boolean(v),
                        )
                    },
                )
            }
            ParameterKind::Choice { options } => {
                let current = config
                    .get_choice(
                        &param.key,
                        match &param.default {
                            study::ParameterValue::Choice(
                                s,
                            ) => s.as_str(),
                            _ => {
                                options.first().unwrap_or(&"")
                            }
                        },
                    )
                    .to_string();
                let options_vec: Vec<String> = options
                    .iter()
                    .map(|s| s.to_string())
                    .collect();
                let selected = options_vec
                    .iter()
                    .find(|o| **o == current)
                    .cloned();
                let key = key.clone();
                row![
                    text(&param.label)
                        .size(tokens::text::BODY),
                    space::horizontal(),
                    pick_list(
                        options_vec,
                        selected,
                        move |v: String| {
                            Message::VbpParamChanged(
                                key.clone(),
                                study::ParameterValue::Choice(
                                    v,
                                ),
                            )
                        },
                    )
                    .width(140),
                ]
                .align_y(Alignment::Center)
                .width(Length::Fill)
                .into()
            }
            ParameterKind::LineStyle => {
                let current = config.get_line_style(
                    &param.key,
                    match &param.default {
                        study::ParameterValue::LineStyle(v) => {
                            *v
                        }
                        _ => {
                            study::config::LineStyleValue::Solid
                        }
                    },
                );
                let options = vec![
                    study::config::LineStyleValue::Solid,
                    study::config::LineStyleValue::Dashed,
                    study::config::LineStyleValue::Dotted,
                ];
                let key = key.clone();
                row![
                    text(&param.label)
                        .size(tokens::text::BODY),
                    space::horizontal(),
                    pick_list(options, Some(current), move |v| {
                        Message::VbpLineStyleChanged(
                            key.clone(),
                            v,
                        )
                    })
                    .width(120),
                ]
                .align_y(Alignment::Center)
                .width(Length::Fill)
                .into()
            }
        }
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
        let mid = levels.len().div_ceil(2);
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
        Tab::Vbp(param_tab) => vbp_tab_default_label(param_tab),
    }
}

/// Default display label for a VBP `ParameterTab`.
fn vbp_tab_default_label(
    tab: study::ParameterTab,
) -> &'static str {
    // VBP study provides custom labels via tab_labels(); these are
    // the fallback defaults matching the study's static LABELS.
    match tab {
        study::ParameterTab::Parameters => "Data",
        study::ParameterTab::Style => "Style",
        study::ParameterTab::Tab4 => "POC",
        study::ParameterTab::Tab5 => "Value Area",
        study::ParameterTab::Tab6 => "Peak & Valley",
        study::ParameterTab::Tab7 => "VWAP",
        study::ParameterTab::Display => "Display",
    }
}

/// Format an integer value according to `DisplayFormat`.
fn format_integer(
    v: i64,
    fmt: study::DisplayFormat,
) -> String {
    match fmt {
        study::DisplayFormat::Integer { suffix } => {
            format!("{v}{suffix}")
        }
        study::DisplayFormat::IntegerOrNone { none_value } => {
            if v == none_value {
                "None".to_string()
            } else {
                format!("{v}")
            }
        }
        study::DisplayFormat::Percent => format!("{v}%"),
        study::DisplayFormat::Float { decimals } => {
            format!("{v:.prec$}", prec = decimals as usize)
        }
        study::DisplayFormat::Auto => format!("{v}"),
    }
}

/// Format a float value according to `DisplayFormat`.
fn format_float(
    v: f32,
    fmt: study::DisplayFormat,
) -> String {
    match fmt {
        study::DisplayFormat::Percent => {
            format!("{:.0}%", v * 100.0)
        }
        study::DisplayFormat::Float { decimals } => {
            format!("{v:.prec$}", prec = decimals as usize)
        }
        study::DisplayFormat::Integer { suffix } => {
            format!("{}{suffix}", v as i64)
        }
        study::DisplayFormat::IntegerOrNone { none_value } => {
            let i = v as i64;
            if i == none_value {
                "None".to_string()
            } else {
                format!("{i}")
            }
        }
        study::DisplayFormat::Auto => format!("{v:.2}"),
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
