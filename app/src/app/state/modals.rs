//! Modal overlay state: all transient UI panels and dialogs, plus the backtest subsystem.

/// Backtest feature state (grouped here because backtest is sidebar/modal-scoped,
/// not a top-level application concern).
pub(crate) struct BacktestState {
    pub(crate) strategy_registry: ::backtest::StrategyRegistry,
    pub(crate) backtest_launch_modal:
        crate::screen::backtest::launch::BacktestLaunchModal,
    pub(crate) show_backtest_modal: bool,
    pub(crate) backtest_trade_repo:
        Option<std::sync::Arc<dyn data::TradeRepository>>,
    pub(crate) backtest_history:
        crate::app::backtest_history::BacktestHistory,
    pub(crate) backtest_manager:
        crate::screen::backtest::manager::BacktestManager,
    pub(crate) show_backtest_manager: bool,
}

pub(crate) struct ModalState {
    pub(crate) theme_editor: crate::modals::ThemeEditor,
    pub(crate) data_management_panel:
        crate::modals::download::DataManagementPanel,
    pub(crate) connections_menu: crate::modals::connections::ConnectionsMenu,
    pub(crate) data_feeds_modal: crate::modals::data_feeds::DataFeedsModal,
    pub(crate) api_key_setup_modal:
        Option<crate::modals::download::ApiKeySetupModal>,
    pub(crate) historical_download_modal:
        Option<crate::modals::download::HistoricalDownloadModal>,
    pub(crate) historical_download_id: Option<uuid::Uuid>,
    pub(crate) replay_manager: crate::modals::replay::ReplayManager,
    pub(crate) backtest: BacktestState,
}
