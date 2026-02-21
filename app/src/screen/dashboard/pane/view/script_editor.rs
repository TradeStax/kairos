use crate::{
    screen::dashboard::pane::view::CompactControls,
    style::{self, tokens},
};
use exchange::{FuturesTicker, FuturesTickerInfo};
use iced::{
    Alignment, Element, Length,
    widget::{button, column, container, pick_list, row, text},
};
use rustc_hash::FxHashMap;

use super::super::{Content, Event, Message, State};

impl State {
    /// Build the Script Editor content view.
    ///
    /// Returns `(body, extra_title_elements)` where `extra_title_elements` are
    /// widgets to push onto the title-bar row.
    pub(crate) fn view_script_editor_body<'a>(
        &'a self,
        id: iced::widget::pane_grid::Pane,
        compact_controls: CompactControls<'a>,
        tickers_info: &'a FxHashMap<FuturesTicker, FuturesTickerInfo>,
        ticker_ranges: &'a std::collections::HashMap<String, String>,
    ) -> (Element<'a, Message>, Vec<Element<'a, Message>>) {
        let mut extras = Vec::new();

        if let Content::ScriptEditor {
            editor,
            script_path,
            script_list,
        } = &self.content
        {
            // Script selector in title bar
            let script_names: Vec<String> =
                script_list.iter().map(|e| e.name.clone()).collect();
            let selected_name = script_path
                .as_ref()
                .and_then(|p| p.file_stem())
                .and_then(|s| s.to_str())
                .map(String::from);

            let picker = pick_list(script_names, selected_name, move |name| {
                Message::PaneEvent(id, Event::ScriptSelected(name))
            })
            .padding([2, 8]);

            let new_btn = button(text("+").size(12))
                .on_press(Message::PaneEvent(id, Event::NewScript))
                .style(|theme, status| {
                    style::button::transparent(theme, status, false)
                })
                .padding([2, 6]);

            let mut title_row = row![picker, new_btn]
                .spacing(tokens::spacing::XS)
                .align_y(Alignment::Center);

            if editor.is_modified() {
                title_row = title_row.push(
                    text("*")
                        .size(14)
                        .color(iced::Color::from_rgb(1.0, 0.6, 0.2)),
                );
            }

            let save_btn = button(text("Save").size(12))
                .on_press(Message::PaneEvent(id, Event::SaveScript))
                .style(|theme, status| {
                    style::button::transparent(theme, status, false)
                })
                .padding([2, 8]);
            title_row = title_row.push(save_btn);

            extras.push(title_row.into());

            // Editor body
            let editor_view = editor
                .view()
                .map(move |msg| Message::PaneEvent(id, Event::EditorInteraction(msg)));

            let base: Element<'a, Message> = container(editor_view)
                .width(Length::Fill)
                .height(Length::Fill)
                .into();

            let body = self.compose_stack_view(
                base,
                id,
                compact_controls,
                || column![].into(),
                None,
                tickers_info,
                ticker_ranges,
            );

            (body, extras)
        } else {
            // Fallback — should not happen
            let body = self.compose_stack_view(
                text("Error: not a script editor").into(),
                id,
                compact_controls,
                || column![].into(),
                None,
                tickers_info,
                ticker_ranges,
            );
            (body, extras)
        }
    }
}
