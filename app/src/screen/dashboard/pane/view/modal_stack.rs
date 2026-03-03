use super::super::{ContextMenuAction, ContextMenuKind, Event, Message, State};
use super::ai_context_bubble;
use super::helpers::link_group_modal;
use crate::{
    components::{display::toast, overlay::context_menu::context_menu},
    modals::{self, pane::Modal},
    screen::dashboard::pane::types::AiContextBubbleEvent,
    style::{self, tokens},
};
use data::{FuturesTicker, FuturesTickerInfo};
use iced::{
    Alignment, Element, Length, padding,
    widget::{column, container, mouse_area, opaque, pane_grid, stack},
};
use rustc_hash::FxHashMap;

/// Alias for the optional compact-controls overlay element.
pub(crate) type CompactControls<'a> = Option<Element<'a, Message>>;

/// Build the context menu items for a given context menu kind and pane.
fn context_menu_items(
    ctx: &ContextMenuKind,
    pane: pane_grid::Pane,
    has_indicators: bool,
) -> Vec<(String, Option<Message>)> {
    match ctx {
        ContextMenuKind::Chart { .. } => {
            let mut items = vec![];

            if has_indicators {
                items.push((
                    "Indicators".into(),
                    Some(Message::PaneEvent(
                        pane,
                        Box::new(Event::ContextMenuAction(ContextMenuAction::OpenIndicators)),
                    )),
                ));
            }

            items.push((
                "Center Last Price".into(),
                Some(Message::PaneEvent(
                    pane,
                    Box::new(Event::ContextMenuAction(ContextMenuAction::CenterLastPrice)),
                )),
            ));

            items.push((
                "Rebuild Chart".into(),
                Some(Message::PaneEvent(
                    pane,
                    Box::new(Event::ContextMenuAction(ContextMenuAction::RebuildChart)),
                )),
            ));

            items
        }
        ContextMenuKind::StudyOverlay { study_index, .. } => {
            let idx = *study_index;
            vec![(
                "Properties".into(),
                Some(Message::PaneEvent(
                    pane,
                    Box::new(Event::ContextMenuAction(
                        ContextMenuAction::OpenStudyProperties(idx),
                    )),
                )),
            )]
        }
        ContextMenuKind::Drawing { id, locked, .. } => {
            let id = *id;
            vec![
                (
                    "Properties".into(),
                    Some(Message::PaneEvent(
                        pane,
                        Box::new(Event::ContextMenuAction(
                            ContextMenuAction::OpenDrawingProperties(id),
                        )),
                    )),
                ),
                (
                    if *locked { "Unlock" } else { "Lock" }.into(),
                    Some(Message::PaneEvent(
                        pane,
                        Box::new(Event::ContextMenuAction(
                            ContextMenuAction::ToggleLockDrawing(id),
                        )),
                    )),
                ),
                (
                    "Clone".into(),
                    Some(Message::PaneEvent(
                        pane,
                        Box::new(Event::ContextMenuAction(ContextMenuAction::CloneDrawing(
                            id,
                        ))),
                    )),
                ),
                (
                    "Delete".into(),
                    Some(Message::PaneEvent(
                        pane,
                        Box::new(Event::ContextMenuAction(ContextMenuAction::DeleteDrawing(
                            id,
                        ))),
                    )),
                ),
            ]
        }
        ContextMenuKind::AiMessage { message_index, .. } => {
            let idx = *message_index;
            vec![(
                "Copy".into(),
                Some(Message::PaneEvent(
                    pane,
                    Box::new(Event::ContextMenuAction(
                        ContextMenuAction::CopyAiMessageText(idx),
                    )),
                )),
            )]
        }
    }
}

