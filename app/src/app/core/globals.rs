use std::sync::OnceLock;

// ── Channel-based event delivery ──────────────────────────────────────────────
//
// Each subsystem gets an (UnboundedSender, Mutex<Option<UnboundedReceiver>>)
// pair stored in a OnceLock.  The sender is cloned freely by background tasks;
// the receiver is taken exactly once by the subscription stream function.

/// Sender + guarded receiver pair for an unbounded channel.
type ChannelPair<T> = (
    tokio::sync::mpsc::UnboundedSender<T>,
    std::sync::Mutex<Option<tokio::sync::mpsc::UnboundedReceiver<T>>>,
);

/// Generic unbounded channel stored in a static `OnceLock`.
///
/// Lazily initialises on first access. The sender can be cloned freely;
/// the receiver is taken exactly once (by the subscription stream).
struct EventChannel<T: Send + 'static> {
    inner: OnceLock<ChannelPair<T>>,
}

impl<T: Send + 'static> EventChannel<T> {
    const fn new() -> Self {
        Self {
            inner: OnceLock::new(),
        }
    }

    fn init(&self) -> &ChannelPair<T> {
        self.inner.get_or_init(|| {
            let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
            (tx, std::sync::Mutex::new(Some(rx)))
        })
    }

    fn sender(&self) -> &tokio::sync::mpsc::UnboundedSender<T> {
        &self.init().0
    }

    fn take_receiver(&self) -> Option<tokio::sync::mpsc::UnboundedReceiver<T>> {
        match self.init().1.lock() {
            Ok(mut guard) => guard.take(),
            Err(e) => {
                log::error!("EventChannel mutex poisoned: {}", e);
                e.into_inner().take()
            }
        }
    }
}

// ── DataEngine event slot ─────────────────────────────────────────────────────
// The DataEngine's event receiver is taken once by the subscription stream.

static DATA_ENGINE_EVENT_SLOT: OnceLock<
    std::sync::Arc<std::sync::Mutex<Option<tokio::sync::mpsc::UnboundedReceiver<data::DataEvent>>>>,
> = OnceLock::new();

pub(crate) fn get_data_event_slot() -> &'static std::sync::Arc<
    std::sync::Mutex<Option<tokio::sync::mpsc::UnboundedReceiver<data::DataEvent>>>,
> {
    DATA_ENGINE_EVENT_SLOT.get_or_init(|| std::sync::Arc::new(std::sync::Mutex::new(None)))
}

pub(crate) fn set_data_event_receiver(rx: tokio::sync::mpsc::UnboundedReceiver<data::DataEvent>) {
    let slot = get_data_event_slot();
    if let Ok(mut guard) = slot.lock() {
        *guard = Some(rx);
    }
}

pub(crate) fn take_data_event_receiver()
-> Option<tokio::sync::mpsc::UnboundedReceiver<data::DataEvent>> {
    match get_data_event_slot().lock() {
        Ok(mut guard) => guard.take(),
        Err(e) => e.into_inner().take(),
    }
}

// ── Rithmic client staging ────────────────────────────────────────────────────
// The connected RithmicClient is staged here so it can be moved into the app
// state after the async connect task completes on the blocking thread.

/// Shared handle to a `RithmicClient` behind async + sync mutexes.
type RithmicClientHandle = std::sync::Arc<tokio::sync::Mutex<data::RithmicClient>>;

/// Staged result from Rithmic connect: the client handle plus the
/// cache-scanned DataIndex (so the app layer can merge it synchronously
/// before resolving chart ranges — avoids a race with the async event).
type RithmicStagedResult = (RithmicClientHandle, data::DataIndex);

/// Staging slot: an `Arc<Mutex<Option<...>>>` so the async connect task
/// can deposit a connected client + cache index for the main thread to
/// pick up.
type RithmicClientSlot = std::sync::Arc<std::sync::Mutex<Option<RithmicStagedResult>>>;

static RITHMIC_CLIENT_STAGING: OnceLock<RithmicClientSlot> = OnceLock::new();

pub(crate) fn get_rithmic_client_staging() -> &'static RithmicClientSlot {
    RITHMIC_CLIENT_STAGING.get_or_init(|| std::sync::Arc::new(std::sync::Mutex::new(None)))
}

// ── Replay ────────────────────────────────────────────────────────────────────

static REPLAY_CHANNEL: EventChannel<crate::services::ReplayEvent> = EventChannel::new();

/// Get the Replay event sender.
pub(crate) fn get_replay_sender()
-> &'static tokio::sync::mpsc::UnboundedSender<crate::services::ReplayEvent> {
    REPLAY_CHANNEL.sender()
}

/// Take the Replay event receiver (called once by the subscription stream).
pub(crate) fn take_replay_receiver()
-> Option<tokio::sync::mpsc::UnboundedReceiver<crate::services::ReplayEvent>> {
    REPLAY_CHANNEL.take_receiver()
}

// ── Backtest ──────────────────────────────────────────────────────────────────

static BACKTEST_CHANNEL: EventChannel<backtest::BacktestProgressEvent> = EventChannel::new();

/// Get the Backtest progress sender.
pub(crate) fn get_backtest_sender()
-> &'static tokio::sync::mpsc::UnboundedSender<backtest::BacktestProgressEvent> {
    BACKTEST_CHANNEL.sender()
}

/// Take the Backtest progress receiver (called once by the subscription stream).
pub(crate) fn take_backtest_receiver()
-> Option<tokio::sync::mpsc::UnboundedReceiver<backtest::BacktestProgressEvent>> {
    BACKTEST_CHANNEL.take_receiver()
}

// ── AI stream ─────────────────────────────────────────────────────────────────

/// Re-export from the AI crate for app-wide use.
pub(crate) use ai::AiStreamEvent;

static AI_CHANNEL: EventChannel<AiStreamEvent> = EventChannel::new();

/// Get the AI stream event sender.
pub(crate) fn get_ai_sender() -> &'static tokio::sync::mpsc::UnboundedSender<AiStreamEvent> {
    AI_CHANNEL.sender()
}

/// Take the AI stream event receiver (called once by the subscription stream).
pub(crate) fn take_ai_receiver() -> Option<tokio::sync::mpsc::UnboundedReceiver<AiStreamEvent>> {
    AI_CHANNEL.take_receiver()
}

// ── Auto-update ─────────────────────────────────────────────────────────────

static UPDATE_CHANNEL: EventChannel<crate::services::updater::UpdateEvent> = EventChannel::new();

pub(crate) fn get_update_sender()
-> &'static tokio::sync::mpsc::UnboundedSender<crate::services::updater::UpdateEvent> {
    UPDATE_CHANNEL.sender()
}

pub(crate) fn take_update_receiver()
-> Option<tokio::sync::mpsc::UnboundedReceiver<crate::services::updater::UpdateEvent>> {
    UPDATE_CHANNEL.take_receiver()
}
