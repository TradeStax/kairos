//! Statistical analysis tools for evaluating backtest results.
//!
//! Provides hypothesis testing, bootstrap confidence intervals, and
//! Monte Carlo simulation to assess the statistical significance and
//! robustness of a trading strategy's performance.

pub mod monte_carlo;
pub mod statistics;

pub use monte_carlo::{MonteCarloResult, MonteCarloSimulator, Percentiles};
pub use statistics::{bootstrap_confidence_interval, t_test_mean_returns};
