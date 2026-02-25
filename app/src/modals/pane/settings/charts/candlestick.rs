use crate::components::input::color_picker::color_picker;
use crate::components::primitives::label::title;
use crate::screen::dashboard::pane::Message;
use crate::split_column;
use crate::style::tokens;

use data::config::theme::{hsva_to_rgba, rgba_to_hsva};
use data::state::pane::{CandleColorField, CandleStyle, KlineConfig, VisualConfig};

use iced::widget::pane_grid;
use iced::{
    Alignment, Color, Element,
    widget::{button, checkbox, column, container, mouse_area, opaque, row, space, stack, text},
};

use super::super::common::{cfg_view_container, sync_all_button};

pub fn kline_cfg_view<'a>(
    cfg: KlineConfig,
    pane: pane_grid::Pane,
) -> Element<'a, Message> {
    let editing = cfg.editing_color;
    let style = cfg.candle_style.clone();

    let bull_section = {
        let bull_body = color_swatch_row(
            "Body",
            &style,
            CandleColorField::BullBody,
            editing,
            pane,
            &cfg,
        );
        let bull_wick = color_swatch_row(
            "Wick",
            &style,
            CandleColorField::BullWick,
            editing,
            pane,
            &cfg,
        );
        let bull_border = color_swatch_row(
            "Border",
            &style,
            CandleColorField::BullBorder,
            editing,
            pane,
            &cfg,
        );
        column![title("Bullish"), bull_body, bull_wick, bull_border]
            .spacing(tokens::spacing::SM)
    };

    let bear_section = {
        let bear_body = color_swatch_row(
            "Body",
            &style,
            CandleColorField::BearBody,
            editing,
            pane,
            &cfg,
        );
        let bear_wick = color_swatch_row(
            "Wick",
            &style,
            CandleColorField::BearWick,
            editing,
            pane,
            &cfg,
        );
        let bear_border = color_swatch_row(
            "Border",
            &style,
            CandleColorField::BearBorder,
            editing,
            pane,
            &cfg,
        );
        column![title("Bearish"), bear_body, bear_wick, bear_border]
            .spacing(tokens::spacing::SM)
    };

    let has_any_custom = style.bull_body_color.is_some()
        || style.bear_body_color.is_some()
        || style.bull_wick_color.is_some()
        || style.bear_wick_color.is_some()
        || style.bull_border_color.is_some()
        || style.bear_border_color.is_some();

    let reset_all_btn = {
        let cfg_for_reset = cfg.clone();
        button(text("Reset all").size(12)).on_press_maybe(has_any_custom.then(|| {
            let mut new_cfg = cfg_for_reset;
            new_cfg.candle_style = CandleStyle::default();
            new_cfg.editing_color = None;
            Message::VisualConfigChanged(pane, VisualConfig::Kline(new_cfg), false)
        }))
    };

    let rendering_section = {
        let cfg_for_vol = cfg.clone();
        let volume_opacity_toggle = checkbox(cfg.candle_style.volume_opacity)
            .label("Volume opacity")
            .on_toggle(move |checked| {
                let mut new_cfg = cfg_for_vol.clone();
                new_cfg.candle_style.volume_opacity = checked;
                Message::VisualConfigChanged(pane, VisualConfig::Kline(new_cfg), false)
            })
            .size(14)
            .text_size(13);
        column![
            title("Rendering"),
            volume_opacity_toggle,
            text("Candle opacity scales with volume")
                .size(11),
        ]
        .spacing(tokens::spacing::SM)
    };

    let debug_section = {
        let cfg_for_debug = cfg.clone();
        let debug_toggle = checkbox(cfg.show_debug_info)
            .label("Show debug info")
            .on_toggle(move |checked| {
                let mut new_cfg = cfg_for_debug.clone();
                new_cfg.show_debug_info = checked;
                Message::VisualConfigChanged(
                    pane,
                    VisualConfig::Kline(new_cfg),
                    false,
                )
            })
            .size(14)
            .text_size(13);
        column![
            title("Debug"),
            debug_toggle,
            text("FPS, frame time, visible candles, LOD level")
                .size(11),
        ]
        .spacing(tokens::spacing::SM)
    };

    let buttons_row = row![
        space::horizontal(),
        reset_all_btn,
        sync_all_button(pane, VisualConfig::Kline(cfg.clone())),
    ]
    .spacing(tokens::spacing::SM)
    .align_y(Alignment::Center);

    let compact_col = split_column![
        bull_section,
        bear_section,
        rendering_section,
        debug_section,
        buttons_row
        ; spacing = tokens::spacing::LG, align_x = Alignment::Start
    ];

    if let Some(field) = editing {
        let current_color = style.get_color(field);
        let display_rgba = current_color
            .unwrap_or_else(|| crate::style::theme::iced_color_to_rgba(default_for_field(field)));
        let hsva = rgba_to_hsva(display_rgba);

        let cfg_for_picker = cfg.clone();
        let picker = color_picker(
            hsva,
            move |new_hsva| {
                let new_color = hsva_to_rgba(new_hsva);
                let mut new_style = cfg_for_picker.candle_style.clone();
                new_style.set_color(field, Some(new_color));
                let mut new_cfg = cfg_for_picker.clone();
                new_cfg.candle_style = new_style;
                new_cfg.editing_color = Some(field);
                Message::VisualConfigChanged(pane, VisualConfig::Kline(new_cfg), false)
            },
            180.0,
        );

        let label = match field {
            CandleColorField::BullBody => "Bullish Body",
            CandleColorField::BearBody => "Bearish Body",
            CandleColorField::BullWick => "Bullish Wick",
            CandleColorField::BearWick => "Bearish Wick",
            CandleColorField::BullBorder => "Bullish Border",
            CandleColorField::BearBorder => "Bearish Border",
        };

        let dismiss = {
            let mut new_cfg = cfg.clone();
            new_cfg.editing_color = None;
            Message::VisualConfigChanged(pane, VisualConfig::Kline(new_cfg), false)
        };

        let popup = container(
            column![text(label).size(tokens::text::LABEL), picker,]
                .spacing(tokens::spacing::SM),
        )
        .padding(tokens::spacing::LG)
        .style(crate::style::dropdown_container);

        cfg_view_container(
            320,
            stack![mouse_area(compact_col).on_press(dismiss), opaque(popup),],
        )
    } else {
        cfg_view_container(320, compact_col)
    }
}

