//! Settings view — right panel with parameters, style, and display tabs.

use super::*;

use crate::components::display::empty_state::EmptyStateBuilder;
use crate::components::form::form_section::FormSectionBuilder;
use crate::components::input::checkbox_field::CheckboxFieldBuilder;
use crate::components::input::color_picker::color_picker;
use crate::components::input::slider_field::SliderFieldBuilder;
use crate::components::layout::button_group::ButtonGroupBuilder;
use crate::components::primitives::icons::Icon;
use crate::style::{self, tokens};

use iced::{
    Alignment, Element, Length,
    widget::{
        button, center, column, container, pick_list, row, rule, scrollable,
        space, text,
    },
};

use super::helpers::placement_badge;

impl IndicatorManagerModal {
    // ── Right Panel ──────────────────────────────────────────────────

    pub(super) fn view_right_panel(&self) -> Element<'_, Message> {
        match &self.selected {
            None => {
                center(
                    EmptyStateBuilder::new(
                        "Select an indicator to view settings",
                    )
                    .icon(Icon::Cog),
                )
                .into()
            }
            Some(SelectedIndicator::Study(id)) => {
                self.view_study_settings(id)
            }
        }
    }

    fn view_study_settings(
        &self,
        study_id: &str,
    ) -> Element<'_, Message> {
        let snapshot = self
            .study_snapshots
            .iter()
            .find(|(id, _)| id == study_id);

        let Some((_, study)) = snapshot else {
            return center(
                EmptyStateBuilder::new("Study not found")
                    .icon(Icon::Close),
            )
            .into();
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

        // Add debug button for Big Trades study
        if study_id == "big_trades" {
            header = header.push(
                button(text("Debug").size(tokens::text::TINY))
                    .on_press(Message::OpenBigTradesDebug)
                    .padding([
                        tokens::spacing::XXS,
                        tokens::spacing::SM,
                    ])
                    .style(style::button::secondary),
            );
            header = header.push(space::horizontal().width(tokens::spacing::SM));
        }

        header = header.push(placement_badge);

        // Tab bar — study-specific tabs and labels
        let tabs = SettingsTab::tabs_for_study(study_id);
        let tab_items: Vec<(String, Message)> = tabs
            .iter()
            .map(|tab| {
                (
                    tab.label_for_study(study_id).to_string(),
                    Message::TabChanged(*tab),
                )
            })
            .collect();
        let selected_tab_idx = tabs
            .iter()
            .position(|t| t == &self.settings_tab)
            .unwrap_or(0);
        let tab_bar =
            ButtonGroupBuilder::new(tab_items, selected_tab_idx)
                .tab_style();

        // Tab content — dispatch to study-specific views when available
        let tab_content = if study_id == "big_trades" {
            match self.settings_tab {
                SettingsTab::Parameters => {
                    self.view_big_trades_data_tab(study_id, config)
                }
                SettingsTab::Style => {
                    self.view_big_trades_style_tab(study_id, config)
                }
                _ => {
                    self.view_parameters_tab(study_id, params, config)
                }
            }
        } else {
            match self.settings_tab {
                SettingsTab::Parameters => {
                    self.view_parameters_tab(study_id, params, config)
                }
                SettingsTab::Style => {
                    self.view_style_tab(study_id, params, config)
                }
                SettingsTab::Display => {
                    self.view_display_tab(study_id, params, config)
                }
            }
        };

        column![
            header,
            rule::horizontal(1).style(style::split_ruler),
            tab_bar,
            scrollable(tab_content)
                .style(style::scroll_bar)
                .height(Length::Fill),
        ]
        .spacing(tokens::spacing::MD)
        .height(Length::Fill)
        .into()
    }

    fn view_parameters_tab<'a>(
        &'a self,
        study_id: &str,
        params: &[study::ParameterDef],
        config: &study::StudyConfig,
    ) -> Element<'a, Message> {
        let parameter_keys: &[&str] = &[
            "period",
            "source",
            "overbought",
            "oversold",
            "std_dev",
            "fast_period",
            "slow_period",
            "signal_period",
            "k_period",
            "d_period",
            "slowing",
            "threshold",
            "min_contracts",
            "aggregation_window_ms",
            "lookback",
            "value_area_pct",
            "ratio",
        ];

        let relevant: Vec<&study::ParameterDef> = params
            .iter()
            .filter(|p| parameter_keys.contains(&p.key))
            .collect();

        if relevant.is_empty() {
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

        let mut section =
            FormSectionBuilder::new("Parameters").spacing(tokens::spacing::LG);

        for param in relevant {
            let widget = self.param_widget(study_id, param, config);
            section = section.push(widget);
        }

        section.into()
    }

    fn view_style_tab<'a>(
        &'a self,
        study_id: &str,
        params: &[study::ParameterDef],
        config: &study::StudyConfig,
    ) -> Element<'a, Message> {
        let style_keys: &[&str] = &[
            "color",
            "width",
            "opacity",
            "line_style",
            "fill_opacity",
            "upper_color",
            "lower_color",
            "middle_color",
            "signal_color",
            "histogram_bull_color",
            "histogram_bear_color",
            "buy_color",
            "sell_color",
            "poc_color",
            "vah_color",
            "val_color",
        ];

        let relevant: Vec<&study::ParameterDef> = params
            .iter()
            .filter(|p| style_keys.contains(&p.key))
            .collect();

        if relevant.is_empty() {
            return container(
                text("No style options")
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

        let mut section =
            FormSectionBuilder::new("Style").spacing(tokens::spacing::LG);

        for param in relevant {
            let widget = self.param_widget(study_id, param, config);
            section = section.push(widget);
        }

        section.into()
    }

    fn view_display_tab<'a>(
        &'a self,
        study_id: &str,
        params: &[study::ParameterDef],
        config: &study::StudyConfig,
    ) -> Element<'a, Message> {
        let display_keys: &[&str] = &[
            "show_labels",
            "show_debug",
            "show_prices",
            "show_percentages",
            "visible",
            "show_fill",
            "show_bands",
            "show_signal",
            "show_histogram",
        ];

        let relevant: Vec<&study::ParameterDef> = params
            .iter()
            .filter(|p| display_keys.contains(&p.key))
            .collect();

        if relevant.is_empty() {
            return container(
                text("No display options")
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

        let mut section =
            FormSectionBuilder::new("Display").spacing(tokens::spacing::LG);

        for param in relevant {
            let widget = self.param_widget(study_id, param, config);
            section = section.push(widget);
        }

        section.into()
    }

    // ── Big Trades: Data Settings tab ──────────────────────────────

    fn view_big_trades_data_tab<'a>(
        &'a self,
        study_id: &str,
        config: &study::StudyConfig,
    ) -> Element<'a, Message> {
        let sid = study_id.to_string();

        let days_to_load = config.get_int("days_to_load", 1);
        let filter_min = config.get_int("filter_min", 50);
        let filter_max = config.get_int("filter_max", 0);
        let agg_window =
            config.get_int("aggregation_window_ms", 40);

        let mut section = FormSectionBuilder::new("Data Settings")
            .spacing(tokens::spacing::LG);

        // Days to Load
        section = section.push(
            SliderFieldBuilder::new(
                "Days to Load",
                1.0f32..=30.0,
                days_to_load as f32,
                {
                    let sid = sid.clone();
                    move |v| Message::ParameterChanged {
                        study_id: sid.clone(),
                        key: "days_to_load".to_string(),
                        value: study::ParameterValue::Integer(
                            v as i64,
                        ),
                    }
                },
            )
            .step(1.0)
            .format(|v| format!("{}", *v as i64)),
        );

        // Filter Min
        section = section.push(
            SliderFieldBuilder::new(
                "Filter Min",
                0.0f32..=500.0,
                filter_min as f32,
                {
                    let sid = sid.clone();
                    move |v| Message::ParameterChanged {
                        study_id: sid.clone(),
                        key: "filter_min".to_string(),
                        value: study::ParameterValue::Integer(
                            v as i64,
                        ),
                    }
                },
            )
            .step(5.0)
            .format(|v| {
                let i = *v as i64;
                if i == 0 { "None".to_string() } else { format!("{i}") }
            }),
        );

        // Filter Max
        section = section.push(
            SliderFieldBuilder::new(
                "Filter Max",
                0.0f32..=2000.0,
                filter_max as f32,
                {
                    let sid = sid.clone();
                    move |v| Message::ParameterChanged {
                        study_id: sid.clone(),
                        key: "filter_max".to_string(),
                        value: study::ParameterValue::Integer(
                            v as i64,
                        ),
                    }
                },
            )
            .step(10.0)
            .format(|v| {
                let i = *v as i64;
                if i == 0 { "None".to_string() } else { format!("{i}") }
            }),
        );

        // Aggregation Window
        section = section.push(
            SliderFieldBuilder::new(
                "Aggregation Window",
                10.0f32..=1000.0,
                agg_window as f32,
                {
                    let sid = sid.clone();
                    move |v| Message::ParameterChanged {
                        study_id: sid.clone(),
                        key: "aggregation_window_ms".to_string(),
                        value: study::ParameterValue::Integer(
                            v as i64,
                        ),
                    }
                },
            )
            .step(10.0)
            .format(|v| format!("{}ms", *v as i64)),
        );

        section.into()
    }

    // ── Big Trades: Style tab ───────────────────────────────────────

    fn view_big_trades_style_tab<'a>(
        &'a self,
        study_id: &str,
        config: &study::StudyConfig,
    ) -> Element<'a, Message> {
        let sid = study_id.to_string();

        let marker_shape = config
            .get_choice("marker_shape", "Circle")
            .to_string();
        let hollow = config.get_bool("hollow", false);
        let show_text = config.get_bool("show_text", true);
        let std_dev = config.get_float("std_dev", 2.5) as f32;
        let min_size = config.get_float("min_size", 6.0) as f32;
        let max_size = config.get_float("max_size", 40.0) as f32;
        let min_opacity =
            config.get_float("min_opacity", 0.4) as f32;
        let max_opacity =
            config.get_float("max_opacity", 1.0) as f32;
        let text_size_val =
            config.get_float("text_size", 11.0) as f32;

        // ── General section ──────────────────────────────────────
        let shape_options: Vec<String> = ["Circle", "Square", "Text Only"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let shape_selected = shape_options
            .iter()
            .find(|o| **o == marker_shape)
            .cloned();

        let mut general = FormSectionBuilder::new("General")
            .spacing(tokens::spacing::LG);

        general = general.push(
            row![
                text("Marker Shape").size(tokens::text::BODY),
                space::horizontal(),
                pick_list(shape_options, shape_selected, {
                    let sid = sid.clone();
                    move |v: String| Message::ParameterChanged {
                        study_id: sid.clone(),
                        key: "marker_shape".to_string(),
                        value: study::ParameterValue::Choice(v),
                    }
                })
                .width(120),
            ]
            .align_y(Alignment::Center)
            .width(Length::Fill),
        );

        general = general.push(
            crate::components::input::toggle_switch::toggle_switch(
                "Hollow Fill",
                hollow,
                {
                    let sid = sid.clone();
                    move |v| Message::ParameterChanged {
                        study_id: sid.clone(),
                        key: "hollow".to_string(),
                        value: study::ParameterValue::Boolean(v),
                    }
                },
            ),
        );

        general = general.push(
            crate::components::input::toggle_switch::toggle_switch(
                "Show Text",
                show_text,
                {
                    let sid = sid.clone();
                    move |v| Message::ParameterChanged {
                        study_id: sid.clone(),
                        key: "show_text".to_string(),
                        value: study::ParameterValue::Boolean(v),
                    }
                },
            ),
        );

        // ── Size section ─────────────────────────────────────────
        let mut size_section = FormSectionBuilder::new("Size")
            .spacing(tokens::spacing::LG)
            .with_top_divider(true);

        size_section = size_section.push(
            SliderFieldBuilder::new(
                "Std Dev",
                0.5f32..=5.0,
                std_dev,
                {
                    let sid = sid.clone();
                    move |v| Message::ParameterChanged {
                        study_id: sid.clone(),
                        key: "std_dev".to_string(),
                        value: study::ParameterValue::Float(
                            v as f64,
                        ),
                    }
                },
            )
            .step(0.1)
            .format(|v| format!("{v:.2}")),
        );

        size_section = size_section.push(
            SliderFieldBuilder::new(
                "Min Size",
                2.0f32..=60.0,
                min_size,
                {
                    let sid = sid.clone();
                    move |v| Message::ParameterChanged {
                        study_id: sid.clone(),
                        key: "min_size".to_string(),
                        value: study::ParameterValue::Float(
                            v as f64,
                        ),
                    }
                },
            )
            .step(1.0)
            .format(|v| format!("{}", *v as i32)),
        );

        size_section = size_section.push(
            SliderFieldBuilder::new(
                "Max Size",
                10.0f32..=100.0,
                max_size,
                {
                    let sid = sid.clone();
                    move |v| Message::ParameterChanged {
                        study_id: sid.clone(),
                        key: "max_size".to_string(),
                        value: study::ParameterValue::Float(
                            v as f64,
                        ),
                    }
                },
            )
            .step(1.0)
            .format(|v| format!("{}", *v as i32)),
        );

        // ── Color section ────────────────────────────────────────
        let mut color_section = FormSectionBuilder::new("Color")
            .spacing(tokens::spacing::LG)
            .with_top_divider(true);

        color_section = color_section.push(
            SliderFieldBuilder::new(
                "Min Opacity",
                0.0f32..=1.0,
                min_opacity,
                {
                    let sid = sid.clone();
                    move |v| Message::ParameterChanged {
                        study_id: sid.clone(),
                        key: "min_opacity".to_string(),
                        value: study::ParameterValue::Float(
                            v as f64,
                        ),
                    }
                },
            )
            .step(0.05)
            .format(|v| format!("{:.0}%", v * 100.0)),
        );

        color_section = color_section.push(
            SliderFieldBuilder::new(
                "Max Opacity",
                0.0f32..=1.0,
                max_opacity,
                {
                    let sid = sid.clone();
                    move |v| Message::ParameterChanged {
                        study_id: sid.clone(),
                        key: "max_opacity".to_string(),
                        value: study::ParameterValue::Float(
                            v as f64,
                        ),
                    }
                },
            )
            .step(0.05)
            .format(|v| format!("{:.0}%", v * 100.0)),
        );

        // Ask color picker
        color_section = color_section.push(
            self.color_swatch_widget(study_id, "ask_color", "Ask Color", config),
        );

        // Bid color picker
        color_section = color_section.push(
            self.color_swatch_widget(study_id, "bid_color", "Bid Color", config),
        );

        // ── Text Settings section ────────────────────────────────
        let mut text_section =
            FormSectionBuilder::new("Text Settings")
                .spacing(tokens::spacing::LG)
                .with_top_divider(true);

        text_section = text_section.push(
            SliderFieldBuilder::new(
                "Text Size",
                6.0f32..=20.0,
                text_size_val,
                {
                    let sid = sid.clone();
                    move |v| Message::ParameterChanged {
                        study_id: sid.clone(),
                        key: "text_size".to_string(),
                        value: study::ParameterValue::Float(
                            v as f64,
                        ),
                    }
                },
            )
            .step(0.5)
            .format(|v| format!("{v:.1}")),
        );

        // Text color picker
        text_section = text_section.push(
            self.color_swatch_widget(
                study_id,
                "text_color",
                "Text Color",
                config,
            ),
        );

        column![general, size_section, color_section, text_section]
            .spacing(0)
            .into()
    }

    /// Reusable color swatch + picker for a specific config key.
    fn color_swatch_widget<'a>(
        &'a self,
        _study_id: &str,
        key: &str,
        label: &'a str,
        config: &study::StudyConfig,
    ) -> Element<'a, Message> {
        let default_color =
            data::SerializableColor::new(1.0, 1.0, 1.0, 1.0);
        let current = config.get_color(key, default_color);
        let iced_color: iced::Color =
            crate::style::theme_bridge::rgba_to_iced_color(current);
        let is_editing =
            self.editing_color_key.as_deref() == Some(key);
        let key_owned = key.to_string();

        let swatch =
            button(space::horizontal().width(22).height(22))
                .style(move |_theme, _status| button::Style {
                    background: Some(iced_color.into()),
                    border: iced::border::rounded(3)
                        .width(if is_editing { 2.0 } else { 1.0 })
                        .color(if is_editing {
                            iced::Color::WHITE
                        } else {
                            iced::Color::from_rgba(
                                1.0, 1.0, 1.0, 0.3,
                            )
                        }),
                    ..button::Style::default()
                })
                .padding(0)
                .on_press(Message::EditColor(key_owned.clone()));

        let mut col = column![
            row![
                text(label).size(tokens::text::BODY),
                space::horizontal(),
                swatch,
            ]
            .align_y(Alignment::Center)
            .width(Length::Fill),
        ]
        .spacing(tokens::spacing::SM);

        if is_editing {
            let hsva = self.editing_color_hsva.unwrap_or_else(|| {
                data::config::theme::rgba_to_hsva(
                    crate::style::theme_bridge::iced_color_to_rgba(
                        iced_color,
                    ),
                )
            });
            col = col.push(
                container(color_picker(
                    hsva,
                    Message::ColorChanged,
                    180.0,
                ))
                .padding(tokens::spacing::SM)
                .style(style::dropdown_container),
            );
        }

        col.into()
    }

    fn param_widget<'a>(
        &'a self,
        study_id: &str,
        param: &study::ParameterDef,
        config: &study::StudyConfig,
    ) -> Element<'a, Message> {
        let sid = study_id.to_string();
        let key = param.key.to_string();

        match &param.kind {
            study::ParameterKind::Integer { min, max } => {
                let current = config.get_int(
                    param.key,
                    match &param.default {
                        study::ParameterValue::Integer(v) => *v,
                        _ => *min,
                    },
                );
                let min_f = *min as f32;
                let max_f = *max as f32;
                let current_f = current as f32;

                SliderFieldBuilder::new(
                    param.label,
                    min_f..=max_f,
                    current_f,
                    {
                        let sid = sid.clone();
                        let key = key.clone();
                        move |v| Message::ParameterChanged {
                            study_id: sid.clone(),
                            key: key.clone(),
                            value: study::ParameterValue::Integer(
                                v as i64,
                            ),
                        }
                    },
                )
                .step(1.0)
                .format(|v| format!("{}", *v as i64))
                .into()
            }
            study::ParameterKind::Float { min, max, step } => {
                let current = config.get_float(
                    param.key,
                    match &param.default {
                        study::ParameterValue::Float(v) => *v,
                        _ => *min,
                    },
                );
                let min_f = *min as f32;
                let max_f = *max as f32;
                let step_f = *step as f32;
                let current_f = current as f32;

                SliderFieldBuilder::new(
                    param.label,
                    min_f..=max_f,
                    current_f,
                    {
                        let sid = sid.clone();
                        let key = key.clone();
                        move |v| Message::ParameterChanged {
                            study_id: sid.clone(),
                            key: key.clone(),
                            value: study::ParameterValue::Float(
                                v as f64,
                            ),
                        }
                    },
                )
                .step(step_f)
                .format(|v| format!("{v:.2}"))
                .into()
            }
            study::ParameterKind::Color => {
                let current = config.get_color(
                    param.key,
                    match &param.default {
                        study::ParameterValue::Color(c) => *c,
                        _ => data::SerializableColor::new(
                            1.0, 1.0, 1.0, 1.0,
                        ),
                    },
                );
                let iced_color: iced::Color =
                    crate::style::theme_bridge::rgba_to_iced_color(current);
                let is_editing = self.editing_color_key.as_deref()
                    == Some(param.key);

                let swatch = button(
                    space::horizontal().width(22).height(22),
                )
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
                        text(param.label).size(tokens::text::BODY),
                        space::horizontal(),
                        swatch,
                    ]
                    .align_y(Alignment::Center)
                    .width(Length::Fill),
                ]
                .spacing(tokens::spacing::SM);

                if is_editing {
                    let hsva = self.editing_color_hsva.unwrap_or_else(|| {
                        data::config::theme::rgba_to_hsva(
                            crate::style::theme_bridge::iced_color_to_rgba(iced_color),
                        )
                    });
                    col = col.push(
                        container(color_picker(
                            hsva,
                            Message::ColorChanged,
                            180.0,
                        ))
                        .padding(tokens::spacing::SM)
                        .style(style::dropdown_container),
                    );
                }

                col.into()
            }
            study::ParameterKind::Boolean => {
                let current = config.get_bool(
                    param.key,
                    match &param.default {
                        study::ParameterValue::Boolean(v) => *v,
                        _ => false,
                    },
                );

                CheckboxFieldBuilder::new(param.label, current, {
                    let sid = sid.clone();
                    let key = key.clone();
                    move |v| Message::ParameterChanged {
                        study_id: sid.clone(),
                        key: key.clone(),
                        value: study::ParameterValue::Boolean(v),
                    }
                })
                .into()
            }
            study::ParameterKind::Choice { options } => {
                let current = config
                    .get_choice(
                        param.key,
                        match &param.default {
                            study::ParameterValue::Choice(s) => {
                                s.as_str()
                            }
                            _ => options.first().unwrap_or(&""),
                        },
                    )
                    .to_string();

                let options_vec: Vec<String> =
                    options.iter().map(|s| s.to_string()).collect();
                let selected = options_vec
                    .iter()
                    .find(|o| **o == current)
                    .cloned();

                row![
                    text(param.label).size(tokens::text::BODY),
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
                    .width(120),
                ]
                .align_y(Alignment::Center)
                .width(Length::Fill)
                .into()
            }
            study::ParameterKind::LineStyle => {
                let current = config.get_line_style(
                    param.key,
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
                    text(param.label).size(tokens::text::BODY),
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
