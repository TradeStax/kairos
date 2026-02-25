mod built_in;

use crate::core::metadata::{StrategyCategory, StrategyMetadata};
use crate::core::strategy::BacktestStrategy;
use std::collections::HashMap;

/// Lightweight descriptor for a registered strategy (no factory).
#[derive(Debug, Clone)]
pub struct StrategyInfo {
    pub id: String,
    pub name: String,
    pub description: String,
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

/// Registry of strategy factories.  Create instances by ID.
pub struct StrategyRegistry {
    factories: HashMap<String, Box<dyn Fn() -> Box<dyn BacktestStrategy> + Send + Sync>>,
    info: HashMap<String, StrategyInfo>,
}

impl StrategyRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self { factories: HashMap::new(), info: HashMap::new() }
    }

    /// Create a registry pre-populated with all built-in strategies.
    pub fn with_built_ins() -> Self {
        let mut r = Self::new();
        built_in::register_all(&mut r);
        r
    }

    /// Register a strategy factory.
    pub fn register<F>(&mut self, id: &str, info: StrategyInfo, factory: F)
    where
        F: Fn() -> Box<dyn BacktestStrategy> + Send + Sync + 'static,
    {
        self.factories.insert(id.to_string(), Box::new(factory));
        self.info.insert(id.to_string(), info);
    }

    /// Create a new instance of the strategy with the given ID.
    pub fn create(&self, id: &str) -> Option<Box<dyn BacktestStrategy>> {
        self.factories.get(id).map(|f| f())
    }

    /// List all registered strategies (sorted by name).
    pub fn list(&self) -> Vec<StrategyInfo> {
        let mut v: Vec<_> = self.info.values().cloned().collect();
        v.sort_by(|a, b| a.name.cmp(&b.name));
        v
    }

    /// Check whether a strategy with the given ID is registered.
    pub fn contains(&self, id: &str) -> bool {
        self.factories.contains_key(id)
    }

    pub fn len(&self) -> usize {
        self.factories.len()
    }

    pub fn is_empty(&self) -> bool {
        self.factories.is_empty()
    }
}

impl Default for StrategyRegistry {
    fn default() -> Self {
        Self::with_built_ins()
    }
}

/// Recreate from built-ins (factories are not Clone).
impl Clone for StrategyRegistry {
    fn clone(&self) -> Self {
        Self::with_built_ins()
    }
}
