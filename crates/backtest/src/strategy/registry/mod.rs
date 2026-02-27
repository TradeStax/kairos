//! Strategy registry and factory system.
//!
//! [`StrategyRegistry`] maps strategy IDs to factory closures,
//! allowing the engine and UI to discover, inspect, and instantiate
//! strategies by name.

mod built_in;

use crate::strategy::Strategy;
use crate::strategy::metadata::{StrategyCategory, StrategyMetadata};
use std::collections::HashMap;

/// Lightweight descriptor for a registered strategy.
///
/// Contains display information without the factory closure,
/// suitable for listing available strategies in the UI.
#[derive(Debug, Clone)]
pub struct StrategyInfo {
    /// Unique strategy identifier.
    pub id: String,
    /// Human-readable display name.
    pub name: String,
    /// Brief description of the strategy's approach.
    pub description: String,
    /// High-level category for grouping.
    pub category: StrategyCategory,
}

impl From<&StrategyMetadata> for StrategyInfo {
    fn from(m: &StrategyMetadata) -> Self {
        Self {
            id: m.id.clone(),
            name: m.name.clone(),
            description: m.description.clone(),
            category: m.category,
        }
    }
}

/// Factory registry for creating strategy instances by ID.
///
/// Strategies register themselves via [`register`](StrategyRegistry::register)
/// with a factory closure. The engine calls
/// [`create`](StrategyRegistry::create) to instantiate a strategy
/// for a backtest run.
///
/// Use [`with_built_ins`](StrategyRegistry::with_built_ins) for a
/// registry pre-populated with all built-in strategies.
pub struct StrategyRegistry {
    factories: HashMap<String, Box<dyn Fn() -> Box<dyn Strategy> + Send + Sync>>,
    info: HashMap<String, StrategyInfo>,
}

impl StrategyRegistry {
    /// Creates an empty registry with no strategies.
    #[must_use]
    pub fn new() -> Self {
        Self {
            factories: HashMap::new(),
            info: HashMap::new(),
        }
    }

    /// Creates a registry pre-populated with all built-in
    /// strategies.
    #[must_use]
    pub fn with_built_ins() -> Self {
        let mut r = Self::new();
        built_in::register_all(&mut r);
        r
    }

    /// Registers a strategy factory under the given ID.
    ///
    /// If a strategy with the same ID already exists, it is
    /// replaced.
    pub fn register<F>(&mut self, id: &str, info: StrategyInfo, factory: F)
    where
        F: Fn() -> Box<dyn Strategy> + Send + Sync + 'static,
    {
        self.factories.insert(id.to_string(), Box::new(factory));
        self.info.insert(id.to_string(), info);
    }

    /// Creates a new instance of the strategy with the given ID.
    ///
    /// Returns `None` if no strategy with that ID is registered.
    #[must_use]
    pub fn create(&self, id: &str) -> Option<Box<dyn Strategy>> {
        self.factories.get(id).map(|f| f())
    }

    /// Lists all registered strategies, sorted by name.
    #[must_use]
    pub fn list(&self) -> Vec<StrategyInfo> {
        let mut v: Vec<_> = self.info.values().cloned().collect();
        v.sort_by(|a, b| a.name.cmp(&b.name));
        v
    }

    /// Returns `true` if a strategy with the given ID is registered.
    #[must_use]
    pub fn contains(&self, id: &str) -> bool {
        self.factories.contains_key(id)
    }

    /// Returns the number of registered strategies.
    #[must_use]
    pub fn len(&self) -> usize {
        self.factories.len()
    }

    /// Returns `true` if no strategies are registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.factories.is_empty()
    }
}

impl Default for StrategyRegistry {
    fn default() -> Self {
        Self::with_built_ins()
    }
}

/// Recreates from built-ins (factory closures are not `Clone`).
impl Clone for StrategyRegistry {
    fn clone(&self) -> Self {
        Self::with_built_ins()
    }
}
