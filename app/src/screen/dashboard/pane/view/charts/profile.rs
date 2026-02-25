use crate::{
    chart,
    modals::{self, pane::Modal},
    screen::dashboard::pane::view::CompactControls,
};
use data::{ContentKind, UserTimezone};
use exchange::{FuturesTicker, FuturesTickerInfo};
use iced::{
    Element,
    widget::column,
};
use rustc_hash::FxHashMap;

use super::super::super::{Event, Message, State};

impl State {
    /// Build the Profile chart content view.
    ///
    /// Returns `(body, extra_title_elements)`.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn view_profile_body<'a>(
        &'a self,
        id: iced::widget::pane_grid::Pane,
        chart_opt: &'a Option<chart::profile::ProfileChart>,
        _modifier: Option<modals::stream::Modifier>,
        compact_controls: CompactControls<'a>,
        uninitialized_base: impl FnOnce(ContentKind) -> Element<'a, Message>,
        timezone: UserTimezone,
        tickers_info: &'a FxHashMap<FuturesTicker, FuturesTickerInfo>,
        ticker_ranges: &'a std::collections::HashMap<String, String>,
    ) -> (Element<'a, Message>, Vec<Element<'a, Message>>) {
        let extra: Vec<Element<'a, Message>> = Vec::new();

        if let Some(chart) = chart_opt {
            let base = chart::view(chart, timezone)
                .map(move |message| {
                    Message::PaneEvent(
                        id,
                        Event::ChartInteraction(message),
                    )
                });

            // Pass empty closure — profile handles settings
            // separately via main_dialog_modal below.
            let body = self.compose_stack_view(
                base,
                id,
                compact_controls,
                || column![].into(),
                None,
                tickers_info,
                ticker_ranges,
            );

            // Layer centered settings dialog when active
            let body =
                if matches!(self.modal, Some(Modal::Settings)) {
                    let cfg = if let Some(
                        data::VisualConfig::Profile(ref saved),
                    ) = self.settings.visual_config
                    {
                        saved.clone()
                    } else {
                        chart.display_config().clone()
                    };
                    let content =
                        modals::pane::settings::profile_cfg_view(
                            cfg, id,
                        );
                    crate::modals::main_dialog_modal(
                        body,
                        content,
                        Message::PaneEvent(
                            id,
                            Event::HideModal,
                        ),
                    )
                } else {
                    body
                };

            (body, extra)
        } else {
            let base =
                uninitialized_base(ContentKind::ProfileChart);
            let body = self.compose_stack_view(
                base,
                id,
                compact_controls,
                || column![].into(),
                None,
                tickers_info,
                ticker_ranges,
            );
            (body, extra)
        }
    }
}
