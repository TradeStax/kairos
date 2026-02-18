//! Exchange Repository Implementations
//!
//! Data source-specific implementations of repository traits.
//! - Databento: Futures market data
//! - Massive: Options market data

pub mod databento_depth;
pub mod databento_trades;
pub mod massive_chains;
pub mod massive_contracts;
pub mod massive_snapshots;

pub use databento_depth::DatabentoDepthRepository;
pub use databento_trades::DatabentoTradeRepository;
pub use massive_chains::MassiveChainRepository;
pub use massive_contracts::MassiveContractRepository;
pub use massive_snapshots::MassiveSnapshotRepository;

use crate::adapter::massive::MassiveError;
use flowsurface_data::repository::RepositoryError;

/// Convert MassiveError to RepositoryError (shared by massive repos)
pub(crate) fn convert_massive_error(e: MassiveError) -> RepositoryError {
    match e {
        MassiveError::SymbolNotFound(s) => RepositoryError::NotFound(s),
        MassiveError::Cache(s) => RepositoryError::Cache(s),
        MassiveError::Parse(s) => RepositoryError::Serialization(s),
        MassiveError::InvalidData(s) => RepositoryError::InvalidData(s),
        MassiveError::Io(e) => RepositoryError::Io(e),
        _ => RepositoryError::Remote(e.to_string()),
    }
}
