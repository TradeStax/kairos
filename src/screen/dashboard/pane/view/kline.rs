use crate::{
    chart,
    modals::{self, ModifierKind},
    screen::dashboard::pane::view::CompactControls,
    style::tokens,
};
use data::{ChartBasis, ContentKind, Timeframe, UserTimezone};
use exchange::{FuturesTicker, FuturesTickerInfo};
use iced::{
    Element,
    widget::{column, row},
};
use rustc_hash::FxHashMap;

use super::super::{Event, Message, State};
use super::helpers::basis_modifier;

impl State {
    /// Build the Kline (candlestick / footprint) chart content view.
    ///
    /// Returns `(body, extra_title_elements)` where `extra_title_elements` are
    /// widgets to push onto the title-bar row.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn view_kline_body<'a>(
        &'a self,
        id: iced::widget::pane_grid::Pane,
        chart_opt: &'a Option<chart::candlestick::KlineChart>,
        modifier: Option<modals::stream::Modifier>,
        compact_controls: CompactControls<'a>,
        uninitialized_base: impl FnOnce(ContentKind) -> Element<'a, Message>,
        timezone: UserTimezone,
        tickers_info: &'a FxHashMap<FuturesTicker, FuturesTickerInfo>,
    ) -> (Element<'a, Message>, Vec<Element<'a, Message>>) {
        let mut extra = Vec::new();

        if let Some(chart) = chart_opt {
            let selected_basis = self
                .settings
                .selected_basis
                .unwrap_or(ChartBasis::Time(Timeframe::M5));

            let kind = if chart.footprint.is_some() {
                ModifierKind::Footprint(selected_basis)
            } else {
                ModifierKind::Candlestick(selected_basis)
            };

            let modifiers: Element<'a, Message> =
                row![basis_modifier(id, selected_basis, modifier, kind),]
                    .spacing(tokens::spacing::XS)
                    .into();
            extra.push(modifiers);

            let base = chart::view(chart, timezone)
                .map(move |message| Message::PaneEvent(id, Event::ChartInteraction(message)));
            let settings_modal = || {
                // Read candle style from the chart's current config
                let cfg = if let Some(data::VisualConfig::Kline(ref saved)) =
                    self.settings.visual_config
                {
                    saved.clone()
                } else {
                    data::state::pane::KlineConfig {
                        candle_style: chart.candle_style().clone(),
                        ..Default::default()
                    }
                };
                modals::pane::settings::kline_cfg_view(cfg, chart.footprint_config().cloned(), id)
            };

            let body = self.compose_stack_view(
                base,
                id,
                compact_controls,
                settings_modal,
                None,
                tickers_info,
            );
            (body, extra)
        } else {
            let base = uninitialized_base(ContentKind::CandlestickChart);
            let body = self.compose_stack_view(
                base,
                id,
                compact_controls,
                || column![].into(),
                None,
                tickers_info,
            );
            (body, extra)
        }
    }
}
