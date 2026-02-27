//! Walk-forward optimization framework for strategy parameter tuning.
//!
//! Provides grid-based parameter search with rolling in-sample /
//! out-of-sample window splits to detect overfitting and estimate
//! realistic out-of-sample performance.

pub mod objective;
pub mod parameter_space;
pub mod result;
pub mod walk_forward;

pub use objective::ObjectiveFunction;
pub use parameter_space::{ParameterGrid, ParameterRange};
pub use result::{WalkForwardResult, WindowResult};
pub use walk_forward::{TimeWindow, WalkForwardConfig, WalkForwardOptimizer};
