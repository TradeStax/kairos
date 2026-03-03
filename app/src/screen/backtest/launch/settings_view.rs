//! Settings view — right panel with parameters, dataset, and execution tabs.

use super::*;

use crate::components::display::empty_state::EmptyStateBuilder;
use crate::components::form::form_section::FormSectionBuilder;
use crate::components::input::slider_field::SliderFieldBuilder;
use crate::components::layout::button_group::ButtonGroupBuilder;
use crate::components::primitives::icons::Icon;
use crate::modals::pane::indicator::helpers::{format_float, format_integer};
use crate::style::{self, tokens};

use iced::{
    Alignment, Element, Length,
    widget::{
        button, center, column, container, pick_list, row, rule, scrollable,
        scrollable::{Direction, Scrollbar},
        space, text, text_input,
    },
};

use super::catalog_view::category_badge;

impl BacktestLaunchModal {
    // ── Right Panel ──────────────────────────────────────────────────

    pub(super) fn view_right_panel(&self) -> Element<'_, Message> {
        let Some(ref selected_id) = self.selected_strategy_id else {
            return center(
                EmptyStateBuilder::new("Select a strategy to configure").icon(Icon::Cog),
            )
            .into();
        };

        let Some((_, strategy)) = self
            .strategy_snapshots
            .iter()
            .find(|(id, _)| id == selected_id)
        else {
            return center(EmptyStateBuilder::new("Strategy not found").icon(Icon::Close)).into();
        };

        let meta = strategy.metadata();
        let cat_badge = category_badge(meta.category);

        // Header
        let header = row![
            text(meta.name.clone()).size(tokens::text::TITLE),
            space::horizontal(),
            cat_badge,
        ]
        .align_y(Alignment::Center)
        .width(Length::Fill);

        // Tab bar
        let tab_items: Vec<(String, Message)> = SettingsTab::ALL
            .iter()
            .map(|tab| (tab.to_string(), Message::TabChanged(*tab)))
            .collect();
        let selected_tab_idx = SettingsTab::ALL
            .iter()
            .position(|t| *t == self.settings_tab)
            .unwrap_or(0);
        let tab_bar = ButtonGroupBuilder::new(tab_items, selected_tab_idx).tab_style();

