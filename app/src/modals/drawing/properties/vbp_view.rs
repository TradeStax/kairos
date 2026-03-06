//! VBP (Volume-by-Price) tab content for the drawing properties modal.

use data::SerializableColor;
use iced::{
    Alignment, Element, Length,
    widget::{button, column, container, pick_list, row, space, text},
};

use crate::components::form::form_section::FormSectionBuilder;
use crate::components::input::slider_field::SliderFieldBuilder;
use crate::style::{self, tokens};

use super::{DrawingPropertiesModal, Message};
use crate::modals::pane::indicator::helpers::{format_float, format_integer};

impl DrawingPropertiesModal {
    /// Render a VBP tab's content with full section grouping, all 6
    /// parameter kinds (Boolean, Choice, Float, Integer, Color,
    /// LineStyle), and conditional visibility.
    pub(super) fn vbp_tab_content(&self, tab: study::ParameterTab) -> Element<'_, Message> {
        let Some(ref vbp) = self.vbp else {
            return text("No VBP config").into();
        };
        let config = &vbp.config;
        let params = &vbp.params;

        // Collect visible params for this tab
        let visible: Vec<&study::ParameterDef> = params
            .iter()
            .filter(|p| p.tab == tab)
            .filter(|p| !super::HIDDEN_KEYS.contains(&p.key.as_str()))
            .filter(|p| p.visible_when.is_visible(config))
            .collect();

        if visible.is_empty() {
            return container(
                text("No configurable parameters")
                    .size(tokens::text::BODY)
                    .style(|theme: &iced::Theme| text::Style {
                        color: Some(theme.extended_palette().background.weak.text),
                    }),
            )
            .padding(tokens::spacing::LG)
            .into();
        }

        // Group by section, preserving order
        let mut section_order: Vec<Option<&study::config::ParameterSection>> = Vec::new();
        for p in &visible {
            let key = p.section.as_ref().map(|s| s.label);
            if !section_order
                .iter()
                .any(|existing| existing.map(|s| s.label) == key)
            {
                section_order.push(p.section.as_ref());
            }
        }
        section_order.sort_by_key(|s| match s {
            Some(sec) => (0, sec.order, ""),
            None => (1, 0, ""),
        });

        let tab_label = super::view::vbp_tab_default_label(tab);

        let mut content_col: Vec<Element<'_, Message>> = Vec::new();
        for section_def in &section_order {
            let section_label = section_def.map(|s| s.label);
            let mut section_params: Vec<&study::ParameterDef> = visible
                .iter()
                .filter(|p| p.section.as_ref().map(|s| s.label) == section_label)
                .copied()
                .collect();
            section_params.sort_by_key(|p| p.order);

            let title = section_label.unwrap_or(tab_label);
            let mut form = FormSectionBuilder::new(title).spacing(tokens::spacing::LG);

            for param in section_params {
                let widget = self.vbp_param_widget(param, config);
                form = form.push(widget);
            }

            content_col.push(form.into());
        }

