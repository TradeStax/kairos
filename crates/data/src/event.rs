//! Data events — emitted by adapters and the DataEngine facade.
//!
//! `DataEvent` is delivered via `mpsc::UnboundedReceiver<DataEvent>` returned
//! from `DataEngine::new()`. Replaces all `OnceLock<Arc<Mutex<>>>` global
//! staging in the app layer.

use crate::connection::types::ConnectionProvider;
#[cfg(feature = "heatmap")]
use crate::domain::Depth;
use crate::domain::index::DataIndex;
use crate::domain::types::FeedId;
use crate::domain::{FuturesTicker, Trade};
use uuid::Uuid;

/// Events emitted by the DataEngine and adapters
#[derive(Debug, Clone)]
pub enum DataEvent {
    // ── Connection lifecycle ──────────────────────────────────────────
    Connected {
        feed_id: FeedId,
        provider: ConnectionProvider,
    },
    Disconnected {
        feed_id: FeedId,
        reason: String,
    },
    ConnectionLost {
        feed_id: FeedId,
    },
    Reconnecting {
        feed_id: FeedId,
        attempt: u32,
    },

    // ── Live market data ──────────────────────────────────────────────
    TradeReceived {
        ticker: FuturesTicker,
        trade: Trade,
    },
    #[cfg(feature = "heatmap")]
    DepthReceived {
        ticker: FuturesTicker,
        depth: Depth,
    },

    // ── Subscriptions ─────────────────────────────────────────────────
    SubscriptionActive {
        ticker: FuturesTicker,
    },
    SubscriptionFailed {
        ticker: FuturesTicker,
        reason: String,
    },
    ProductCodesReceived(Vec<String>),

    // ── Download progress ─────────────────────────────────────────────
    DownloadProgress {
        request_id: Uuid,
        current_day: usize,
        total_days: usize,
    },
    DownloadComplete {
        request_id: Uuid,
        days_cached: usize,
    },

    // ── Data availability ─────────────────────────────────────────────
    DataIndexUpdated(DataIndex),
}
