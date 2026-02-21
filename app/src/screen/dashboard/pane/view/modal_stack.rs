use super::super::{ContextMenuAction, ContextMenuKind, Event, Message, State};
use super::helpers::link_group_modal;
use crate::{
    components::{display::toast, overlay::context_menu::context_menu},
    modals::{self, pane::Modal},
    style::{self, tokens},
};
use exchange::{FuturesTicker, FuturesTickerInfo};
use iced::{
    Alignment, Element, padding,
    widget::{column, container, pane_grid, stack},
};
use rustc_hash::FxHashMap;

/// Alias for the optional compact-controls overlay element.
pub(crate) type CompactControls<'a> = Option<Element<'a, Message>>;

impl State {
    pub(crate) fn compose_stack_view<'a, F>(
        &'a self,
        base: Element<'a, Message>,
        pane: pane_grid::Pane,
        compact_controls: Option<Element<'a, Message>>,
        settings_modal: F,
        selected_tickers: Option<&'a [FuturesTickerInfo]>,
        tickers_info: &'a FxHashMap<FuturesTicker, FuturesTickerInfo>,
        ticker_ranges: &'a std::collections::HashMap<String, String>,
    ) -> Element<'a, Message>
    where
        F: FnOnce() -> Element<'a, Message>,
    {
        use modals::pane::stack_modal;

        let base = toast::Manager::new(base, &self.notifications, Alignment::End, move |msg| {
            Message::PaneEvent(pane, Event::DeleteNotification(msg))
        })
        .into();

        let on_blur = Message::PaneEvent(pane, Event::HideModal);

        let base = match &self.modal {
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
                    .view(
                        tickers_info,
                        selected_tickers,
                        self.ticker_info,
                        ticker_ranges,
                    )
                    .map(move |msg| {
                        Message::PaneEvent(pane, Event::MiniTickersListInteraction(msg))
                    });

                let content: Element<_> = container(mini_list)
                    .max_width(260)
                    .max_height(480)
                    .clip(true)
                    .padding(
                        padding::top(tokens::spacing::MD)
                            .left(tokens::spacing::XL)
                            .right(tokens::spacing::XL)
                            .bottom(tokens::spacing::MD),
                    )
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
            Some(Modal::IndicatorManager(manager)) => {
                let content = manager.view().map(move |msg| {
                    Message::PaneEvent(
                        pane,
                        Event::IndicatorManagerInteraction(msg),
                    )
                });
                crate::modals::main_dialog_modal(
                    base,
                    content,
                    Message::PaneEvent(
                        pane,
                        Event::IndicatorManagerInteraction(
                            crate::modals::pane::indicator_manager::Message::Close,
                        ),
                    ),
                )
            }
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
            Some(Modal::DrawingProperties(props)) => {
                let content = props
                    .view()
                    .map(move |msg| Message::PaneEvent(pane, Event::DrawingPropertiesChanged(msg)));
                crate::modals::main_dialog_modal(
                    base,
                    content,
                    Message::PaneEvent(
                        pane,
                        Event::DrawingPropertiesChanged(
                            crate::modals::drawing_properties::Message::Close,
                        ),
                    ),
                )
            }
            Some(Modal::BigTradesDebug) => {
                let (output, tick_size) = self
                    .content
                    .big_trades_debug_info()
                    .unwrap_or((&study::StudyOutput::Empty, 0.25));
                let content = modals::pane::settings::big_trades_debug_view(
                    output, tick_size,
                );
                stack_modal(
                    base,
                    content,
                    on_blur,
                    padding::all(tokens::spacing::LG),
                    Alignment::Center,
                )
            }
            None => base,
        };

        // Context menu overlay on top of everything
        if let Some(ref ctx_menu) = self.context_menu {
            let items = match ctx_menu {
                ContextMenuKind::Chart { .. } => {
                    let mut items = vec![];

                    if self.content.has_indicators() {
                        items.push((
                            "Indicators".into(),
                            Some(Message::PaneEvent(
                                pane,
                                Event::ContextMenuAction(ContextMenuAction::OpenIndicators),
                            )),
                        ));
                    }

                    items.push((
                        "Center Last Price".into(),
                        Some(Message::PaneEvent(
                            pane,
                            Event::ContextMenuAction(ContextMenuAction::CenterLastPrice),
                        )),
                    ));

                    items.push((
                        "Rebuild Chart".into(),
                        Some(Message::PaneEvent(
                            pane,
                            Event::ContextMenuAction(ContextMenuAction::RebuildChart),
                        )),
                    ));

                    items
                }
                ContextMenuKind::Drawing { id, locked, .. } => {
                    let id = *id;
                    vec![
                        (
                            "Properties".into(),
                            Some(Message::PaneEvent(
                                pane,
                                Event::ContextMenuAction(ContextMenuAction::OpenDrawingProperties(
                                    id,
                                )),
                            )),
                        ),
                        (
                            if *locked { "Unlock" } else { "Lock" }.into(),
                            Some(Message::PaneEvent(
                                pane,
                                Event::ContextMenuAction(ContextMenuAction::ToggleLockDrawing(id)),
                            )),
                        ),
                        (
                            "Clone".into(),
                            Some(Message::PaneEvent(
                                pane,
                                Event::ContextMenuAction(ContextMenuAction::CloneDrawing(id)),
                            )),
                        ),
                        (
                            "Delete".into(),
                            Some(Message::PaneEvent(
                                pane,
                                Event::ContextMenuAction(ContextMenuAction::DeleteDrawing(id)),
                            )),
                        ),
                    ]
                }
            };
            let position = ctx_menu.position();
            let overlay = context_menu(
                items,
                position,
                Message::PaneEvent(pane, Event::DismissContextMenu),
            );
            stack![base, overlay].into()
        } else {
            base
        }
    }
}
