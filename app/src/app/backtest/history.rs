use std::sync::Arc;
use uuid::Uuid;

/// Lifecycle status of a backtest run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BacktestStatus {
    Running,
    Completed,
    Failed,
}

/// A single entry in the backtest history.
#[allow(dead_code)]
pub struct BacktestHistoryEntry {
    pub id: Uuid,
    pub status: BacktestStatus,
    pub strategy_name: String,
    pub ticker: String,
    pub config: ::backtest::BacktestConfig,
    pub started_at_ms: u64,
    pub progress: f32,
    pub progress_message: String,
    pub live_trades: Vec<::backtest::TradeRecord>,
    pub live_equity: Vec<::backtest::EquityPoint>,
    pub initial_capital: f64,
    pub result: Option<Arc<::backtest::BacktestResult>>,
    pub error: Option<String>,
}

/// In-memory registry of all backtest runs during this session.
pub struct BacktestHistory {
    entries: Vec<BacktestHistoryEntry>,
}

impl BacktestHistory {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Add a new running backtest entry.
    pub fn add_running(
        &mut self,
        id: Uuid,
        strategy_name: String,
        ticker: String,
        config: ::backtest::BacktestConfig,
    ) {
        let initial_capital = config.initial_capital_usd;
        self.entries.push(BacktestHistoryEntry {
            id,
            status: BacktestStatus::Running,
            strategy_name,
            ticker,
            config,
            started_at_ms: system_time_ms(),
            progress: 0.0,
            progress_message: String::new(),
            live_trades: Vec::new(),
            live_equity: Vec::new(),
            initial_capital,
            result: None,
            error: None,
        });
    }

    pub fn update_progress(&mut self, id: Uuid, pct: f32, message: String) {
        if let Some(entry) = self.get_mut(id) {
            entry.progress = pct;
            entry.progress_message = message;
        }
    }

    pub fn append_live_trade(&mut self, id: Uuid, trade: ::backtest::TradeRecord) {
        if let Some(entry) = self.get_mut(id) {
            entry.live_trades.push(trade);
        }
    }

    pub fn append_live_equity(&mut self, id: Uuid, point: ::backtest::EquityPoint) {
        if let Some(entry) = self.get_mut(id) {
            entry.live_equity.push(point);
        }
    }

    pub fn mark_completed(&mut self, id: Uuid, result: Arc<::backtest::BacktestResult>) {
        if let Some(entry) = self.get_mut(id) {
            entry.status = BacktestStatus::Completed;
            entry.progress = 1.0;
            entry.result = Some(result);
        }
    }

    pub fn mark_failed(&mut self, id: Uuid, error: String) {
        if let Some(entry) = self.get_mut(id) {
            entry.status = BacktestStatus::Failed;
            entry.error = Some(error);
        }
    }

    pub fn get(&self, id: Uuid) -> Option<&BacktestHistoryEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn get_mut(&mut self, id: Uuid) -> Option<&mut BacktestHistoryEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    /// All entries sorted newest first.
    pub fn all_sorted(&self) -> Vec<&BacktestHistoryEntry> {
        let mut sorted: Vec<&BacktestHistoryEntry> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.started_at_ms.cmp(&a.started_at_ms));
        sorted
    }

    pub fn remove(&mut self, id: Uuid) {
        self.entries.retain(|e| e.id != id);
    }

    #[allow(dead_code)]
    pub fn running_ids(&self) -> Vec<Uuid> {
        self.entries
            .iter()
            .filter(|e| e.status == BacktestStatus::Running)
            .map(|e| e.id)
            .collect()
    }
}

fn system_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
