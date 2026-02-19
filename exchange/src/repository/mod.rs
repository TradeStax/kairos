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

use crate::adapter::massive::MassiveError;
use flowsurface_data::repository::RepositoryError;

/// Convert a MassiveError into a RepositoryError
fn convert_massive_error(e: MassiveError) -> RepositoryError {
    match e {
        MassiveError::SymbolNotFound(s) => RepositoryError::NotFound(s),
        MassiveError::RateLimit(s) => RepositoryError::RateLimit(s),
        MassiveError::Cache(s) => RepositoryError::Cache(s),
        other => RepositoryError::Remote(other.to_string()),
    }
}
