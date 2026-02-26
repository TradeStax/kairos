use crate::screen::dashboard::pane::config::ContentKind;
use crate::{
    components, components::primitives::label::*, screen::dashboard::pane::view::CompactControls,
    style::tokens,
};
use data::{FuturesTicker, FuturesTickerInfo};
use iced::{
    Alignment, Element,
    widget::{center, column, pick_list},
};
use rustc_hash::FxHashMap;

use super::super::super::{Event, Message, State};

impl State {
    /// Build the Starter content view (content-type picker placeholder).
    pub(crate) fn view_starter_body<'a>(
        &'a self,
        id: iced::widget::pane_grid::Pane,
        compact_controls: CompactControls<'a>,
        tickers_info: &'a FxHashMap<FuturesTicker, FuturesTickerInfo>,
        ticker_ranges: &'a std::collections::HashMap<String, String>,
    ) -> Element<'a, Message> {
        let content_picklist =
            pick_list(ContentKind::ALL, Some(ContentKind::Starter), move |kind| {
                Message::PaneEvent(id, Box::new(Event::ContentSelected(kind)))
            });

        let base: Element<_> = components::display::toast::Manager::new(
            center(
                column![heading("Choose a view to get started"), content_picklist]
                    .align_x(Alignment::Center)
                    .spacing(tokens::spacing::LG),
            ),
            &self.notifications,
            Alignment::End,
            move |msg| Message::PaneEvent(id, Box::new(Event::DeleteNotification(msg))),
        )
        .into();

        self.compose_stack_view(
            base,
            id,
            compact_controls,
            || column![].into(),
            None,
            tickers_info,
            ticker_ranges,
        )
    }
}
