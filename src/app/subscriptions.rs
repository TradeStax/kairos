use iced::{keyboard, Subscription};
use futures::stream::StreamExt;

use super::{ChartMessage, DownloadMessage, Message};
use crate::window;


/// Rithmic streaming event monitor
/// Drains ALL events from the global buffer every 50ms
fn rithmic_event_monitor() -> impl futures::stream::Stream<Item = Message> {
    futures::stream::unfold((), |_| async {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let events: Vec<exchange::Event> = {
            if let Ok(mut buf) = super::get_rithmic_events().lock() {
                if buf.is_empty() {
                    return Some((Vec::new(), ()));
                }
                buf.drain(..).collect()
            } else {
                Vec::new()
            }
        };

        Some((events, ()))
    })
    .flat_map(|events| {
        futures::stream::iter(
            events
                .into_iter()
                .map(Message::RithmicStreamEvent),
        )
    })
}

/// Replay engine event monitor
/// Drains ALL events from the global buffer every 50ms
fn replay_event_monitor() -> impl futures::stream::Stream<Item = Message> {
    futures::stream::unfold((), |_| async {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let events: Vec<data::services::ReplayEvent> = {
            if let Ok(mut buf) = super::get_replay_events().lock() {
                if buf.is_empty() {
                    return Some((Vec::new(), ()));
                }
                buf.drain(..).collect()
            } else {
                Vec::new()
            }
        };

        Some((events, ()))
    })
    .flat_map(|events| {
        futures::stream::iter(
            events
                .into_iter()
                .map(Message::ReplayEvent),
        )
    })
}

/// Download progress monitoring subscription
/// Uses global download progress state to avoid Subscription capture issues
pub fn download_progress_monitor() -> impl futures::stream::Stream<Item = Message> {
    futures::stream::unfold((), |_| async {
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        let messages: Vec<Message> = {
            if let Ok(progress) = super::get_download_progress().lock() {
                progress
                    .iter()
                    .map(|(&pane_id, &(current, total))| {
                        Message::Download(DownloadMessage::DataDownloadProgress {
                            pane_id,
                            current,
                            total,
                        })
                    })
                    .collect()
            } else {
                Vec::new()
            }
        };

        Some((messages, ()))
    })
    .flat_map(futures::stream::iter)
}

/// Replay drag tracking subscription
/// Only active during drag operations on the floating replay panel
fn replay_drag_subscription() -> Subscription<Message> {
    iced::event::listen_with(|event, _status, _id| match event {
        iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => Some(
            Message::Replay(crate::modal::replay_manager::Message::DragMove(position)),
        ),
        iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
            iced::mouse::Button::Left,
        )) => Some(Message::Replay(
            crate::modal::replay_manager::Message::DragEnd,
        )),
        _ => None,
    })
}

/// Build the main application subscription
pub fn build_subscription(
    replay_is_dragging: bool,
) -> Subscription<Message> {
    let window_events = window::events().map(Message::WindowEvent);

    let tick = iced::time::every(std::time::Duration::from_millis(100)).map(Message::Tick);

    // Poll for loading status updates every 500ms.
    // The handler in update.rs short-circuits when no service is available.
    let status_poll = iced::time::every(std::time::Duration::from_millis(500))
        .map(|_| Message::Chart(ChartMessage::UpdateLoadingStatus));

    // Download progress monitoring subscription
    let download_poll = Subscription::run(download_progress_monitor);

    // Rithmic streaming event subscription
    let rithmic_poll = Subscription::run(rithmic_event_monitor);

    // Replay engine event subscription
    let replay_poll = Subscription::run(replay_event_monitor);

    let hotkeys = keyboard::listen().filter_map(|event| {
        let keyboard::Event::KeyPressed { key, .. } = event else {
            return None;
        };
        match key {
            keyboard::Key::Named(keyboard::key::Named::Escape) => Some(Message::GoBack),
            _ => None,
        }
    });

    let mut subs = vec![
        window_events,
        tick,
        status_poll,
        download_poll,
        rithmic_poll,
        replay_poll,
        hotkeys,
    ];

    if replay_is_dragging {
        subs.push(replay_drag_subscription());
    }

    Subscription::batch(subs)
}
