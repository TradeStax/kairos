//! Content body dispatch for the pane view.

use crate::config::UserTimezone;
use crate::screen::dashboard::pane::config::ContentKind;
use crate::{
    components::display::empty_state::EmptyStateBuilder,
    components::display::progress_bar::ProgressBarBuilder,
    components::primitives::{Icon, label::*},
    style::{palette, tokens},
};
use data::{FuturesTicker, FuturesTickerInfo};
use iced::widget::pane_grid;
use iced::{
    Alignment, Element, Length,
    widget::{center, column, text},
};
use rustc_hash::FxHashMap;

use super::super::{Content, Message, State};
use super::assistant;
#[cfg(feature = "heatmap")]
use super::helpers;
use super::modal_stack::CompactControls;
use super::modals::stream::Modifier;
#[cfg(feature = "heatmap")]
use crate::modals;
#[cfg(feature = "heatmap")]
use crate::screen::dashboard::ladder;
#[cfg(feature = "heatmap")]
use crate::screen::dashboard::pane::Event;

/// Build the `uninitialized_base` closure content — shows loading/error/empty states.
pub(super) fn uninitialized_base<'a>(
    kind: ContentKind,
    loading_status: &'a data::LoadingStatus,
) -> Element<'a, Message> {
    if loading_status.is_loading() {
        let (status_text, progress_value, progress_max) = match loading_status {
            data::LoadingStatus::Downloading {
                schema,
                days_complete,
                days_total,
                ..
            } => (
                format!("Downloading {} ({}/{})", schema, days_complete, days_total),
                Some(*days_complete as f32),
                Some(*days_total as f32),
            ),
            data::LoadingStatus::LoadingFromCache {
                schema,
                days_loaded,
                days_total,
                progress_fraction,
                ..
            } => {
                let (val, max) = if let Some(pf) = progress_fraction {
                    (*pf * 100.0, 100.0_f32)
                } else {
                    (*days_loaded as f32, *days_total as f32)
                };
                (
                    format!("Loading {} ({}/{})", schema, days_loaded, days_total),
                    Some(val),
                    Some(max),
                )
            }
            data::LoadingStatus::Building {
                operation,
                progress,
            } => (
                format!("{} ({:.0}%)", operation, progress * 100.0),
                Some(*progress * 100.0),
                Some(100.0_f32),
            ),
            _ => ("Loading\u{2026}".to_string(), None, None),
        };

        let content: Element<'a, Message> =
            if let (Some(val), Some(max)) = (progress_value, progress_max) {
                column![
                    text(status_text.clone()).size(tokens::text::SMALL),
                    ProgressBarBuilder::<Message>::new(val, max)
                        .show_percentage(false)
                        .into_element()
                ]
                .spacing(tokens::spacing::XS)
                .width(Length::Fixed(240.0))
                .align_x(Alignment::Center)
                .into()
            } else {
                heading(status_text).into()
            };
        center(content).into()
    } else if let data::LoadingStatus::Error { message } = loading_status {
        let content = column![
            text(char::from(Icon::Close).to_string())
                .font(crate::components::primitives::ICONS_FONT)
                .size(tokens::component::icon::EMPTY_STATE)
                .style(palette::error_text),
            heading(kind.to_string()),
            title(message)
        ]
        .spacing(tokens::spacing::MD)
        .align_x(Alignment::Center);

        center(content).into()
    } else {
        center(
            EmptyStateBuilder::new("Select a ticker to start charting")
                .icon(Icon::ChartOutline)
                .into_element(),
        )
        .into()
    }
}

/// Dispatch content body based on pane content type.
///
/// Returns the body element plus any extras to be pushed to the stream-info row.
pub(super) fn dispatch_body<'a>(
    state: &'a State,
    id: pane_grid::Pane,
    modifier: Option<Modifier>,
    compact_controls: CompactControls<'a>,
    timezone: UserTimezone,
    tickers_info: &'a FxHashMap<FuturesTicker, FuturesTickerInfo>,
    ticker_ranges: &'a std::collections::HashMap<String, String>,
) -> (Element<'a, Message>, Vec<Element<'a, Message>>) {
    match &state.content {
        Content::Starter => {
            let body = state.view_starter_body(id, compact_controls, tickers_info, ticker_ranges);
            (body, vec![])
        }
        Content::Comparison(chart) => {
            let (body, extras) = state.view_comparison_body(
                id,
                chart,
                modifier,
                compact_controls,
                |kind| uninitialized_base(kind, &state.loading_status),
                timezone,
                tickers_info,
                ticker_ranges,
            );
            (body, extras)
        }
        #[cfg(feature = "heatmap")]
        Content::Ladder(panel) => {
            if let Some(panel) = panel {
                let basis = state
                    .settings
                    .selected_basis
                    .unwrap_or(data::ChartBasis::Time(data::Timeframe::M5));

                let kind = modals::ModifierKind::Orderbook(basis);
                let modifiers = helpers::basis_modifier(id, basis, modifier, kind);
                let extra = vec![modifiers];

                let base = ladder::view(panel, timezone).map(move |message| {
                    Message::PaneEvent(id, Box::new(Event::PanelInteraction(message)))
                });

                let settings_modal =
                    || modals::pane::settings::ladder_cfg_view(panel.config.clone(), id);

                let body = state.compose_stack_view(
                    base,
                    id,
                    compact_controls,
                    settings_modal,
                    None,
                    tickers_info,
                    ticker_ranges,
                );
                (body, extra)
            } else {
                let base = uninitialized_base(ContentKind::Ladder, &state.loading_status);
                let body = state.compose_stack_view(
                    base,
                    id,
                    compact_controls,
                    || column![].into(),
                    None,
                    tickers_info,
                    ticker_ranges,
                );
                (body, vec![])
            }
        }
        #[cfg(feature = "heatmap")]
        Content::Heatmap {
            chart,
            indicators,
            studies: _,
            ..
        } => {
            let (body, extras) = state.view_heatmap_body(
                id,
                chart,
                indicators,
                modifier,
                compact_controls,
                |kind| uninitialized_base(kind, &state.loading_status),
                timezone,
                tickers_info,
                ticker_ranges,
            );
            (body, extras)
        }
        Content::Candlestick { chart, .. } => {
            let (body, extras) = state.view_kline_body(
                id,
                chart,
                modifier,
                compact_controls,
                |kind| uninitialized_base(kind, &state.loading_status),
                timezone,
                tickers_info,
                ticker_ranges,
            );
            (body, extras)
        }
        Content::Profile { chart, .. } => {
            let (body, extras) = state.view_profile_body(
                id,
                chart,
                modifier,
                compact_controls,
                |kind| uninitialized_base(kind, &state.loading_status),
                timezone,
                tickers_info,
                ticker_ranges,
            );
            (body, extras)
        }
        Content::AiAssistant(ai_state) => {
            let base = assistant::view_assistant(ai_state, id, state.link_group.is_some());
            let body = state.compose_stack_view(
                base,
                id,
                compact_controls,
                || column![].into(),
                None,
                tickers_info,
                ticker_ranges,
            );
            (body, vec![])
        }
    }
}
