use iced::{keyboard, Subscription};
use futures::stream::StreamExt;

use super::Message;
use crate::screen::dashboard::tickers_table::TickersTable;
use crate::window;

/// Download progress monitoring subscription
/// Uses global download progress state to avoid Subscription capture issues
pub fn download_progress_monitor() -> impl futures::stream::Stream<Item = Message> {
    futures::stream::unfold((), |_| async {
        // Sleep for 200ms between polls
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        // Check global progress state
        if let Ok(progress) = super::get_download_progress().lock() {
            if !progress.is_empty() {
                // Get first active download (could iterate all if needed)
                if let Some((&pane_id, &(current, total))) = progress.iter().next() {
                    // Emit progress message
                    return Some((Message::DataDownloadProgress { pane_id, current, total }, ()));
                }
            }
        }

        // No active downloads, emit dummy message that will be filtered
        Some((Message::DataDownloadProgress {
            pane_id: uuid::Uuid::nil(),
            current: 0,
            total: 0
        }, ()))
    })
    .filter(|msg| {
        // Filter out dummy messages (total=0 indicates no active downloads)
        futures::future::ready(match msg {
            Message::DataDownloadProgress { total, .. } => *total > 0,
            _ => false,
        })
    })
}

/// Build the main application subscription
pub fn build_subscription(tickers_table: &TickersTable) -> Subscription<Message> {
    let window_events = window::events().map(Message::WindowEvent);
    let tickers_sub = tickers_table.subscription().map(Message::TickersTable);

    let tick = iced::time::every(std::time::Duration::from_millis(100)).map(Message::Tick);

    // Poll for loading status updates every 500ms
    let status_poll = iced::time::every(std::time::Duration::from_millis(500))
        .map(|_| Message::UpdateLoadingStatus);

    // Download progress monitoring subscription
    let download_poll = Subscription::run(download_progress_monitor);

    let hotkeys = keyboard::listen().filter_map(|event| {
        let keyboard::Event::KeyPressed { key, .. } = event else {
            return None;
        };
        match key {
            keyboard::Key::Named(keyboard::key::Named::Escape) => Some(Message::GoBack),
            _ => None,
        }
    });

    Subscription::batch(vec![
        tickers_sub,
        window_events,
        tick,
        status_poll,
        download_poll,
        hotkeys,
    ])
}
