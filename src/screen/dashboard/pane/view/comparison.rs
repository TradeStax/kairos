use crate::{
    modal::{self, ModifierKind},
    screen::dashboard::{pane::view::CompactControls, tickers_table::TickersTable},
    style::tokens,
};
use data::{ChartBasis, ContentKind, Timeframe, UserTimezone};
use iced::{
    Element,
    widget::{column, row},
};

use super::super::helpers::basis_modifier;
use super::super::{Event, Message, State};

impl State {
    /// Build the Comparison chart content view.
    ///
    /// Returns `(body, extra_title_elements)` where `extra_title_elements` are
    /// widgets to push onto the title-bar row.
    pub(crate) fn view_comparison_body<'a>(
        &'a self,
        id: iced::widget::pane_grid::Pane,
        chart: &'a Option<crate::chart::comparison::ComparisonChart>,
        modifier: Option<modal::stream::Modifier>,
        compact_controls: CompactControls<'a>,
        uninitialized_base: impl FnOnce(ContentKind) -> Element<'a, Message>,
        timezone: UserTimezone,
        tickers_table: &'a TickersTable,
    ) -> (Element<'a, Message>, Vec<Element<'a, Message>>) {
        let mut extra = Vec::new();

        if let Some(c) = chart {
            let selected_basis = self
                .settings
                .selected_basis
                .unwrap_or(ChartBasis::Time(Timeframe::M15));
            let kind = ModifierKind::Comparison(selected_basis);

            let modifiers: Element<'a, Message> =
                row![basis_modifier(id, selected_basis, modifier, kind),]
                    .spacing(tokens::spacing::XS)
                    .into();

            extra.push(modifiers);

            let base = c.view(timezone).map(move |message| {
                Message::PaneEvent(id, Event::ComparisonChartInteraction(message))
            });

            let settings_modal = || modal::pane::settings::comparison_cfg_view(id, c);
            let selected_tickers = c.selected_tickers();
            // Use Box::leak to create a static reference for the title bar
            let selected_tickers_static: &'static [_] =
                Box::leak(selected_tickers.into_boxed_slice());

            let body = self.compose_stack_view(
                base,
                id,
                None,
                compact_controls,
                settings_modal,
                Some(selected_tickers_static),
                tickers_table,
            );
            (body, extra)
        } else {
            let base = uninitialized_base(ContentKind::ComparisonChart);
            let body = self.compose_stack_view(
                base,
                id,
                None,
                compact_controls,
                || column![].into(),
                None,
                tickers_table,
            );
            (body, extra)
        }
    }
}
