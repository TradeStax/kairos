//! Right panel: tab bar and detail views (Overview, Touches, Flow).

use iced::{
    Alignment, Color, Element, Length,
    widget::{button, column, container, row, scrollable},
};

use study::orderflow::level_analyzer::types::MonitoredLevel;

use crate::components::display::status_dot::status_badge_themed;
use crate::components::layout::section_header::SectionHeaderBuilder;
use crate::components::primitives;
use crate::style;
use crate::style::{palette, tokens};

use super::{
    DetailTab, LevelDetailModal, Message, fmt_delta, fmt_duration_ms, fmt_volume, status_color,
    status_label,
};

impl LevelDetailModal {
    pub(super) fn view_right_panel(&self) -> Element<'_, Message> {
        let tab_bar = self.view_tab_bar();

        let content: Element<'_, Message> = match self.selected_level() {
            None => container(primitives::label_text("Select a level to view details"))
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .into(),
            Some(level) => match self.active_tab {
                DetailTab::Overview => view_overview(level),
                DetailTab::Touches => view_touches(level),
                DetailTab::Flow => view_flow(level),
            },
        };

        column![tab_bar, content]
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn view_tab_bar(&self) -> Element<'_, Message> {
        let tabs = [
            (DetailTab::Overview, "Overview"),
            (DetailTab::Touches, "Touches"),
            (DetailTab::Flow, "Flow"),
        ];

        let mut tab_row = row![].spacing(tokens::spacing::XXS);
        for (tab, label) in tabs {
            let is_active = self.active_tab == tab;
            let btn = button(primitives::small(label))
                .on_press(Message::SwitchTab(tab))
                .padding([tokens::spacing::XS, tokens::spacing::MD]);

            tab_row = if is_active {
                tab_row.push(btn.style(style::button::primary))
            } else {
                tab_row.push(
                    btn.style(|theme, status| style::button::transparent(theme, status, false)),
                )
            };
        }

        container(tab_row)
            .padding([tokens::spacing::SM, tokens::spacing::LG])
            .width(Length::Fill)
            .into()
    }
}

const KEY_W: f32 = 100.0;
const FLOW_KEY_W: f32 = 110.0;

/// A key-value row using owned strings (avoids lifetime issues).
fn kv_row<'a>(
    key: &'static str,
    value: String,
    key_width: f32,
    mono: bool,
    color: Option<Color>,
) -> Element<'a, Message> {
    let key_el = primitives::small(key).width(Length::Fixed(key_width));

    let mut val_el = if mono {
        primitives::mono(value)
    } else {
        primitives::small(value)
    };

    if let Some(c) = color {
        val_el = val_el.color(c);
    }

    row![key_el, val_el].spacing(tokens::spacing::XS).into()
}

