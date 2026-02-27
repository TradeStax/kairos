//! Settings view — right panel with parameters, style, and display tabs.
//!
//! Renders study settings entirely from `ParameterDef` metadata:
//! tabs, sections, ordering, formatting, and conditional visibility
//! are all derived from the study's parameter definitions.

use super::*;

use crate::components::display::empty_state::EmptyStateBuilder;
use crate::components::form::form_section::FormSectionBuilder;
use crate::components::input::color_picker::color_picker;
use crate::components::input::slider_field::SliderFieldBuilder;
use crate::components::layout::button_group::ButtonGroupBuilder;
use crate::components::primitives::icons::Icon;
use crate::style::{self, tokens};

use iced::{
    Alignment, Element, Length,
    widget::{
        button, center, column, container, pick_list, row, rule, scrollable,
        scrollable::{Direction, Scrollbar},
        space, text,
    },
};

use super::helpers::placement_badge;

impl IndicatorManagerModal {
    // ── Right Panel ──────────────────────────────────────────────────

    pub(super) fn view_right_panel(&self) -> Element<'_, Message> {
        match &self.selected {
            None => center(
                EmptyStateBuilder::new("Select an indicator to view settings").icon(Icon::Cog),
            )
            .into(),
            Some(SelectedIndicator::Study(id)) => self.view_study_settings(id),
        }
    }

    fn view_study_settings(&self, study_id: &str) -> Element<'_, Message> {
        let snapshot = self.study_snapshots.iter().find(|(id, _)| id == study_id);

        let Some((_, study)) = snapshot else {
            return center(EmptyStateBuilder::new("Study not found").icon(Icon::Close)).into();
        };

        let placement = study.placement();
        let params = study.parameters();
        let config = study.config();

        let placement_badge = placement_badge(placement);

        // Header
        let mut header = row![
            text(study.name()).size(tokens::text::TITLE),
            space::horizontal(),
        ]
        .align_y(Alignment::Center)
        .width(Length::Fill);

        header = header.push(placement_badge);

        // Discover tabs from parameter metadata
        let tabs = discover_tabs(params);
        let tab_labels = study.tab_labels();

        let tab_items: Vec<(String, Message)> = tabs
            .iter()
            .map(|tab| {
                let label = tab_label_for(*tab, tab_labels);
                (
                    label.to_string(),
                    Message::TabChanged(param_tab_to_settings_tab(*tab)),
                )
            })
            .collect();

        let current_param_tab = settings_tab_to_param_tab(self.settings_tab);
        let selected_tab_idx = tabs
            .iter()
            .position(|t| *t == current_param_tab)
            .unwrap_or(0);

        let tab_bar = ButtonGroupBuilder::new(tab_items, selected_tab_idx).tab_style();

        // Tab content — generic data-driven rendering
        let active_tab = tabs
            .get(selected_tab_idx)
            .copied()
            .unwrap_or(study::ParameterTab::Parameters);

        let tab_content = self.view_tab_content(study_id, params, config, active_tab);

        column![
            header,
            rule::horizontal(1).style(style::split_ruler),
            tab_bar,
            scrollable::Scrollable::with_direction(
                tab_content,
                Direction::Vertical(
                    Scrollbar::new()
                        .width(tokens::layout::SCROLLBAR_WIDTH)
                        .scroller_width(tokens::layout::SCROLLBAR_WIDTH,),
                ),
            )
            .style(style::scroll_bar)
            .height(Length::Fill),
        ]
        .spacing(tokens::spacing::MD)
        .height(Length::Fill)
        .into()
    }

    /// Generic data-driven tab content renderer.
    ///
    /// Filters parameters for the active tab, checks visibility,
    /// groups by section, sorts by order, and renders each parameter
    /// using `param_widget`.
    fn view_tab_content<'a>(
        &'a self,
        study_id: &str,
        params: &'a [study::ParameterDef],
        config: &'a study::StudyConfig,
        tab: study::ParameterTab,
    ) -> Element<'a, Message> {
        // Collect visible params for this tab
        let visible: Vec<&study::ParameterDef> = params
            .iter()
            .filter(|p| p.tab == tab)
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

        // Group parameters by section
        // Collect unique sections in definition order, preserving
        // the section's own `order` field for sorting.
        let mut section_order: Vec<Option<&study::ParameterSection>> = Vec::new();
        for p in &visible {
            let key = p.section.as_ref().map(|s| s.label);
            if !section_order
                .iter()
                .any(|existing| existing.map(|s| s.label) == key)
            {
                section_order.push(p.section.as_ref());
            }
        }

        // Sort sections: None-sections keep their discovery order,
        // Some-sections sort by their `order` field.
        section_order.sort_by_key(|s| match s {
            Some(sec) => (0, sec.order, ""),
            None => (1, 0, ""),
        });

        let mut content_col: Vec<Element<'a, Message>> = Vec::new();

        for section_def in &section_order {
            let section_label = section_def.map(|s| s.label);

            // Collect params for this section, sorted by order
            let mut section_params: Vec<&study::ParameterDef> = visible
                .iter()
                .filter(|p| p.section.as_ref().map(|s| s.label) == section_label)
                .copied()
                .collect();
            section_params.sort_by_key(|p| p.order);

            // Build form section
            let title = section_label.unwrap_or(tab_default_label(tab));

            let mut form = FormSectionBuilder::new(title).spacing(tokens::spacing::LG);

            for param in section_params {
                let widget = self.param_widget(study_id, param, config);
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

    fn param_widget<'a>(
        &'a self,
        study_id: &str,
        param: &'a study::ParameterDef,
        config: &study::StudyConfig,
    ) -> Element<'a, Message> {
        let sid = study_id.to_string();
        let key = param.key.to_string();

        match &param.kind {
            study::ParameterKind::Integer { min, max } => {
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

                SliderFieldBuilder::new(&param.label, min_f..=max_f, current_f, {
                    let sid = sid.clone();
                    let key = key.clone();
                    move |v| Message::ParameterChanged {
                        study_id: sid.clone(),
                        key: key.clone(),
                        value: study::ParameterValue::Integer(v as i64),
                    }
                })
                .step(1.0)
                .format(move |v| format_integer(*v as i64, fmt))
                .into()
            }
            study::ParameterKind::Float { min, max, step } => {
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

                SliderFieldBuilder::new(&param.label, min_f..=max_f, current_f, {
                    let sid = sid.clone();
                    let key = key.clone();
                    move |v| Message::ParameterChanged {
                        study_id: sid.clone(),
                        key: key.clone(),
                        value: study::ParameterValue::Float(v as f64),
                    }
                })
                .step(step_f)
                .format(move |v| format_float(*v, fmt))
                .into()
            }
            study::ParameterKind::Color => {
                let current = config.get_color(
                    &param.key,
                    match &param.default {
                        study::ParameterValue::Color(c) => *c,
                        _ => data::SerializableColor::new(1.0, 1.0, 1.0, 1.0),
                    },
                );
                let iced_color: iced::Color = crate::style::theme::rgba_to_iced_color(current);
                let is_editing = self.editing_color_key.as_deref() == Some(param.key.as_str());

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
                    .on_press(Message::EditColor(key.clone()));

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
                    let hsva = self.editing_color_hsva.unwrap_or_else(|| {
                        crate::config::theme::rgba_to_hsva(crate::style::theme::iced_color_to_rgba(
                            iced_color,
                        ))
                    });
                    col = col.push(
                        container(color_picker(hsva, Message::ColorChanged, 180.0))
                            .padding(tokens::spacing::SM)
                            .style(style::dropdown_container),
                    );
                }

                col.into()
            }
            study::ParameterKind::Boolean => {
                let current = config.get_bool(
                    &param.key,
                    match &param.default {
                        study::ParameterValue::Boolean(v) => *v,
                        _ => false,
                    },
                );

                crate::components::input::toggle_switch::toggle_switch(&param.label, current, {
                    let sid = sid.clone();
                    let key = key.clone();
                    move |v| Message::ParameterChanged {
                        study_id: sid.clone(),
                        key: key.clone(),
                        value: study::ParameterValue::Boolean(v),
                    }
                })
            }
            study::ParameterKind::Choice { options } => {
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

                row![
                    text(&param.label).size(tokens::text::BODY),
                    space::horizontal(),
                    pick_list(options_vec, selected, {
                        let sid = sid.clone();
                        let key = key.clone();
                        move |v: String| Message::ParameterChanged {
                            study_id: sid.clone(),
                            key: key.clone(),
                            value: study::ParameterValue::Choice(v),
                        }
                    })
                    .width(140),
                ]
                .align_y(Alignment::Center)
                .width(Length::Fill)
                .into()
            }
            study::ParameterKind::LineStyle => {
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

                row![
                    text(&param.label).size(tokens::text::BODY),
                    space::horizontal(),
                    pick_list(options, Some(current), {
                        let sid = sid.clone();
                        let key = key.clone();
                        move |v| Message::ParameterChanged {
                            study_id: sid.clone(),
                            key: key.clone(),
                            value: study::ParameterValue::LineStyle(v),
                        }
                    })
                    .width(120),
                ]
                .align_y(Alignment::Center)
                .width(Length::Fill)
                .into()
            }
        }
    }
}

