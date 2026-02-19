//! Exchange Repository Implementations
//!
//! Data source-specific implementations of repository traits.
//! - Databento: Futures market data (historical)
//! - Rithmic: Futures market data (realtime + historical)
//! - Massive: Options market data

pub mod databento_depth;
pub mod databento_trades;
pub mod massive_chains;
pub mod massive_contracts;
pub mod massive_snapshots;
pub mod rithmic_depth;
pub mod rithmic_trades;

pub use databento_depth::DatabentoDepthRepository;
pub use databento_trades::DatabentoTradeRepository;
pub use massive_chains::MassiveChainRepository;
pub use massive_contracts::MassiveContractRepository;
pub use massive_snapshots::MassiveSnapshotRepository;
pub use rithmic_depth::RithmicDepthRepository;
pub use rithmic_trades::RithmicTradeRepository;
