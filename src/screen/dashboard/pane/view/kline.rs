use crate::{
    chart,
    modal::{self, ModifierKind, pane::Modal},
    screen::dashboard::pane::view::CompactControls,
    style::tokens,
};
use data::{ChartBasis, ContentKind, KlineIndicator, Timeframe, UserTimezone};
use exchange::{FuturesTicker, FuturesTickerInfo};
use iced::{
    Element,
    widget::{column, row},
};
use rustc_hash::FxHashMap;

use super::helpers::basis_modifier;
use super::super::{Event, Message, State};

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
        indicators: &'a [KlineIndicator],
        chart_kind: &'a data::KlineChartKind,
        modifier: Option<modal::stream::Modifier>,
        compact_controls: CompactControls<'a>,
        uninitialized_base: impl FnOnce(ContentKind) -> Element<'a, Message>,
        timezone: UserTimezone,
        tickers_info: &'a FxHashMap<FuturesTicker, FuturesTickerInfo>,
    ) -> (Element<'a, Message>, Vec<Element<'a, Message>>) {
        let mut extra = Vec::new();

        if let Some(chart) = chart_opt {
            match chart_kind {
                data::KlineChartKind::Footprint { .. } => {
                    let basis = self
                        .settings
                        .selected_basis
                        .unwrap_or(ChartBasis::Time(Timeframe::M5));
                    let kind = ModifierKind::Footprint(basis);

                    // Tick multiplier removed - only for crypto
                    let modifiers: Element<'a, Message> =
                        basis_modifier(id, basis, modifier, kind);

                    extra.push(modifiers);
                }
                data::KlineChartKind::Candles => {
                    let selected_basis = self
                        .settings
                        .selected_basis
                        .unwrap_or(ChartBasis::Time(Timeframe::M5));
                    let kind = ModifierKind::Candlestick(selected_basis);

                    let modifiers: Element<'a, Message> =
                        row![basis_modifier(id, selected_basis, modifier, kind),]
                            .spacing(tokens::spacing::XS)
                            .into();

                    extra.push(modifiers);
                }
            }

            let base = chart::view(chart, indicators, timezone)
                .map(move |message| Message::PaneEvent(id, Event::ChartInteraction(message)));
            let settings_modal = || {
                // Read candle style from the chart's current config
                let cfg = if let Some(data::VisualConfig::Kline(ref saved)) =
                    self.settings.visual_config
                {
                    saved.clone()
                } else {
                    let mut default = data::state::pane_config::KlineConfig::default();
                    default.candle_style = chart.candle_style().clone();
                    default
                };
                modal::pane::settings::kline_cfg_view(
                    cfg,
                    chart.study_configurator(),
                    chart_kind,
                    id,
                    chart.basis(),
                )
            };

            let indicator_modal = if self.modal == Some(Modal::Indicators) {
                Some(modal::pane::indicators::content_row_kline(
                    id, indicators, true, // Kline allows dragging
                ))
            } else {
                None
            };

            let body = self.compose_stack_view(
                base,
                id,
                indicator_modal,
                compact_controls,
                settings_modal,
                None,
                tickers_info,
            );
            (body, extra)
        } else {
            let content_kind = match chart_kind {
                data::KlineChartKind::Candles => ContentKind::CandlestickChart,
                data::KlineChartKind::Footprint { .. } => ContentKind::FootprintChart,
            };
            let base = uninitialized_base(content_kind);
            let body = self.compose_stack_view(
                base,
                id,
                None,
                compact_controls,
                || column![].into(),
                None,
                tickers_info,
            );
            (body, extra)
        }
    }
}