// ── Helper functions ─────────────────────────────────────────────────

/// Discover which tabs are present, deduplicating and preserving the
/// order from parameter definitions.
fn discover_tabs(params: &[study::ParameterDef]) -> Vec<study::ParameterTab> {
    let mut tabs: Vec<study::ParameterTab> = Vec::new();
    for p in params {
        if !tabs.contains(&p.tab) {
            tabs.push(p.tab);
        }
    }
    tabs
}

/// Get the display label for a tab, using custom labels from the study
/// if available, or falling back to default names.
fn tab_label_for(
    tab: study::ParameterTab,
    custom: Option<&[(&'static str, study::ParameterTab)]>,
) -> &'static str {
    if let Some(labels) = custom {
        for (label, t) in labels {
            if *t == tab {
                return label;
            }
        }
    }
    tab_default_label(tab)
}

/// Default display label for a `ParameterTab`.
fn tab_default_label(tab: study::ParameterTab) -> &'static str {
    match tab {
        study::ParameterTab::Parameters => "Parameters",
        study::ParameterTab::Style => "Style",
        study::ParameterTab::Display => "Display",
        study::ParameterTab::PocSettings => "POC",
        study::ParameterTab::ValueArea => "Value Area",
        study::ParameterTab::Nodes => "Nodes",
        study::ParameterTab::Vwap => "VWAP",
        study::ParameterTab::Absorption => "Absorption",
    }
}