        column(content_col)
            .spacing(if section_order.len() > 1 {
                tokens::spacing::XL
            } else {
                0.0
            })
            .into()
    }

    /// Render a single VBP parameter as a widget.
    pub(super) fn vbp_param_widget<'a>(
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
                SliderFieldBuilder::new(&param.label, min_f..=max_f, current_f, move |v| {
                    Message::VbpParamChanged(key.clone(), study::ParameterValue::Integer(v as i64))
                })
                .step(1.0)
                .format(move |v| format_integer(*v as i64, fmt))
                .into()
            }
            ParameterKind::Float { min, max, step, .. } => {
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
                SliderFieldBuilder::new(&param.label, min_f..=max_f, current_f, move |v| {
                    Message::VbpParamChanged(key.clone(), study::ParameterValue::Float(v as f64))
                })
                .step(step_f)
                .format(move |v| format_float(*v, fmt))
                .into()
            }
            ParameterKind::Color => {
                let current = config.get_color(
                    &param.key,
                    match &param.default {
                        study::ParameterValue::Color(c) => *c,
                        _ => SerializableColor::new(1.0, 1.0, 1.0, 1.0),
                    },
                );
                let iced_color = crate::style::theme::rgba_to_iced_color(current);
                let is_editing = self
                    .vbp
                    .as_ref()
                    .is_some_and(|v| v.editing_color_key.as_deref() == Some(param.key.as_str()));
                let key_for_press = key.clone();
                let key_for_picker = key.clone();

                let swatch = button(space::horizontal().width(22).height(22))
                    .style(move |_theme, _status| button::Style {
                        background: Some(iced_color.into()),
                        border: iced::border::rounded(3)
                            .width(if is_editing { 2.0 } else { 1.0 })
                            .color(if is_editing {
                                iced::Color::WHITE
                            } else {
                                iced::Color::from_rgba(1.0, 1.0, 1.0, 0.3)
                            }),
                        ..button::Style::default()
                    })
                    .padding(0)
                    .on_press(Message::VbpEditColor(key_for_press));

                let mut col = column![
                    row![
                        text(&param.label).size(tokens::text::BODY),
                        space::horizontal(),
                        swatch,
                    ]
                    .align_y(Alignment::Center)
                    .width(Length::Fill),
                ]
                .spacing(tokens::spacing::SM);

                if is_editing {
                    let hsva = self
                        .vbp
                        .as_ref()
                        .and_then(|v| v.editing_color_hsva)
                        .unwrap_or_else(|| {
                            crate::config::theme::rgba_to_hsva(
                                crate::style::theme::iced_color_to_rgba(iced_color),
                            )
                        });
                    col = col.push(
                        container(crate::components::input::color_picker::color_picker(
                            hsva,
                            move |h| Message::VbpColorChanged(key_for_picker.clone(), h),
                            180.0,
                        ))
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
                        study::ParameterValue::Boolean(v) => *v,
                        _ => false,
                    },
                );
                let key = key.clone();
                crate::components::input::toggle_switch::toggle_switch(
                    &param.label,
                    current,
                    move |v| {
                        Message::VbpParamChanged(key.clone(), study::ParameterValue::Boolean(v))
                    },
                )
            }
            ParameterKind::Choice { options } => {
                let current = config
                    .get_choice(
                        &param.key,
                        match &param.default {
                            study::ParameterValue::Choice(s) => s.as_str(),
                            _ => options.first().unwrap_or(&""),
                        },
                    )
                    .to_string();
                let options_vec: Vec<String> = options.iter().map(|s| s.to_string()).collect();
                let selected = options_vec.iter().find(|o| **o == current).cloned();
                let key = key.clone();
                row![
                    text(&param.label).size(tokens::text::BODY),
                    space::horizontal(),
                    pick_list(options_vec, selected, move |v: String| {
                        Message::VbpParamChanged(key.clone(), study::ParameterValue::Choice(v))
                    },)
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
                        study::ParameterValue::LineStyle(v) => *v,
                        _ => study::config::LineStyleValue::Solid,
                    },
                );
                let options = vec![
                    study::config::LineStyleValue::Solid,
                    study::config::LineStyleValue::Dashed,
                    study::config::LineStyleValue::Dotted,
                ];
                let key = key.clone();
                row![
                    text(&param.label).size(tokens::text::BODY),
                    space::horizontal(),
                    pick_list(options, Some(current), move |v| {
                        Message::VbpLineStyleChanged(key.clone(), v)
                    })
                    .width(120),
                ]
                .align_y(Alignment::Center)
                .width(Length::Fill)
                .into()
            }
            ParameterKind::DynamicChoice | ParameterKind::MultiChoice { .. } => {
                // Not used in VBP settings currently
                space::horizontal().into()
            }
        }
    }
}
