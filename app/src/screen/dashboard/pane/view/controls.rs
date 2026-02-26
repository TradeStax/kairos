use super::super::{Content, Event, Message, State};
use crate::{
    components::display::tooltip::button_with_tooltip,
    components::primitives::{Icon, icon_text},
    modals::pane::Modal,
    screen::dashboard::pane::types::AiAssistantEvent,
    style::{self, tokens},
};
use iced::{
    Element, Length, Theme,
    alignment::Vertical,
    padding,
    widget::{button, pane_grid, row, tooltip},
};

impl State {
    pub(crate) fn view_controls(
        &'_ self,
        pane: pane_grid::Pane,
        total_panes: usize,
        is_maximized: bool,
        is_popout: bool,
    ) -> Element<'_, Message> {
        let modal_btn_style = |modal: Modal| {
            let is_active = self.modal == Some(modal);
            move |theme: &Theme, status: button::Status| {
                style::button::transparent(theme, status, is_active)
            }
        };

        let control_btn_style = |is_active: bool| {
            move |theme: &Theme, status: button::Status| {
                style::button::transparent(theme, status, is_active)
            }
        };

        let treat_as_starter =
            matches!(&self.content, Content::Starter) || !self.content.initialized();

        let tooltip_pos = tooltip::Position::Bottom;
        let mut buttons = row![];

        let show_modal = |modal: Modal| Message::PaneEvent(pane, Box::new(Event::ShowModal(modal)));

        if !treat_as_starter {
            // AI pane: gear toggles the in-panel settings overlay; trash clears history
            if let Content::AiAssistant(ai_state) = &self.content {
                let is_settings_open = ai_state.show_settings;
                buttons = buttons.push(button_with_tooltip(
                    icon_text(Icon::Cog, 12),
                    Message::PaneEvent(
                        pane,
                        Box::new(Event::AiAssistant(AiAssistantEvent::ToggleSettings)),
                    ),
                    Some("Settings"),
                    tooltip_pos,
                    move |theme: &Theme, status: button::Status| {
                        style::button::transparent(theme, status, is_settings_open)
                    },
                ));
                buttons = buttons.push(button_with_tooltip(
                    icon_text(Icon::TrashBin, 12),
                    Message::PaneEvent(
                        pane,
                        Box::new(Event::AiAssistant(AiAssistantEvent::ClearHistory)),
                    ),
                    Some("Clear History"),
                    tooltip_pos,
                    control_btn_style(false),
                ));
            } else {
                buttons = buttons.push(button_with_tooltip(
                    icon_text(Icon::Cog, 12),
                    show_modal(Modal::Settings),
                    None,
                    tooltip_pos,
                    modal_btn_style(Modal::Settings),
                ));
            }
        }
        let show_indicators = matches!(&self.content, Content::Candlestick { .. });
        #[cfg(feature = "heatmap")]
        let show_indicators = show_indicators || matches!(&self.content, Content::Heatmap { .. });
        if !treat_as_starter && show_indicators {
            buttons = buttons.push(button_with_tooltip(
                icon_text(Icon::ChartOutline, 12),
                Message::PaneEvent(pane, Box::new(Event::OpenIndicatorManager)),
                Some("Indicators"),
                tooltip_pos,
                {
                    let is_active = matches!(self.modal, Some(Modal::IndicatorManager(_)));
                    move |theme: &Theme, status: button::Status| {
                        style::button::transparent(theme, status, is_active)
                    }
                },
            ));
        }

        if is_popout {
            buttons = buttons.push(button_with_tooltip(
                icon_text(Icon::Popout, 12),
                Message::Merge,
                Some("Merge"),
                tooltip_pos,
                control_btn_style(is_popout),
            ));
        } else if total_panes > 1 {
            buttons = buttons.push(button_with_tooltip(
                icon_text(Icon::Popout, 12),
                Message::Popout,
                Some("Pop out"),
                tooltip_pos,
                control_btn_style(is_popout),
            ));
        }

        if total_panes > 1 {
            let (resize_icon, message) = if is_maximized {
                (Icon::ResizeSmall, Message::Restore)
            } else {
                (Icon::ResizeFull, Message::MaximizePane(pane))
            };

            buttons = buttons.push(button_with_tooltip(
                icon_text(resize_icon, 12),
                message,
                None,
                tooltip_pos,
                control_btn_style(is_maximized),
            ));

            buttons = buttons.push(button_with_tooltip(
                icon_text(Icon::Close, 12),
                Message::ClosePane(pane),
                None,
                tooltip_pos,
                control_btn_style(false),
            ));
        }

        buttons
            .padding(padding::right(tokens::spacing::XS).left(tokens::spacing::XS))
            .align_y(Vertical::Center)
            .height(Length::Fixed(tokens::layout::TITLE_BAR_HEIGHT))
            .into()
    }
}
