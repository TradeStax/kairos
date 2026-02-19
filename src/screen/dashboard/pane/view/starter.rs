use crate::{
    component,
    component::primitives::label::*,
    screen::dashboard::pane::view::CompactControls,
    style::tokens,
};
use data::ContentKind;
use exchange::{FuturesTicker, FuturesTickerInfo};
use iced::{
    Alignment, Element,
    widget::{center, column, pick_list},
};
use rustc_hash::FxHashMap;

use super::super::{Event, Message, State};

impl State {
    /// Build the Starter content view (content-type picker placeholder).
    pub(crate) fn view_starter_body<'a>(
        &'a self,
        id: iced::widget::pane_grid::Pane,
        compact_controls: CompactControls<'a>,
        tickers_info: &'a FxHashMap<FuturesTicker, FuturesTickerInfo>,
    ) -> Element<'a, Message> {
        let content_picklist =
            pick_list(ContentKind::ALL, Some(ContentKind::Starter), move |kind| {
                Message::PaneEvent(id, Event::ContentSelected(kind))
            });

        let base: Element<_> = component::display::toast::Manager::new(
            center(
                column![heading("Choose a view to get started"), content_picklist]
                    .align_x(Alignment::Center)
                    .spacing(tokens::spacing::LG),
            ),
            &self.notifications,
            Alignment::End,
            move |msg| Message::PaneEvent(id, Event::DeleteNotification(msg)),
        )
        .into();

        self.compose_stack_view(
            base,
            id,
            None,
            compact_controls,
            || column![].into(),
            None,
            tickers_info,
        )
    }
}