        // Tab content
        let tab_content: Element<'_, Message> = match self.settings_tab {
            SettingsTab::Parameters => self.view_parameters_tab(selected_id, strategy.as_ref()),
            SettingsTab::Dataset => self.view_dataset_tab(),
            SettingsTab::Execution => self.view_execution_tab(),
        };

        // Footer
        let footer = self.view_footer();

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
            rule::horizontal(1).style(style::split_ruler),
            footer,
        ]
        .spacing(tokens::spacing::MD)
        .height(Length::Fill)
        .into()
    }

    // ── Parameters Tab ───────────────────────────────────────────────

    fn view_parameters_tab<'a>(
        &'a self,
        study_id: &str,
        strategy: &'a dyn backtest::Strategy,
    ) -> Element<'a, Message> {
        let params = strategy.parameters();
        let config = strategy.config();

        let visible: Vec<&study::ParameterDef> = params
            .iter()
            .filter(|p| p.tab == study::ParameterTab::Parameters)
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

        // Group by section
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
        section_order.sort_by_key(|s| match s {
            Some(sec) => (0, sec.order, ""),
            None => (1, 0, ""),
        });

        let mut content_col: Vec<Element<'a, Message>> = Vec::new();

        for section_def in &section_order {
            let section_label = section_def.map(|s| s.label);
            let mut section_params: Vec<&study::ParameterDef> = visible
                .iter()
                .filter(|p| p.section.as_ref().map(|s| s.label) == section_label)
                .copied()
                .collect();
            section_params.sort_by_key(|p| p.order);

            let title = section_label.unwrap_or("Parameters");

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
                        strategy_id: sid.clone(),
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
                        strategy_id: sid.clone(),
                        key: key.clone(),
                        value: study::ParameterValue::Float(v as f64),
                    }
                })
                .step(step_f)
                .format(move |v| format_float(*v, fmt))
                .into()
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
                        strategy_id: sid.clone(),
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
                            strategy_id: sid.clone(),
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
            // Color and LineStyle not used by backtest strategies
            _ => space::horizontal().into(),
        }
    }

    // ── Dataset Tab ──────────────────────────────────────────────────

    fn view_dataset_tab(&self) -> Element<'_, Message> {
        if self.connections.is_empty() {
            return center(
                EmptyStateBuilder::new(
                    "No data connections configured \u{2014} add a \
                     connection to run backtests",
                )
                .icon(Icon::Cog),
            )
            .into();
        }

        // Connection picker
        let conn_names: Vec<String> = self.connections.iter().map(|c| c.to_string()).collect();
        let selected_conn = self
            .selected_connection_idx
            .map(|idx| conn_names[idx].clone());

        let conn_dropdown = pick_list(
            conn_names.clone(),
            selected_conn,
            move |selected: String| {
                let idx = conn_names.iter().position(|n| n == &selected).unwrap_or(0);
                Message::ConnectionSelected(idx)
            },
        )
        .width(Length::Fill);

        let source_section = FormSectionBuilder::new("Data Source").push(
            row![
                text("Connection")
                    .size(tokens::text::BODY)
                    .width(Length::Fixed(tokens::component::form::LABEL_WIDTH)),
                conn_dropdown,
            ]
            .spacing(tokens::spacing::MD)
            .align_y(Alignment::Center),
        );

        // Instrument section — only if a connection is selected
        let instrument_section: Element<'_, Message> = if self.selected_connection_idx.is_some() {
            if self.connection_tickers.is_empty() {
                container(
                    text("No data available for this connection")
                        .size(tokens::text::BODY)
                        .style(|theme: &iced::Theme| text::Style {
                            color: Some(theme.extended_palette().background.weak.text),
                        }),
                )
                .padding(tokens::spacing::LG)
                .into()
            } else if self.connection_tickers.len() == 1 {
                // Single ticker — show as read-only text
                let (sym, name) = &self.connection_tickers[0];
                FormSectionBuilder::new("Instrument")
                    .push(
                        row![
                            text("Ticker")
                                .size(tokens::text::BODY)
                                .width(Length::Fixed(tokens::component::form::LABEL_WIDTH,)),
                            text(format!("{} ({})", sym, name)).size(tokens::text::BODY),
                        ]
                        .spacing(tokens::spacing::MD)
                        .align_y(Alignment::Center),
                    )
                    .into()
            } else {
                // Multiple tickers — pick list
                let display_names: Vec<String> = self
                    .connection_tickers
                    .iter()
                    .map(|(sym, name)| format!("{} ({})", sym, name))
                    .collect();
                let selected_display = self.selected_ticker.as_ref().and_then(|sel| {
                    self.connection_tickers
                        .iter()
                        .find(|(sym, _)| sym == sel)
                        .map(|(sym, name)| format!("{} ({})", sym, name))
                });

                let ticker_dropdown =
                    pick_list(display_names, selected_display, move |selected: String| {
                        let sym = selected.split(' ').next().unwrap_or(&selected);
                        Message::TickerSelected(sym.to_string())
                    })
                    .width(Length::Fill);

                FormSectionBuilder::new("Instrument")
                    .push(
                        row![
                            text("Ticker")
                                .size(tokens::text::BODY)
                                .width(Length::Fixed(tokens::component::form::LABEL_WIDTH,)),
                            ticker_dropdown,
                        ]
                        .spacing(tokens::spacing::MD)
                        .align_y(Alignment::Center),
                    )
                    .into()
            }
        } else {
            space::vertical().height(0).into()
        };

        // Date Range section
        let calendar_view = self.calendar.view(Message::Calendar);

        let mut date_section = FormSectionBuilder::new("Date Range").push(calendar_view);

        if self.calendar_mode == CalendarMode::AnyDate {
            date_section = date_section.push(
                text("Data will be downloaded automatically")
                    .size(tokens::text::TINY)
                    .style(|theme: &iced::Theme| text::Style {
                        color: Some(theme.extended_palette().background.weak.text),
                    }),
            );
        }

        column![source_section, instrument_section, date_section]
            .spacing(tokens::spacing::XL)
            .into()
    }

    // ── Execution Tab ────────────────────────────────────────────────

    fn view_execution_tab(&self) -> Element<'_, Message> {
        // Timeframe section (moved from Dataset)
        let timeframe_options: Vec<data::Timeframe> = data::Timeframe::KLINE.to_vec();
        let timeframe_strs: Vec<String> =
            timeframe_options.iter().map(|tf| tf.to_string()).collect();
        let selected_tf = timeframe_strs
            .iter()
            .zip(timeframe_options.iter())
            .find(|(_, tf)| **tf == self.selected_timeframe)
            .map(|(s, _)| s.clone());

        let timeframe_dropdown = pick_list(
            timeframe_strs.clone(),
            selected_tf,
            move |selected: String| {
                let idx = timeframe_strs
                    .iter()
                    .position(|s| s == &selected)
                    .unwrap_or(0);
                Message::TimeframeSelected(timeframe_options[idx])
            },
        )
        .width(Length::Fill);

        let engine_section =
            FormSectionBuilder::new("Engine").push(form_row("Timeframe", timeframe_dropdown));

        // Capital section
        let capital_section = FormSectionBuilder::new("Capital")
            .push(form_row(
                "Initial Capital ($)",
                text_input("100000", &self.initial_capital_str)
                    .on_input(Message::InitialCapitalChanged)
                    .width(Length::Fill),
            ))
            .push(form_row(
                "Commission/Side ($)",
                text_input("2.50", &self.commission_str)
                    .on_input(Message::CommissionChanged)
                    .width(Length::Fill),
            ));

        // Slippage section
        let slippage_options = vec![
            SlippageMode::None,
            SlippageMode::FixedTick,
            SlippageMode::Percentage,
        ];
        let selected_slippage = slippage_options
            .iter()
            .find(|s| **s == self.slippage_mode)
            .copied();

        let mut slippage_section = FormSectionBuilder::new("Slippage").push(form_row(
            "Mode",
            pick_list(
                slippage_options,
                selected_slippage,
                Message::SlippageModeChanged,
            )
            .width(Length::Fill),
        ));

        if self.slippage_mode == SlippageMode::FixedTick {
            slippage_section = slippage_section.push(form_row(
                "Ticks",
                text_input("0", &self.slippage_ticks_str)
                    .on_input(Message::SlippageTicksChanged)
                    .width(Length::Fill),
            ));
        }

        // Position sizing section
        let size_options = vec![
            PositionSizeModeUI::Fixed,
            PositionSizeModeUI::RiskPercent,
            PositionSizeModeUI::RiskDollars,
        ];
        let selected_size = size_options
            .iter()
            .find(|s| **s == self.position_size_mode)
            .copied();

        let size_label = match self.position_size_mode {
            PositionSizeModeUI::Fixed => "Contracts",
            PositionSizeModeUI::RiskPercent => "Risk %",
            PositionSizeModeUI::RiskDollars => "Risk $",
        };

        let sizing_section = FormSectionBuilder::new("Position Sizing")
            .push(form_row(
                "Mode",
                pick_list(
                    size_options,
                    selected_size,
                    Message::PositionSizeModeChanged,
                )
                .width(Length::Fill),
            ))
            .push(form_row(
                size_label,
                text_input("1", &self.position_size_value_str)
                    .on_input(Message::PositionSizeValueChanged)
                    .width(Length::Fill),
            ))
            .push(form_row(
                "Max Concurrent",
                text_input("1", &self.max_concurrent_str)
                    .on_input(Message::MaxConcurrentChanged)
                    .width(Length::Fill),
            ));

        // Risk limits section
        let mut risk_section = FormSectionBuilder::new("Risk Limits").push(
            crate::components::input::toggle_switch::toggle_switch(
                "Max Drawdown Limit",
                self.max_drawdown_enabled,
                Message::MaxDrawdownToggled,
            ),
        );

        if self.max_drawdown_enabled {
            risk_section = risk_section.push(form_row(
                "Max DD %",
                text_input("20", &self.max_drawdown_pct_str)
                    .on_input(Message::MaxDrawdownPctChanged)
                    .width(Length::Fill),
            ));
        }

        // Session section
        let session_section = FormSectionBuilder::new("Session")
            .push(form_row(
                "RTH Open (HHMM)",
                text_input("930", &self.rth_open_str)
                    .on_input(Message::RthOpenChanged)
                    .width(Length::Fill),
            ))
            .push(form_row(
                "RTH Close (HHMM)",
                text_input("1600", &self.rth_close_str)
                    .on_input(Message::RthCloseChanged)
                    .width(Length::Fill),
            ));

        column![
            engine_section,
            capital_section,
            slippage_section,
            sizing_section,
            risk_section,
            session_section,
        ]
        .spacing(tokens::spacing::XL)
        .into()
    }

    // ── Footer ───────────────────────────────────────────────────────

    fn view_footer(&self) -> Element<'_, Message> {
        let error: Element<'_, Message> = if let Some(err) = &self.validation_error {
            text(err.as_str())
                .size(tokens::text::BODY)
                .style(|theme: &iced::Theme| {
                    let p = theme.extended_palette();
                    text::Style {
                        color: Some(p.danger.base.color),
                    }
                })
                .into()
        } else {
            space::vertical().height(0).into()
        };

        let run_label = if self.is_running {
            format!("Running\u{2026} {:.0}%", self.run_progress * 100.0)
        } else {
            "Run Backtest".to_string()
        };

        let can_run = !self.is_running
            && self.selected_connection_idx.is_some()
            && self.selected_ticker.is_some();
        let run_btn = button(
            text(run_label)
                .size(tokens::text::LABEL)
                .align_x(Alignment::Center),
        )
        .width(Length::Fill)
        .on_press_maybe(if can_run {
            Some(Message::RunPressed)
        } else {
            None
        })
        .padding([tokens::spacing::MD, tokens::spacing::XL])
        .style(style::button::primary);

        let cancel_btn = button(
            text("Cancel")
                .size(tokens::text::LABEL)
                .align_x(Alignment::Center),
        )
        .width(Length::Fill)
        .on_press(Message::Close)
        .padding([tokens::spacing::MD, tokens::spacing::XL]);

        column![
            error,
            row![cancel_btn, run_btn].spacing(tokens::spacing::MD),
        ]
        .spacing(tokens::spacing::SM)
        .into()
    }
}

// ── Small view helpers ───────────────────────────────────────────────

fn form_row<'a>(label: &'a str, input: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    row![
        text(label)
            .size(tokens::text::BODY)
            .width(Length::Fixed(tokens::component::form::LABEL_WIDTH)),
        input.into(),
    ]
    .spacing(tokens::spacing::MD)
    .align_y(Alignment::Center)
    .into()
}
