use futures::stream::StreamExt;
use iced::{Subscription, keyboard};

use super::{ChartMessage, DownloadMessage, Message};
use crate::window;

/// Rithmic streaming event monitor
/// Drains ALL events from the global buffer every 50ms.
/// Sleeps longer (500ms) when no Rithmic feed is active.
fn rithmic_event_monitor() -> impl futures::stream::Stream<Item = Message> {
    futures::stream::unfold((), |_| async {
        let interval = if super::globals::is_rithmic_active() {
            std::time::Duration::from_millis(50)
        } else {
            std::time::Duration::from_millis(500)
        };
        tokio::time::sleep(interval).await;

        let events: Vec<exchange::Event> = {
            if let Ok(mut buf) = super::globals::get_rithmic_events().lock() {
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
    .flat_map(|events| futures::stream::iter(events.into_iter().map(Message::RithmicStreamEvent)))
}

/// Replay engine event monitor
/// Drains ALL events from the global buffer every 50ms.
/// Sleeps longer (500ms) when no replay session is active.
fn replay_event_monitor() -> impl futures::stream::Stream<Item = Message> {
    futures::stream::unfold((), |_| async {
        let interval = if super::globals::is_replay_active() {
            std::time::Duration::from_millis(50)
        } else {
            std::time::Duration::from_millis(500)
        };
        tokio::time::sleep(interval).await;

        let events: Vec<data::services::ReplayEvent> = {
            if let Ok(mut buf) = super::globals::get_replay_events().lock() {
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
    .flat_map(|events| futures::stream::iter(events.into_iter().map(Message::ReplayEvent)))
}

/// Download progress monitoring subscription
/// Uses global download progress state to avoid Subscription capture issues.
/// Polls at 200ms when a download is active, 2000ms when idle.
pub fn download_progress_monitor() -> impl futures::stream::Stream<Item = Message> {
    futures::stream::unfold((), |_| async {
        let interval = if super::globals::is_download_active() {
            std::time::Duration::from_millis(200)
        } else {
            std::time::Duration::from_millis(2000)
        };
        tokio::time::sleep(interval).await;

        let messages: Vec<Message> = {
            if let Ok(progress) = super::globals::get_download_progress().lock() {
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
        iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => Some(Message::Replay(
            crate::modals::replay::Message::DragMove(position),
        )),
        iced::Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left)) => {
            Some(Message::Replay(crate::modals::replay::Message::DragEnd))
        }
        _ => None,
    })
}

/// Build the main application subscription
pub fn build_subscription(replay_is_dragging: bool) -> Subscription<Message> {
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
        let keyboard::Event::KeyPressed {
            key, modifiers, ..
        } = event
        else {
            return None;
        };
        match key {
            keyboard::Key::Named(keyboard::key::Named::Escape) => Some(Message::GoBack),
            keyboard::Key::Character(ref c) if c.as_str() == "s" && modifiers.command() => {
                Some(Message::SaveFocusedScript)
            }
            // Ctrl+Shift+Z → Redo drawing (must be before Ctrl+Z)
            keyboard::Key::Character(ref c)
                if c.as_str() == "z" && modifiers.command() && modifiers.shift() =>
            {
                Some(Message::Dashboard {
                    layout_id: None,
                    event: crate::screen::dashboard::Message::DrawingRedo,
                })
            }
            // Ctrl+Z → Undo drawing
            keyboard::Key::Character(ref c)
                if c.as_str() == "z" && modifiers.command() && !modifiers.shift() =>
            {
                Some(Message::Dashboard {
                    layout_id: None,
                    event: crate::screen::dashboard::Message::DrawingUndo,
                })
            }
            // Ctrl+D → Duplicate selected drawing
            keyboard::Key::Character(ref c)
                if c.as_str() == "d" && modifiers.command() =>
            {
                Some(Message::Dashboard {
                    layout_id: None,
                    event: crate::screen::dashboard::Message::DrawingDuplicate,
                })
            }
            // Home → Scroll to latest
            keyboard::Key::Named(keyboard::key::Named::Home) => Some(Message::Dashboard {
                layout_id: None,
                event: crate::screen::dashboard::Message::ScrollToLatest,
            }),
            // + → Zoom in
            keyboard::Key::Character(ref c)
                if (c.as_str() == "+" || c.as_str() == "=") && !modifiers.command() =>
            {
                Some(Message::Dashboard {
                    layout_id: None,
                    event: crate::screen::dashboard::Message::ZoomStep(0.5),
                })
            }
            // - → Zoom out
            keyboard::Key::Character(ref c)
                if c.as_str() == "-" && !modifiers.command() =>
            {
                Some(Message::Dashboard {
                    layout_id: None,
                    event: crate::screen::dashboard::Message::ZoomStep(-0.5),
                })
            }
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
