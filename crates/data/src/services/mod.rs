//! Service Layer - Business Logic Orchestration
//!
//! Services coordinate between repositories, apply business logic,
//! and provide high-level operations for the application layer.
//!
//! ## Architecture
//!
//! ```text
//! Application/GUI Layer
//!       ↓
//! Service Layer (this module)
//!   ├─ Orchestrates repository calls
//!   ├─ Applies business logic
//!   ├─ Handles caching strategy
//!   └─ Coordinates aggregation
//!       ↓
//! Repository Layer
//!   ├─ Data access abstraction
//!   └─ Cache + Remote coordination
//!       ↓
//! Exchange Layer
//!   └─ External API calls
//! ```
//!
//! ## Key Services
//!
//! - **MarketDataService**: Fetch and aggregate market data
//! - **CacheManagerService**: Manage cache lifecycle
//! - **ReplayEngineService**: Replay historical data

pub mod cache_manager;
pub mod feed_merger;
pub mod gex_calculator;
pub mod market_data;
pub mod options_data;
pub mod replay_engine;

// Re-export for convenience
pub use cache_manager::CacheManagerService;
pub use feed_merger::merge_segments;
pub use gex_calculator::GexCalculationService;
pub use market_data::{DataRequestEstimate, MarketDataService, ServiceError, ServiceResult};
pub use options_data::OptionsDataService;
pub use replay_engine::{ReplayEngine, ReplayEngineConfig, ReplayEvent, VolumeBucket};
