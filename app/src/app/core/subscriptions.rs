use iced::{Subscription, keyboard};

use super::super::{DownloadMessage, Message};
use crate::infra::window;

/// Rithmic streaming event monitor.
/// Blocks on recv() until an event arrives — zero CPU when idle.
fn rithmic_event_monitor() -> impl futures::stream::Stream<Item = Message> {
    let receiver = super::globals::take_rithmic_receiver();
    futures::stream::unfold(receiver, |maybe_rx| async move {
        match maybe_rx {
            Some(mut rx) => match rx.recv().await {
                Some(event) => Some((Message::RithmicStreamEvent(event), Some(rx))),
                None => None, // sender dropped — channel closed
            },
            None => {
                // Receiver already taken or channel not initialized.
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                None
            }
        }
    })
}

/// Replay engine event monitor.
/// Blocks on recv() until an event arrives — zero CPU when idle.
fn replay_event_monitor() -> impl futures::stream::Stream<Item = Message> {
    let receiver = super::globals::take_replay_receiver();
    futures::stream::unfold(receiver, |maybe_rx| async move {
        match maybe_rx {
            Some(mut rx) => match rx.recv().await {
                Some(event) => Some((Message::ReplayEvent(event), Some(rx))),
                None => None,
            },
            None => {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                None
            }
        }
    })
}

/// Download progress monitoring subscription.
/// Blocks on recv() until a progress event arrives.
pub fn download_progress_monitor() -> impl futures::stream::Stream<Item = Message> {
    let receiver = super::globals::take_download_receiver();
    futures::stream::unfold(receiver, |maybe_rx| async move {
        match maybe_rx {
            Some(mut rx) => match rx.recv().await {
                Some(event) => Some((
                    Message::Download(DownloadMessage::DataDownloadProgress {
                        pane_id: event.pane_id,
                        current: event.current,
                        total: event.total,
                    }),
                    Some(rx),
                )),
                None => None,
            },
            None => {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                None
            }
        }
    })
}

/// Backtest progress event monitor.
/// Blocks on recv() until an event arrives — zero CPU when idle.
fn backtest_event_monitor() -> impl futures::stream::Stream<Item = Message> {
    let receiver = super::globals::take_backtest_receiver();
    futures::stream::unfold(receiver, |maybe_rx| async move {
        match maybe_rx {
            Some(mut rx) => match rx.recv().await {
                Some(event) => Some((
                    Message::Backtest(
                        super::super::messages::BacktestMessage::ProgressEvent(event),
                    ),
                    Some(rx),
                )),
                None => None,
            },
            None => {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                None
            }
        }
    })
}

/// AI assistant stream event monitor.
/// Blocks on recv() until an event arrives — zero CPU when idle.
fn ai_stream_monitor() -> impl futures::stream::Stream<Item = Message> {
    let receiver = super::globals::take_ai_receiver();
    futures::stream::unfold(receiver, |maybe_rx| async move {
        match maybe_rx {
            Some(mut rx) => match rx.recv().await {
                Some(event) => Some((Message::AiStreamEvent(event), Some(rx))),
                None => None,
            },
            None => {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                None
            }
        }
    })
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
        .map(|_| Message::Chart(super::super::ChartMessage::UpdateLoadingStatus));

    // Download progress monitoring subscription
    let download_poll = Subscription::run(download_progress_monitor);

    // Rithmic streaming event subscription
    let rithmic_poll = Subscription::run(rithmic_event_monitor);

    // Replay engine event subscription
    let replay_poll = Subscription::run(replay_event_monitor);

    // Backtest progress event subscription
    let backtest_poll = Subscription::run(backtest_event_monitor);

    // AI assistant stream event subscription
    let ai_poll = Subscription::run(ai_stream_monitor);

    let hotkeys = keyboard::listen().filter_map(|event| {
        let keyboard::Event::KeyPressed {
            key, modifiers, ..
        } = event
        else {
            return None;
        };
        match key {
            keyboard::Key::Named(keyboard::key::Named::Escape) => Some(Message::GoBack),
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
        backtest_poll,
        ai_poll,
        hotkeys,
    ];

    if replay_is_dragging {
        subs.push(replay_drag_subscription());
    }

    Subscription::batch(subs)
}
