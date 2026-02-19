use super::super::{Event, Message, State};
use super::helpers::link_group_modal;
use crate::{
    component::display::toast,
    modal::{self, pane::Modal},
    style::{self, tokens},
};
use exchange::{FuturesTicker, FuturesTickerInfo};
use iced::{
    Alignment, Element, padding,
    widget::{column, container, pane_grid},
};
use rustc_hash::FxHashMap;

/// Alias for the optional compact-controls overlay element.
pub(crate) type CompactControls<'a> = Option<Element<'a, Message>>;

impl State {
    pub(crate) fn compose_stack_view<'a, F>(
        &'a self,
        base: Element<'a, Message>,
        pane: pane_grid::Pane,
        indicator_modal: Option<Element<'a, Message>>,
        compact_controls: Option<Element<'a, Message>>,
        settings_modal: F,
        selected_tickers: Option<&'a [FuturesTickerInfo]>,
        tickers_info: &'a FxHashMap<FuturesTicker, FuturesTickerInfo>,
    ) -> Element<'a, Message>
    where
        F: FnOnce() -> Element<'a, Message>,
    {
        use modal::pane::stack_modal;

        let base = toast::Manager::new(base, &self.notifications, Alignment::End, move |msg| {
            Message::PaneEvent(pane, Event::DeleteNotification(msg))
        })
        .into();

        let on_blur = Message::PaneEvent(pane, Event::HideModal);

        match &self.modal {
            Some(Modal::LinkGroup) => {
                let content = link_group_modal(pane, self.link_group);

                stack_modal(
                    base,
                    content,
                    on_blur,
                    padding::right(tokens::spacing::LG).left(tokens::spacing::XS),
                    Alignment::Start,
                )
            }
            Some(Modal::StreamModifier(modifier)) => stack_modal(
                base,
                modifier.view(self.ticker_info).map(move |message| {
                    Message::PaneEvent(pane, Event::StreamModifierChanged(message))
                }),
                Message::PaneEvent(pane, Event::HideModal),
                padding::right(tokens::spacing::LG).left(48),
                Alignment::Start,
            ),
            Some(Modal::MiniTickersList(panel)) => {
                let mini_list = panel
                    .view(tickers_info, selected_tickers, self.ticker_info)
                    .map(move |msg| {
                        Message::PaneEvent(pane, Event::MiniTickersListInteraction(msg))
                    });

                let content: Element<_> = container(mini_list)
                    .max_width(260)
                    .max_height(480)
                    .clip(true)
                    .padding(padding::top(tokens::spacing::MD)
                        .left(tokens::spacing::XL)
                        .right(tokens::spacing::XL)
                        .bottom(tokens::spacing::MD))
                    .style(style::chart_modal)
                    .into();

                stack_modal(
                    base,
                    content,
                    Message::PaneEvent(pane, Event::HideModal),
                    padding::left(tokens::spacing::LG),
                    Alignment::Start,
                )
            }
            Some(Modal::Settings) => stack_modal(
                base,
                settings_modal(),
                on_blur,
                padding::right(tokens::spacing::LG).left(tokens::spacing::LG),
                Alignment::End,
            ),
            Some(Modal::Indicators) => stack_modal(
                base,
                indicator_modal.unwrap_or_else(|| column![].into()),
                on_blur,
                padding::right(tokens::spacing::LG).left(tokens::spacing::LG),
                Alignment::End,
            ),
            Some(Modal::Controls) => stack_modal(
                base,
                if let Some(controls) = compact_controls {
                    controls
                } else {
                    column![].into()
                },
                on_blur,
                padding::left(tokens::spacing::LG),
                Alignment::End,
            ),
            Some(Modal::DataManagement(panel)) => {
                let pane_id = pane;
                stack_modal(
                    base,
                    panel.view().map(move |msg| {
                        Message::PaneEvent(pane_id, Event::DataManagementInteraction(msg))
                    }),
                    on_blur,
                    padding::all(tokens::spacing::LG),
                    Alignment::Center,
                )
            }
            None => base,
        }
    }
}