/// Overview tab: identity + metrics sections.
fn view_overview(level: &MonitoredLevel) -> Element<'_, Message> {
    let status = level.status;
    let status_badge: Element<'_, Message> =
        status_badge_themed(status_color(status), status_label(status));

    let hold_rate = if level.touch_count > 0 {
        format!(
            "{:.0}%",
            level.hold_count as f64 / level.touch_count as f64 * 100.0
        )
    } else {
        "-".into()
    };

    let strength_str = if level.strength > 0.0 {
        format!("{:.0}%", level.strength * 100.0)
    } else {
        "-".into()
    };

    let delta_val = level.net_delta;
    let delta_color = if delta_val > 0.0 {
        Some(palette::success_color(&iced::Theme::Dark))
    } else if delta_val < 0.0 {
        Some(palette::error_color(&iced::Theme::Dark))
    } else {
        None
    };

    let block_buy_count = level
        .touches
        .iter()
        .flat_map(|t| &t.blocks)
        .filter(|b| b.is_buy)
        .count();
    let block_sell_count = level
        .touches
        .iter()
        .flat_map(|t| &t.blocks)
        .filter(|b| !b.is_buy)
        .count();
    let total_blocks = block_buy_count + block_sell_count;
    let block_vol = level.flow.block_buy_volume + level.flow.block_sell_volume;

    let session_str = {
        let tag = level.session_key.short_tag();
        if tag.is_empty() {
            level.session_key.session_type.to_string()
        } else {
            tag
        }
    };

    let identity = column![
        kv_row("Price", format!("{:.2}", level.price), KEY_W, true, None),
        kv_row(
            "Source",
            level.source.label().to_string(),
            KEY_W,
            false,
            None
        ),
        kv_row("Session", session_str, KEY_W, false, None),
        row![
            primitives::small("Status").width(Length::Fixed(KEY_W)),
            status_badge,
        ]
        .spacing(tokens::spacing::XS)
        .align_y(Alignment::Center),
    ]
    .spacing(tokens::spacing::SM);

    let metrics = column![
        SectionHeaderBuilder::new("Metrics").with_divider(true),
        kv_row("Strength", strength_str, KEY_W, true, None),
        kv_row("Touches", level.touch_count.to_string(), KEY_W, true, None),
        kv_row("Hold Rate", hold_rate, KEY_W, true, None),
        kv_row(
            "Break Count",
            level.break_count.to_string(),
            KEY_W,
            true,
            None
        ),
        kv_row(
            "Volume",
            fmt_volume(level.total_volume_absorbed),
            KEY_W,
            true,
            None
        ),
        kv_row("Net Delta", fmt_delta(delta_val), KEY_W, true, delta_color),
        kv_row(
            "Time at Level",
            fmt_duration_ms(level.time_at_level),
            KEY_W,
            true,
            None
        ),
        kv_row(
            "Absorption",
            format!("{:.1}x", level.flow.absorption_ratio),
            KEY_W,
            true,
            None
        ),
        kv_row(
            "Blocks",
            format!("{total_blocks} ({block_buy_count}B / {block_sell_count}S)"),
            KEY_W,
            true,
            None,
        ),
        kv_row("Block Volume", fmt_volume(block_vol), KEY_W, true, None),
    ]
    .spacing(tokens::spacing::SM);

    scrollable(
        column![identity, metrics]
            .spacing(tokens::spacing::LG)
            .padding([tokens::spacing::SM, tokens::spacing::LG]),
    )
    .height(Length::Fill)
    .into()
}

/// Touches tab: list of touch events, most recent first.
fn view_touches(level: &MonitoredLevel) -> Element<'_, Message> {
    if level.touches.is_empty() {
        return container(primitives::small("No touch events yet"))
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .into();
    }

    let total = level.touches.len();
    let items = level.touches.iter().rev().enumerate().fold(
        column![].spacing(tokens::spacing::SM),
        |col, (rev_idx, touch)| {
            let num = total - rev_idx;
            col.push(view_touch_item(num, touch))
        },
    );

    scrollable(items.padding([tokens::spacing::SM, tokens::spacing::LG]))
        .height(Length::Fill)
        .into()
}

