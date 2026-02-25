use std::sync::OnceLock;

use super::super::init::services;

// ── Channel-based event delivery ──────────────────────────────────────────────
//
// Each subsystem gets an (UnboundedSender, Mutex<Option<UnboundedReceiver>>)
// pair stored in a OnceLock.  The sender is cloned freely by background tasks;
// the receiver is taken exactly once by the subscription stream function.

// ── Rithmic ───────────────────────────────────────────────────────────────────

static RITHMIC_CHANNEL: OnceLock<(
    tokio::sync::mpsc::UnboundedSender<exchange::Event>,
    std::sync::Mutex<Option<tokio::sync::mpsc::UnboundedReceiver<exchange::Event>>>,
)> = OnceLock::new();

fn init_rithmic_channel() -> &'static (
    tokio::sync::mpsc::UnboundedSender<exchange::Event>,
    std::sync::Mutex<Option<tokio::sync::mpsc::UnboundedReceiver<exchange::Event>>>,
) {
    RITHMIC_CHANNEL.get_or_init(|| {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        (tx, std::sync::Mutex::new(Some(rx)))
    })
}

/// Get the Rithmic event sender (for background tasks to push events).
pub(crate) fn get_rithmic_sender()
-> &'static tokio::sync::mpsc::UnboundedSender<exchange::Event> {
    &init_rithmic_channel().0
}

/// Take the Rithmic event receiver (called once by the subscription stream).
pub(crate) fn take_rithmic_receiver()
-> Option<tokio::sync::mpsc::UnboundedReceiver<exchange::Event>> {
    init_rithmic_channel().1.lock().ok()?.take()
}

// ── Replay ────────────────────────────────────────────────────────────────────

static REPLAY_CHANNEL: OnceLock<(
    tokio::sync::mpsc::UnboundedSender<data::services::ReplayEvent>,
    std::sync::Mutex<Option<tokio::sync::mpsc::UnboundedReceiver<data::services::ReplayEvent>>>,
)> = OnceLock::new();

fn init_replay_channel() -> &'static (
    tokio::sync::mpsc::UnboundedSender<data::services::ReplayEvent>,
    std::sync::Mutex<Option<tokio::sync::mpsc::UnboundedReceiver<data::services::ReplayEvent>>>,
) {
    REPLAY_CHANNEL.get_or_init(|| {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        (tx, std::sync::Mutex::new(Some(rx)))
    })
}

/// Get the Replay event sender.
pub(crate) fn get_replay_sender()
-> &'static tokio::sync::mpsc::UnboundedSender<data::services::ReplayEvent> {
    &init_replay_channel().0
}

/// Take the Replay event receiver (called once by the subscription stream).
pub(crate) fn take_replay_receiver()
-> Option<tokio::sync::mpsc::UnboundedReceiver<data::services::ReplayEvent>> {
    init_replay_channel().1.lock().ok()?.take()
}

// ── Download ──────────────────────────────────────────────────────────────────

/// Progress update from the download subsystem.
#[derive(Debug, Clone)]
pub(crate) struct DownloadProgressEvent {
    pub(crate) pane_id: uuid::Uuid,
    pub(crate) current: usize,
    pub(crate) total: usize,
}

static DOWNLOAD_CHANNEL: OnceLock<(
    tokio::sync::mpsc::UnboundedSender<DownloadProgressEvent>,
    std::sync::Mutex<Option<tokio::sync::mpsc::UnboundedReceiver<DownloadProgressEvent>>>,
)> = OnceLock::new();

fn init_download_channel() -> &'static (
    tokio::sync::mpsc::UnboundedSender<DownloadProgressEvent>,
    std::sync::Mutex<Option<tokio::sync::mpsc::UnboundedReceiver<DownloadProgressEvent>>>,
) {
    DOWNLOAD_CHANNEL.get_or_init(|| {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        (tx, std::sync::Mutex::new(Some(rx)))
    })
}

/// Get the Download progress sender.
pub(crate) fn get_download_sender()
-> &'static tokio::sync::mpsc::UnboundedSender<DownloadProgressEvent> {
    &init_download_channel().0
}

/// Take the Download progress receiver (called once by the subscription stream).
pub(crate) fn take_download_receiver()
-> Option<tokio::sync::mpsc::UnboundedReceiver<DownloadProgressEvent>> {
    init_download_channel().1.lock().ok()?.take()
}

// ── Backtest ──────────────────────────────────────────────────────────────────

static BACKTEST_CHANNEL: OnceLock<(
    tokio::sync::mpsc::UnboundedSender<backtest::BacktestProgressEvent>,
    std::sync::Mutex<
        Option<tokio::sync::mpsc::UnboundedReceiver<backtest::BacktestProgressEvent>>,
    >,
)> = OnceLock::new();

