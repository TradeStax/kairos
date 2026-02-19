use crate::{
    chart,
    modal::{self, ModifierKind, pane::Modal},
    screen::dashboard::pane::view::CompactControls,
};
use data::{ChartBasis, ContentKind, Timeframe, UserTimezone};
use exchange::{FuturesTicker, FuturesTickerInfo};
use iced::{Element, widget::column};
use rustc_hash::FxHashMap;

use super::helpers::basis_modifier;
use super::super::{Event, Message, State};

impl State {
    /// Build the Heatmap chart content view.
    ///
    /// Returns `(body, extra_title_elements)` where `extra_title_elements` are
    /// widgets to push onto the title-bar row.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn view_heatmap_body<'a>(
        &'a self,
        id: iced::widget::pane_grid::Pane,
        chart_opt: &'a Option<chart::heatmap::HeatmapChart>,
        indicators: &'a [data::HeatmapIndicator],
        modifier: Option<modal::stream::Modifier>,
        compact_controls: CompactControls<'a>,
        uninitialized_base: impl FnOnce(ContentKind) -> Element<'a, Message>,
        timezone: UserTimezone,
        tickers_info: &'a FxHashMap<FuturesTicker, FuturesTickerInfo>,
    ) -> (Element<'a, Message>, Vec<Element<'a, Message>>) {
        let mut extra = Vec::new();

        if let Some(chart) = chart_opt {
            let ticker_info = self.ticker_info;
            let _exchange = ticker_info.as_ref().map(|info| info.ticker.venue);

            let basis = self
                .settings
                .selected_basis
                .unwrap_or(ChartBasis::Time(Timeframe::M5));

            let kind = ModifierKind::Heatmap(basis);

            // Tick multiplier removed - only for crypto
            let modifiers: Element<'a, Message> = basis_modifier(id, basis, modifier, kind);

            extra.push(modifiers);

            let base = chart::view(chart, indicators, timezone)
                .map(move |message| Message::PaneEvent(id, Event::ChartInteraction(message)));
            let settings_modal = || {
                // Convert chart::heatmap::VisualConfig to data::HeatmapConfig
                let visual = chart.visual_config();
                let cfg = data::state::pane_config::HeatmapConfig {
                    trade_size_filter: visual.trade_size_filter,
                    order_size_filter: visual.order_size_filter,
                    trade_size_scale: visual.trade_size_scale,
                    coalescing: None, // CoalesceKind is not exposed, use None
                    rendering_mode: data::state::pane_config::HeatmapRenderMode::Auto,
                    max_trade_markers: visual.max_trade_markers,
                    performance_preset: None,
                };
                // Convert chart::heatmap::HeatmapStudy to data studies and leak
                // for 'static lifetime
                let data_studies: Vec<data::domain::chart_ui_types::heatmap::HeatmapStudy> = chart
                    .studies
                    .iter()
                    .map(|s| match s {
                        crate::chart::heatmap::HeatmapStudy::VolumeProfile(kind) => {
                            data::domain::chart_ui_types::heatmap::HeatmapStudy::VolumeProfile(
                                *kind,
                            )
                        }
                    })
                    .collect();
                // Use Box::leak to create a static reference
                let studies_static: &'static [_] = Box::leak(data_studies.into_boxed_slice());
                modal::pane::settings::heatmap_cfg_view(
                    cfg,
                    id,
                    chart.study_configurator(),
                    studies_static,
                    basis,
                )
            };

            let indicator_modal = if self.modal == Some(Modal::Indicators) {
                Some(modal::pane::indicators::content_row_heatmap(
                    id, indicators, false, // Heatmap doesn't allow dragging
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
            let base = uninitialized_base(ContentKind::HeatmapChart);
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
