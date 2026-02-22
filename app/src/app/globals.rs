use std::collections::HashMap;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

use super::services;

// Activity flags for polling guards — avoid spinning when no feed is active
static RITHMIC_ACTIVE: AtomicBool = AtomicBool::new(false);
static REPLAY_ACTIVE: AtomicBool = AtomicBool::new(false);
static DOWNLOAD_ACTIVE: AtomicBool = AtomicBool::new(false);

pub fn is_rithmic_active() -> bool {
    RITHMIC_ACTIVE.load(Ordering::Relaxed)
}

pub fn set_rithmic_active(active: bool) {
    RITHMIC_ACTIVE.store(active, Ordering::Relaxed);
}

pub fn is_replay_active() -> bool {
    REPLAY_ACTIVE.load(Ordering::Relaxed)
}

pub fn set_replay_active(active: bool) {
    REPLAY_ACTIVE.store(active, Ordering::Relaxed);
}

pub fn is_download_active() -> bool {
    DOWNLOAD_ACTIVE.load(Ordering::Relaxed)
}

pub fn set_download_active(active: bool) {
    DOWNLOAD_ACTIVE.store(active, Ordering::Relaxed);
}

// Global download progress state (shared between async tasks and subscriptions)
#[allow(clippy::type_complexity)]
static DOWNLOAD_PROGRESS: OnceLock<
    std::sync::Arc<std::sync::Mutex<HashMap<uuid::Uuid, (usize, usize)>>>,
> = OnceLock::new();

pub fn get_download_progress()
-> &'static std::sync::Arc<std::sync::Mutex<HashMap<uuid::Uuid, (usize, usize)>>> {
    DOWNLOAD_PROGRESS.get_or_init(|| std::sync::Arc::new(std::sync::Mutex::new(HashMap::new())))
}

// Global staging for Rithmic streaming events
static RITHMIC_EVENTS: OnceLock<std::sync::Arc<std::sync::Mutex<Vec<exchange::Event>>>> =
    OnceLock::new();

pub fn get_rithmic_events() -> &'static std::sync::Arc<std::sync::Mutex<Vec<exchange::Event>>> {
    RITHMIC_EVENTS.get_or_init(|| std::sync::Arc::new(std::sync::Mutex::new(Vec::new())))
}

// Global staging for Replay engine events
static REPLAY_EVENTS: OnceLock<
    std::sync::Arc<std::sync::Mutex<Vec<data::services::ReplayEvent>>>,
> = OnceLock::new();

pub fn get_replay_events()
-> &'static std::sync::Arc<std::sync::Mutex<Vec<data::services::ReplayEvent>>> {
    REPLAY_EVENTS.get_or_init(|| std::sync::Arc::new(std::sync::Mutex::new(Vec::new())))
}

// Staging slot for Rithmic service result (non-Clone, consumed once)
static RITHMIC_SERVICE_RESULT: OnceLock<
    std::sync::Arc<std::sync::Mutex<Option<services::RithmicServiceResult>>>,
> = OnceLock::new();

pub fn get_rithmic_service_staging()
-> &'static std::sync::Arc<std::sync::Mutex<Option<services::RithmicServiceResult>>> {
    RITHMIC_SERVICE_RESULT.get_or_init(|| std::sync::Arc::new(std::sync::Mutex::new(None)))
}
