//! Data events emitted by adapters and the [`DataEngine`](crate::engine::DataEngine).
//!
//! [`DataEvent`] is delivered via `mpsc::UnboundedReceiver<DataEvent>` returned
//! from `DataEngine::new()`. The app layer subscribes to this channel in an
//! Iced subscription to drive UI updates.

use crate::connection::types::ConnectionProvider;
#[cfg(feature = "heatmap")]
use crate::domain::Depth;
use crate::domain::index::DataIndex;
use crate::domain::types::FeedId;
use crate::domain::{FuturesTicker, Trade};
use uuid::Uuid;

/// Events emitted by the DataEngine and adapters.
///
/// Covers connection lifecycle, live market data, subscription status,
/// download progress, and data availability changes.
#[derive(Debug, Clone)]
pub enum DataEvent {
    // ── Connection lifecycle ──────────────────────────────────────────
    /// An adapter successfully connected
    Connected {
        feed_id: FeedId,
        provider: ConnectionProvider,
    },
    /// An adapter disconnected (user-initiated or clean shutdown)
    Disconnected { feed_id: FeedId, reason: String },
    /// Connection was lost unexpectedly
    ConnectionLost { feed_id: FeedId },
    /// Adapter is attempting to reconnect
    Reconnecting { feed_id: FeedId, attempt: u32 },

    // ── Live market data ──────────────────────────────────────────────
    /// A new trade was received from a live feed
    TradeReceived { ticker: FuturesTicker, trade: Trade },
    /// A new depth snapshot was received from a live feed
    #[cfg(feature = "heatmap")]
    DepthReceived { ticker: FuturesTicker, depth: Depth },

    // ── Subscriptions ─────────────────────────────────────────────────
    /// A ticker subscription is now active
    SubscriptionActive { ticker: FuturesTicker },
    /// A ticker subscription failed to activate
    SubscriptionFailed {
        ticker: FuturesTicker,
        reason: String,
    },
    /// Available product codes received from the exchange
    ProductCodesReceived(Vec<String>),

    // ── Download progress ─────────────────────────────────────────────
    /// Progress update during a multi-day data download
    DownloadProgress {
        request_id: Uuid,
        current_day: usize,
        total_days: usize,
    },
    /// A data download completed successfully
    DownloadComplete {
        request_id: Uuid,
        days_cached: usize,
    },

    // ── Data availability ─────────────────────────────────────────────
    /// The data availability index was updated (new data cached or feed connected)
    DataIndexUpdated(DataIndex),
}