impl State {
    pub(crate) fn compose_stack_view<'a, F>(
        &'a self,
        base: Element<'a, Message>,
        pane: pane_grid::Pane,
        compact_controls: Option<Element<'a, Message>>,
        settings_modal: F,
        selected_tickers: Option<&[FuturesTickerInfo]>,
        tickers_info: &'a FxHashMap<FuturesTicker, FuturesTickerInfo>,
        ticker_ranges: &'a std::collections::HashMap<String, String>,
    ) -> Element<'a, Message>
    where
        F: FnOnce() -> Element<'a, Message>,
    {
        use modals::pane::stack_modal;

        let base = toast::Manager::new(base, &self.notifications, Alignment::End, move |msg| {
            Message::PaneEvent(pane, Box::new(Event::DeleteNotification(msg)))
        })
        .into();

        let on_blur = Message::PaneEvent(pane, Box::new(Event::HideModal));

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
                    Message::PaneEvent(pane, Box::new(Event::StreamModifierChanged(message)))
                }),
                Message::PaneEvent(pane, Box::new(Event::HideModal)),
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
                        Message::PaneEvent(pane, Box::new(Event::MiniTickersListInteraction(msg)))
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
                    Message::PaneEvent(pane, Box::new(Event::HideModal)),
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
                    Message::PaneEvent(pane, Box::new(Event::IndicatorManagerInteraction(msg)))
                });
                crate::modals::main_dialog_modal(
                    base,
                    content,
                    Message::PaneEvent(
                        pane,
                        Box::new(Event::IndicatorManagerInteraction(
                            crate::modals::pane::indicator::Message::Close,
                        )),
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
                        Message::PaneEvent(pane_id, Box::new(Event::DataManagementInteraction(msg)))
                    }),
                    on_blur,
                    padding::all(tokens::spacing::LG),
                    Alignment::Center,
                )
            }
            Some(Modal::DrawingProperties(props)) => {
                let content = props.view().map(move |msg| {
                    Message::PaneEvent(pane, Box::new(Event::DrawingPropertiesChanged(msg)))
                });
                crate::modals::main_dialog_modal(
                    base,
                    content,
                    Message::PaneEvent(
                        pane,
                        Box::new(Event::DrawingPropertiesChanged(
                            crate::modals::drawing::properties::Message::Close,
                        )),
                    ),
                )
            }
            Some(Modal::LevelDetail(modal)) => {
                let content = modal.view().map(move |msg| {
                    Message::PaneEvent(pane, Box::new(Event::LevelDetailInteraction(msg)))
                });
                crate::modals::main_dialog_modal(
                    base,
                    content,
                    Message::PaneEvent(
                        pane,
                        Box::new(Event::LevelDetailInteraction(
                            crate::modals::pane::level_detail::Message::Close,
                        )),
                    ),
                )
            }
            None => base,
        };

        // AI context bubble overlay (between modals and context menu)
        let base = if let Some(ref bubble) = self.ai_context_bubble {
            let panel = ai_context_bubble::view(bubble, pane);
            let bubble_width = 340.0_f32;
            let left = (bubble.anchor.x - bubble_width / 2.0).max(0.0);
            let top = bubble.anchor.y + 8.0;
            let overlay = container(opaque(panel))
                .width(Length::Fill)
                .height(Length::Fill)
                .padding(iced::Padding {
                    top,
                    right: 0.0,
                    bottom: 0.0,
                    left,
                });
            stack![
                base,
                mouse_area(overlay).on_press(Message::PaneEvent(
                    pane,
                    Box::new(Event::AiContextBubble(AiContextBubbleEvent::Dismiss),)
                ))
            ]
            .into()
        } else {
            base
        };

        // Context menu overlay on top of everything
        if let Some(ref ctx_menu) = self.context_menu {
            let items = context_menu_items(ctx_menu, pane, self.content.has_indicators());
            let position = ctx_menu.position();
            let overlay = context_menu(
                items,
                position,
                Message::PaneEvent(pane, Box::new(Event::DismissContextMenu)),
            );
            stack![base, overlay].into()
        } else {
            base
        }
    }
}