fn init_backtest_channel() -> &'static (
    tokio::sync::mpsc::UnboundedSender<backtest::BacktestProgressEvent>,
    std::sync::Mutex<
        Option<tokio::sync::mpsc::UnboundedReceiver<backtest::BacktestProgressEvent>>,
    >,
) {
    BACKTEST_CHANNEL.get_or_init(|| {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        (tx, std::sync::Mutex::new(Some(rx)))
    })
}

/// Get the Backtest progress sender.
pub(crate) fn get_backtest_sender()
-> &'static tokio::sync::mpsc::UnboundedSender<backtest::BacktestProgressEvent> {
    &init_backtest_channel().0
}

/// Take the Backtest progress receiver (called once by the subscription stream).
pub(crate) fn take_backtest_receiver()
-> Option<tokio::sync::mpsc::UnboundedReceiver<backtest::BacktestProgressEvent>> {
    init_backtest_channel().1.lock().ok()?.take()
}

// ── AI stream ─────────────────────────────────────────────────────────────────

static AI_CHANNEL: OnceLock<(
    tokio::sync::mpsc::UnboundedSender<AiStreamEventClone>,
    std::sync::Mutex<Option<tokio::sync::mpsc::UnboundedReceiver<AiStreamEventClone>>>,
)> = OnceLock::new();

fn init_ai_channel() -> &'static (
    tokio::sync::mpsc::UnboundedSender<AiStreamEventClone>,
    std::sync::Mutex<Option<tokio::sync::mpsc::UnboundedReceiver<AiStreamEventClone>>>,
) {
    AI_CHANNEL.get_or_init(|| {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        (tx, std::sync::Mutex::new(Some(rx)))
    })
}

/// Get the AI stream event sender.
pub(crate) fn get_ai_sender()
-> &'static tokio::sync::mpsc::UnboundedSender<AiStreamEventClone> {
    &init_ai_channel().0
}

/// Take the AI stream event receiver (called once by the subscription stream).
pub(crate) fn take_ai_receiver()
-> Option<tokio::sync::mpsc::UnboundedReceiver<AiStreamEventClone>> {
    init_ai_channel().1.lock().ok()?.take()
}

// ── AI event types ────────────────────────────────────────────────────────────

/// A single tool call + result pair, used to sync back to api_history.
#[derive(Debug, Clone)]
pub struct ToolRoundSync {
    pub call_id: String,
    pub name: String,
    pub arguments: String,
    pub result_json: String,
}

/// Clone-safe AI stream event for passing through the channel.
/// All fields are Clone-able (String-based, no non-Clone types).
#[derive(Debug, Clone)]
pub enum AiStreamEventClone {
    Delta {
        conversation_id: uuid::Uuid,
        text: String,
    },
    ToolCallStarted {
        conversation_id: uuid::Uuid,
        call_id: String,
        name: String,
        arguments_json: String,
        display_summary: String,
    },
    ToolCallResult {
        conversation_id: uuid::Uuid,
        call_id: String,
        name: String,
        content_json: String,
        display_summary: String,
        is_error: bool,
    },
    /// Marks the end of a text segment (before tool calls start).
    TextSegmentComplete {
        conversation_id: uuid::Uuid,
    },
    Complete {
        conversation_id: uuid::Uuid,
        prompt_tokens: u32,
        completion_tokens: u32,
    },
    Error {
        conversation_id: uuid::Uuid,
        error: String,
    },
    /// Sync tool call rounds back to the pane's api_history so
    /// follow-up messages include prior tool context.
    ApiHistorySync {
        conversation_id: uuid::Uuid,
        rounds: Vec<ToolRoundSync>,
        /// Final assistant text (if any) produced after all tool rounds.
        final_text: Option<String>,
    },
    ApiKeyMissing {
        conversation_id: uuid::Uuid,
    },
    /// AI-initiated drawing action to be applied on the main thread.
    DrawingAction {
        conversation_id: uuid::Uuid,
        action: super::super::update::ai::AiDrawingAction,
    },
}

// ── Rithmic service result staging slot ──────────────────────────────────────
// Justified exception: RithmicServiceResult is non-Clone and consumed once.
// It cannot use the channel pattern above.

static RITHMIC_SERVICE_RESULT: OnceLock<
    std::sync::Arc<std::sync::Mutex<Option<services::RithmicServiceResult>>>,
> = OnceLock::new();

pub(crate) fn get_rithmic_service_staging()
-> &'static std::sync::Arc<std::sync::Mutex<Option<services::RithmicServiceResult>>> {
    RITHMIC_SERVICE_RESULT.get_or_init(|| std::sync::Arc::new(std::sync::Mutex::new(None)))
}
