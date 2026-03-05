use iced::{Subscription, keyboard};

use super::super::Message;
use crate::window;

/// DataEngine event monitor.
/// Blocks on recv() until an event arrives — zero CPU when idle.
/// Replaces the old rithmic_event_monitor and download_progress_monitor.
///
/// Polls for the receiver until it becomes available (DataEngine may
/// initialize after the subscription starts). The stream only ends
/// when the channel is actually closed (all senders dropped).
fn data_event_monitor() -> impl futures::stream::Stream<Item = Message> {
    futures::stream::unfold(
        None::<tokio::sync::mpsc::UnboundedReceiver<data::DataEvent>>,
        |maybe_rx| async move {
            // Acquire receiver if we don't have one yet
            let mut rx = match maybe_rx {
                Some(rx) => rx,
                None => loop {
                    if let Some(rx) = super::globals::take_data_event_receiver() {
                        log::info!("DataEngine event monitor: receiver acquired");
                        break rx;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                },
            };

            // Wait for next event
            match rx.recv().await {
                Some(event) => Some((Message::DataEvent(event), Some(rx))),
                None => {
                    log::warn!("DataEngine event channel closed");
                    None
                }
            }
        },
    )
}

/// Replay engine event monitor.
/// Polls for the receiver until it becomes available (replay engine may
/// initialize after the subscription starts). The stream only ends
/// when the channel is actually closed (all senders dropped).
fn replay_event_monitor() -> impl futures::stream::Stream<Item = Message> {
    futures::stream::unfold(
        None::<tokio::sync::mpsc::UnboundedReceiver<crate::services::ReplayEvent>>,
        |maybe_rx| async move {
            let mut rx = match maybe_rx {
                Some(rx) => rx,
                None => loop {
                    if let Some(rx) = super::globals::take_replay_receiver() {
                        log::info!("Replay event monitor: receiver acquired");
                        break rx;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                },
            };

            match rx.recv().await {
                Some(event) => Some((Message::ReplayEvent(event), Some(rx))),
                None => {
                    log::warn!("Replay event channel closed");
                    None
                }
            }
        },
    )
}

/// Backtest progress event monitor.
/// Blocks on recv() until an event arrives — zero CPU when idle.
fn backtest_event_monitor() -> impl futures::stream::Stream<Item = Message> {
    let receiver = super::globals::take_backtest_receiver();
    futures::stream::unfold(receiver, |maybe_rx| async move {
        match maybe_rx {
            Some(mut rx) => rx.recv().await.map(|event| {
                (
                    Message::Backtest(super::super::messages::BacktestMessage::ProgressEvent(
                        event,
                    )),
                    Some(rx),
                )
            }),
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
            Some(mut rx) => rx
                .recv()
                .await
                .map(|event| (Message::AiStreamEvent(event), Some(rx))),
            None => {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                None
            }
        }
    })
}

/// Auto-update event monitor.
/// Blocks on recv() until an event arrives — zero CPU when idle.
fn update_event_monitor() -> impl futures::stream::Stream<Item = Message> {
    futures::stream::unfold(
        None::<tokio::sync::mpsc::UnboundedReceiver<crate::services::updater::UpdateEvent>>,
        |maybe_rx| async move {
            let mut rx = match maybe_rx {
                Some(rx) => rx,
                None => loop {
                    if let Some(rx) = super::globals::take_update_receiver() {
                        log::info!("Update event monitor: receiver acquired");
                        break rx;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                },
            };
            match rx.recv().await {
                Some(event) => {
                    let msg = match event {
                        crate::services::updater::UpdateEvent::DownloadProgress {
                            downloaded,
                            total,
                        } => Message::Update(
                            super::super::messages::UpdateMessage::DownloadProgress {
                                downloaded,
                                total,
                            },
                        ),
                        crate::services::updater::UpdateEvent::DownloadComplete(result) => {
                            Message::Update(
                                super::super::messages::UpdateMessage::DownloadComplete(result),
                            )
                        }
                        crate::services::updater::UpdateEvent::CheckComplete(result) => {
                            Message::Update(super::super::messages::UpdateMessage::CheckComplete(
                                result.map(Some),
                            ))
                        }
                    };
                    Some((msg, Some(rx)))
                }
                None => {
                    log::warn!("Update event channel closed");
                    None
                }
            }
        },
    )
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

    // DataEngine event subscription (replaces rithmic_poll and download_poll)
    let data_event_poll = Subscription::run(data_event_monitor);

    // Replay engine event subscription
    let replay_poll = Subscription::run(replay_event_monitor);

    // Backtest progress event subscription
    let backtest_poll = Subscription::run(backtest_event_monitor);

    // AI assistant stream event subscription
    let ai_poll = Subscription::run(ai_stream_monitor);

    // Auto-update event subscription
    let update_poll = Subscription::run(update_event_monitor);

    let hotkeys = keyboard::listen().filter_map(|event| {
        let keyboard::Event::KeyPressed { key, modifiers, .. } = event else {
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
                    event: Box::new(crate::screen::dashboard::Message::DrawingRedo),
                })
            }
            // Ctrl+Z → Undo drawing
            keyboard::Key::Character(ref c)
                if c.as_str() == "z" && modifiers.command() && !modifiers.shift() =>
            {
                Some(Message::Dashboard {
                    layout_id: None,
                    event: Box::new(crate::screen::dashboard::Message::DrawingUndo),
                })
            }
            // Ctrl+D → Duplicate selected drawing
            keyboard::Key::Character(ref c) if c.as_str() == "d" && modifiers.command() => {
                Some(Message::Dashboard {
                    layout_id: None,
                    event: Box::new(crate::screen::dashboard::Message::DrawingDuplicate),
                })
            }
            // Home → Scroll to latest
            keyboard::Key::Named(keyboard::key::Named::Home) => Some(Message::Dashboard {
                layout_id: None,
                event: Box::new(crate::screen::dashboard::Message::ScrollToLatest),
            }),
            // + → Zoom in
            keyboard::Key::Character(ref c)
                if (c.as_str() == "+" || c.as_str() == "=") && !modifiers.command() =>
            {
                Some(Message::Dashboard {
                    layout_id: None,
                    event: Box::new(crate::screen::dashboard::Message::ZoomStep(0.5)),
                })
            }
            // - → Zoom out
            keyboard::Key::Character(ref c) if c.as_str() == "-" && !modifiers.command() => {
                Some(Message::Dashboard {
                    layout_id: None,
                    event: Box::new(crate::screen::dashboard::Message::ZoomStep(-0.5)),
                })
            }
            _ => None,
        }
    });

    let mut subs = vec![
        window_events,
        tick,
        status_poll,
        data_event_poll,
        replay_poll,
        backtest_poll,
        ai_poll,
        update_poll,
        hotkeys,
    ];

    if replay_is_dragging {
        subs.push(replay_drag_subscription());
    }

    Subscription::batch(subs)
}
