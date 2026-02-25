use super::{Dashboard, Message, pane};
use crate::{
    style::{self, tokens},
    infra::window::{self, Window},
};
use data::UserTimezone;
use exchange::{FuturesTicker, FuturesTickerInfo};
use iced::{
    Element, Length, Task,
    widget::{PaneGrid, center, container},
};
use rustc_hash::FxHashMap;

impl Dashboard {
    pub fn load_layout(&mut self, _main_window: window::Id) -> Task<Message> {
        let mut open_popouts_tasks: Vec<Task<Message>> = vec![];
        let mut new_popout = Vec::new();
        let mut keys_to_remove = Vec::new();

        for (old_window_id, (_, specs)) in &self.popout {
            keys_to_remove.push((*old_window_id, specs.clone()));
        }

        for (old_window_id, window_spec) in keys_to_remove {
            let (pos_x, pos_y) = window_spec.clone().position_coords();
            let (width, height) = window_spec.clone().size_coords();

            let (window, task) = window::open(window::Settings {
                position: window::Position::Specific(iced::Point::new(pos_x, pos_y)),
                size: iced::Size::new(width, height),
                exit_on_close_request: false,
                ..window::settings()
            });

            open_popouts_tasks.push(task.then(|_| Task::none()));

            if let Some((removed_pane, specs)) = self.popout.remove(&old_window_id) {
                new_popout.push((window, (removed_pane, specs)));
            }
        }

        for (window, (pane, specs)) in new_popout {
            self.popout.insert(window, (pane, specs));
        }

        Task::batch(open_popouts_tasks)
    }

    pub fn view<'a>(
        &'a self,
        main_window: &'a Window,
        tickers_info: &'a FxHashMap<FuturesTicker, FuturesTickerInfo>,
        timezone: UserTimezone,
        ticker_ranges: &'a std::collections::HashMap<String, String>,
    ) -> Element<'a, Message> {
        let pane_grid: Element<_> = PaneGrid::new(&self.panes, |id, pane, maximized| {
            let is_focused = self.focus == Some((main_window.id, id));
            pane.view(
                id,
                self.panes.len(),
                is_focused,
                maximized,
                main_window.id,
                main_window,
                timezone,
                tickers_info,
                ticker_ranges,
            )
        })
        .min_size(240)
        .on_click(pane::Message::PaneClicked)
        .on_drag(pane::Message::PaneDragged)
        .on_resize(8, pane::Message::PaneResized)
        .spacing(tokens::spacing::SM)
        .style(style::pane_grid)
        .into();

        pane_grid.map(move |message| Message::Pane(main_window.id, message))
    }

    pub fn view_window<'a>(
        &'a self,
        window: window::Id,
        main_window: &'a Window,
        tickers_info: &'a FxHashMap<FuturesTicker, FuturesTickerInfo>,
        timezone: UserTimezone,
        ticker_ranges: &'a std::collections::HashMap<String, String>,
    ) -> Element<'a, Message> {
        // Pre-compute available chart panes from all panes (main + popout)
        if let Some((state, _)) = self.popout.get(&window) {
            let content = container(
                PaneGrid::new(state, |id, pane, _maximized| {
                    let is_focused = self.focus == Some((window, id));
                    pane.view(
                        id,
                        state.len(),
                        is_focused,
                        false,
                        window,
                        main_window,
                        timezone,
                        tickers_info,
                        ticker_ranges,
                    )
                })
                .on_click(pane::Message::PaneClicked),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(tokens::spacing::MD);

            Element::new(content).map(move |message| Message::Pane(window, message))
        } else {
            Element::new(center("No pane found for window"))
                .map(move |message| Message::Pane(window, message))
        }
    }

    pub fn go_back(&mut self, main_window: window::Id) -> bool {
        let Some((window, pane)) = self.focus else {
            return false;
        };

        let Some(state) = self.get_mut_pane(main_window, window, pane) else {
            return false;
        };

        if state.modal.is_some() {
            state.modal = None;
            return true;
        }
        false
    }
}