/// Single touch event block.
fn view_touch_item<'a>(
    num: usize,
    touch: &study::orderflow::level_analyzer::types::TouchEvent,
) -> Element<'a, Message> {
    let held_label = if touch.held { "Hold" } else { "Break" };
    let quality = format!("q:{:.2}", touch.quality_score);

    let start_secs = touch.start_time / 1_000;
    let end_secs = touch.end_time / 1_000;
    let start_hms = format_hms(start_secs);
    let end_hms = format_hms(end_secs);

    let header = row![
        primitives::small(format!("#{num}")),
        primitives::small(held_label),
        primitives::small(quality),
        primitives::tiny(format!("{start_hms} \u{2013} {end_hms}")),
    ]
    .spacing(tokens::spacing::SM)
    .align_y(Alignment::Center);

    let vol_str = fmt_volume(touch.volume);
    let delta_str = fmt_delta(touch.delta);
    let buy_str = fmt_volume(touch.buy_volume);
    let sell_str = fmt_volume(touch.sell_volume);

    let detail = primitives::tiny(format!(
        "Vol {vol_str}  \u{0394} {delta_str}  Buy {buy_str} / Sell {sell_str}"
    ));

    let mut item = column![header, detail].spacing(tokens::spacing::XXS);

    if !touch.blocks.is_empty() {
        let buy_blocks: Vec<_> = touch.blocks.iter().filter(|b| b.is_buy).collect();
        let sell_blocks: Vec<_> = touch.blocks.iter().filter(|b| !b.is_buy).collect();

        let buy_lots: f64 = buy_blocks.iter().map(|b| b.quantity).sum();
        let sell_lots: f64 = sell_blocks.iter().map(|b| b.quantity).sum();

        let block_str = format!(
            "Blocks: {}B ({}) / {}S ({})",
            buy_blocks.len(),
            fmt_volume(buy_lots),
            sell_blocks.len(),
            fmt_volume(sell_lots),
        );
        item = item.push(primitives::tiny(block_str));
    }

    container(item)
        .padding([tokens::spacing::XS, tokens::spacing::SM])
        .width(Length::Fill)
        .style(|theme: &iced::Theme| {
            let p = theme.extended_palette();
            iced::widget::container::Style {
                background: Some(p.background.weak.color.into()),
                border: iced::Border {
                    radius: 4.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        })
        .into()
}

/// Flow tab: volume breakdown, block analysis, absorption.
fn view_flow(level: &MonitoredLevel) -> Element<'_, Message> {
    let flow = &level.flow;

    let total_vol = flow.buy_volume + flow.sell_volume;
    let ratio_str = if flow.sell_volume > 0.0 {
        format!("{:.2}", flow.buy_volume / flow.sell_volume)
    } else {
        "-".into()
    };

    let abs = flow.absorption_ratio;
    let interpretation = if abs > 2.0 {
        "Strong defense"
    } else if abs >= 1.0 {
        "Moderate"
    } else {
        "Neutral"
    };

    let volume_section = column![
        SectionHeaderBuilder::new("Volume").with_divider(true),
        kv_row(
            "Buy Volume",
            fmt_volume(flow.buy_volume),
            FLOW_KEY_W,
            true,
            None
        ),
        kv_row(
            "Sell Volume",
            fmt_volume(flow.sell_volume),
            FLOW_KEY_W,
            true,
            None
        ),
        kv_row(
            "Total Volume",
            fmt_volume(total_vol),
            FLOW_KEY_W,
            true,
            None
        ),
        kv_row("Buy/Sell Ratio", ratio_str, FLOW_KEY_W, true, None),
    ]
    .spacing(tokens::spacing::SM);

    let block_section = column![
        SectionHeaderBuilder::new("Block Analysis").with_divider(true),
        kv_row(
            "Block Count",
            flow.block_count.to_string(),
            FLOW_KEY_W,
            true,
            None
        ),
        kv_row(
            "Block Buy Vol",
            fmt_volume(flow.block_buy_volume),
            FLOW_KEY_W,
            true,
            None
        ),
        kv_row(
            "Block Sell Vol",
            fmt_volume(flow.block_sell_volume),
            FLOW_KEY_W,
            true,
            None
        ),
    ]
    .spacing(tokens::spacing::SM);

    let absorption_section = column![
        SectionHeaderBuilder::new("Absorption").with_divider(true),
        kv_row(
            "Absorption Ratio",
            format!("{abs:.2}x"),
            FLOW_KEY_W,
            true,
            None
        ),
        kv_row(
            "Interpretation",
            interpretation.to_string(),
            FLOW_KEY_W,
            false,
            None
        ),
    ]
    .spacing(tokens::spacing::SM);

    scrollable(
        column![volume_section, block_section, absorption_section]
            .spacing(tokens::spacing::LG)
            .padding([tokens::spacing::SM, tokens::spacing::LG]),
    )
    .height(Length::Fill)
    .into()
}

/// Format epoch seconds to HH:MM:SS.
fn format_hms(epoch_secs: u64) -> String {
    let h = (epoch_secs % 86400) / 3600;
    let m = (epoch_secs % 3600) / 60;
    let s = epoch_secs % 60;
    format!("{h:02}:{m:02}:{s:02}")
}
