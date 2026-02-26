pub(crate) mod ai_context_bubble;
mod assistant;
mod body;
mod charts;
mod controls;
mod header;
pub(crate) mod helpers;
mod modal_stack;

pub(crate) use modal_stack::CompactControls;

use crate::config::UserTimezone;
use crate::{
    modals::{self, pane::Modal},
    style::{self, tokens},
    window::{self, Window},
};
use data::{FuturesTicker, FuturesTickerInfo};
use iced::{
    Alignment, Length, Renderer, Theme,
    alignment::Vertical,
    padding,
    widget::{button, container, pane_grid},
};
use rustc_hash::FxHashMap;

use super::{Event, Message, State};

impl State {
    pub fn view<'a>(
        &'a self,
        id: pane_grid::Pane,
        panes: usize,
        is_focused: bool,
        maximized: bool,
        window: window::Id,
        main_window: &'a Window,
        timezone: UserTimezone,
        tickers_info: &'a FxHashMap<FuturesTicker, FuturesTickerInfo>,
        ticker_ranges: &'a std::collections::HashMap<String, String>,
    ) -> pane_grid::Content<'a, Message, Theme, Renderer> {
        // Build the base stream-info row (link group + ticker + AI labels)
        let (mut stream_info_element, is_ai_pane) =
            header::build_stream_info_row(self, id, tickers_info, ticker_ranges, timezone);

        let modifier: Option<modals::stream::Modifier> = self.modal.clone().and_then(|m| {
            if let Modal::StreamModifier(modifier) = m {
                Some(modifier)
            } else {
                None
            }
        });

        let compact_controls: CompactControls<'a> = if self.modal == Some(Modal::Controls) {
            Some(
                container(self.view_controls(id, panes, maximized, window != main_window.id))
                    .style(style::chart_modal)
                    .into(),
            )
        } else {
            None
        };

        // Dispatch content body and collect any extras for the stream-info row
        let (body, extras) = body::dispatch_body(
            self,
            id,
            modifier,
            compact_controls,
            timezone,
            tickers_info,
            ticker_ranges,
        );

        for e in extras {
            stream_info_element = stream_info_element.push(e);
        }

        // Append loading status badge to stream-info row
        stream_info_element = header::append_loading_badge(stream_info_element, self, is_ai_pane);

        let content = pane_grid::Content::new(body)
            .style(move |theme| style::pane_background(theme, is_focused));

        let controls = {
            let compact_control = container(
                button(
                    crate::components::primitives::label::label_text("...").align_y(Alignment::End),
                )
                .on_press(Message::PaneEvent(
                    id,
                    Box::new(Event::ShowModal(Modal::Controls)),
                ))
                .style(move |theme, status| {
                    style::button::transparent(
                        theme,
                        status,
                        self.modal == Some(Modal::Controls) || self.modal == Some(Modal::Settings),
                    )
                }),
            )
            .align_y(Alignment::Center)
            .height(Length::Fixed(tokens::layout::TITLE_BAR_HEIGHT))
            .padding(tokens::spacing::XS);

            if self.modal == Some(Modal::Controls) {
                pane_grid::Controls::new(compact_control)
            } else {
                pane_grid::Controls::dynamic(
                    self.view_controls(id, panes, maximized, window != main_window.id),
                    compact_control,
                )
            }
        };

        let title_bar = pane_grid::TitleBar::new(
            stream_info_element
                .padding(padding::left(tokens::spacing::XS))
                .align_y(Vertical::Center)
                .spacing(tokens::spacing::XS)
                .height(Length::Fixed(tokens::layout::TITLE_BAR_HEIGHT)),
        )
        .controls(controls)
        .style(style::pane_title_bar);

        content.title_bar(if self.modal.is_none() {
            title_bar
        } else {
            title_bar.always_show_controls()
        })
    }
}
