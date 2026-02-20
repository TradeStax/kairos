//! Repository Pattern - Data Access Abstraction
//!
//! This module provides clean trait-based abstractions for accessing market data.
//! Implementations live in the exchange layer where they belong.
//!
//! ## Clean Architecture
//!
//! ```text
//! MarketDataService (data layer)
//!      ↓
//! TradeRepository trait (interface)
//!      ↓
//! DatabentoTradeRepository (exchange layer)
//!      ↓
//! HistoricalDataManager (handles cache + API)
//!      ├─→ [.dbn.zst cache files] (fast, local)
//!      └─→ Databento API (slow, expensive)
//! ```
//!
//! ## Key Benefits
//! - **Abstraction**: Services don't know about Databento specifics
//! - **Testability**: Easy to mock repositories
//! - **Flexibility**: Can swap exchange adapters
//! - **Single Responsibility**: Exchange layer owns cache format
//!
//! ## Usage
//!
//! ```rust,ignore
//! use data::repository::TradeRepository;
//! use exchange::DatabentoTradeRepository;
//!
//! // Exchange layer provides the implementation
//! let repo: Arc<dyn TradeRepository> = Arc::new(
//!     DatabentoTradeRepository::new(config).await?
//! );
//!
//! // Data layer uses the trait
//! let service = MarketDataService::new(repo, depth_repo);
//! ```

pub mod composite;
pub mod traits;

// Re-export traits for convenience
pub use traits::{
    CacheCoverageReport, DepthRepository, OptionChainRepository, OptionContractRepository,
    OptionSnapshotRepository, RepositoryError, RepositoryResult, RepositoryStats, TradeRepository,
};

pub use composite::{CompositeTradeRepository, FeedRepo};
