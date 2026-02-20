pub mod chains;
pub mod contracts;
pub mod snapshots;

pub use chains::MassiveChainRepository;
pub use contracts::MassiveContractRepository;
pub use snapshots::MassiveSnapshotRepository;

use crate::adapter::massive::MassiveError;
use flowsurface_data::repository::RepositoryError;

/// Convert a MassiveError into a RepositoryError
pub(crate) fn convert_massive_error(e: MassiveError) -> RepositoryError {
    match e {
        MassiveError::SymbolNotFound(s) => RepositoryError::NotFound(s),
        MassiveError::RateLimit(s) => RepositoryError::RateLimit(s),
        MassiveError::Cache(s) => RepositoryError::Cache(s),
        other => RepositoryError::Remote(other.to_string()),
    }
}