/// Convert `ParameterTab` to `SettingsTab`.
fn param_tab_to_settings_tab(tab: study::ParameterTab) -> SettingsTab {
    match tab {
        study::ParameterTab::Parameters => SettingsTab::Parameters,
        study::ParameterTab::Style => SettingsTab::Style,
        study::ParameterTab::Display => SettingsTab::Display,
        study::ParameterTab::PocSettings => SettingsTab::PocSettings,
        study::ParameterTab::ValueArea => SettingsTab::ValueArea,
        study::ParameterTab::Nodes => SettingsTab::Nodes,
        study::ParameterTab::Vwap => SettingsTab::Vwap,
        study::ParameterTab::Absorption => SettingsTab::Absorption,
    }
}

/// Convert `SettingsTab` to `ParameterTab`.
fn settings_tab_to_param_tab(tab: SettingsTab) -> study::ParameterTab {
    match tab {
        SettingsTab::Parameters => study::ParameterTab::Parameters,
        SettingsTab::Style => study::ParameterTab::Style,
        SettingsTab::Display => study::ParameterTab::Display,
        SettingsTab::PocSettings => study::ParameterTab::PocSettings,
        SettingsTab::ValueArea => study::ParameterTab::ValueArea,
        SettingsTab::Nodes => study::ParameterTab::Nodes,
        SettingsTab::Vwap => study::ParameterTab::Vwap,
        SettingsTab::Absorption => study::ParameterTab::Absorption,
    }
}

use super::helpers::{format_float, format_integer};