/// A row with a label + small color swatch button.
/// Clicking the swatch toggles the color picker for that field.
fn color_swatch_row<'a>(
    label: &'a str,
    style: &CandleStyle,
    field: CandleColorField,
    editing: Option<CandleColorField>,
    pane: pane_grid::Pane,
    cfg: &KlineConfig,
) -> Element<'a, Message> {
    let current_color = style.get_color(field);
    let display_rgba = current_color
        .unwrap_or_else(|| crate::style::theme::iced_color_to_rgba(default_for_field(field)));
    let is_active = editing == Some(field);
    let is_custom = current_color.is_some();

    let swatch = button(space::horizontal().width(24).height(16))
        .style(move |_theme, _status| button::Style {
            background: Some(iced::Background::Color(
                crate::style::theme::rgba_to_iced_color(display_rgba),
            )),
            border: iced::border::rounded(3)
                .width(if is_active { 2.0 } else { 1.0 })
                .color(if is_active {
                    Color::WHITE
                } else {
                    Color::from_rgba(1.0, 1.0, 1.0, 0.3)
                }),
            ..button::Style::default()
        })
        .padding(0)
        .on_press({
            let new_editing = if is_active { None } else { Some(field) };
            let mut new_cfg = cfg.clone();
            new_cfg.editing_color = new_editing;
            Message::VisualConfigChanged(pane, VisualConfig::Kline(new_cfg), false)
        });

    let label_text = text(label).size(13);
    let status = if is_custom {
        text("custom").size(11)
    } else {
        text("theme").size(11)
    };

    row![label_text, space::horizontal(), status, swatch]
        .spacing(tokens::spacing::SM)
        .align_y(Alignment::Center)
        .into()
}

/// Default color for each field (matches the original hardcoded palette colors).
fn default_for_field(field: CandleColorField) -> Color {
    match field {
        CandleColorField::BullBody | CandleColorField::BullWick => {
            Color::from_rgb(0.2, 0.8, 0.2) // green (success)
        }
        CandleColorField::BearBody | CandleColorField::BearWick => {
            Color::from_rgb(0.9, 0.2, 0.2) // red (danger)
        }
        CandleColorField::BullBorder | CandleColorField::BearBorder => Color::TRANSPARENT,
    }
}
