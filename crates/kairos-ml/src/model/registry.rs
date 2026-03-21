//! # Model Registry
//!
//! Centralized model loading and management.

use super::{Model, ModelError};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Registry for managing and loading models
pub struct ModelRegistry {
    /// Cached loaded models
    cache: RwLock<HashMap<String, Arc<Box<dyn Model>>>>,
}

impl ModelRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
        }
    }

    /// Register and load a model with a given name
    pub fn register<M: Model + 'static>(
        &mut self,
        name: &str,
        factory: impl Fn() -> Result<M, ModelError> + 'static,
    ) {
        let model = factory().expect("Failed to create model");
        let arc_model: Arc<Box<dyn Model>> = Arc::new(Box::new(model));

        let mut cache = self.cache.write().unwrap();
        cache.insert(name.to_string(), arc_model);
    }

    /// Load a model by name from cache
    pub fn load(&self, name: &str) -> Result<Arc<Box<dyn Model>>, ModelError> {
        let cache = self.cache.read().unwrap();
        cache
            .get(name)
            .cloned()
            .ok_or_else(|| ModelError::ModelNotFound(name.to_string()))
    }

    /// List all registered model names
    pub fn list(&self) -> Vec<String> {
        let cache = self.cache.read().unwrap();
        cache.keys().cloned().collect()
    }

    /// Check if a model is registered
    pub fn contains(&self, name: &str) -> bool {
        let cache = self.cache.read().unwrap();
        cache.contains_key(name)
    }

    /// Clear the model cache
    pub fn clear_cache(&self) {
        let mut cache = self.cache.write().unwrap();
        cache.clear();
    }
}

impl Default for ModelRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::super::{ModelOutput, TradingSignal};
    use super::*;

    struct TestModel {
        name: String,
        input_shape: Vec<i64>,
        output_shape: Vec<i64>,
    }

    impl TestModel {
        fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
                input_shape: vec![1, 10, 5],
                output_shape: vec![1, 3],
            }
        }
    }

    impl Model for TestModel {
        fn predict(&self, _input: &crate::model::Tensor) -> Result<ModelOutput, ModelError> {
            Ok(ModelOutput::Classification {
                probabilities: [0.7, 0.2, 0.1],
                prediction: TradingSignal::Long,
            })
        }

        fn input_shape(&self) -> Vec<i64> {
            self.input_shape.clone()
        }

        fn output_shape(&self) -> Vec<i64> {
            self.output_shape.clone()
        }

        fn name(&self) -> &str {
            &self.name
        }
    }

    fn create_test_model() -> Result<TestModel, ModelError> {
        Ok(TestModel::new("test"))
    }

    #[test]
    fn test_registry_register_and_load() {
        let mut registry = ModelRegistry::new();
        registry.register("test", create_test_model);

        let model = registry.load("test").unwrap();
        assert_eq!(model.name(), "test");
    }

    #[test]
    fn test_registry_load_unknown_returns_error() {
        let registry = ModelRegistry::new();

        let result = registry.load("nonexistent");
        assert!(result.is_err());
        match result {
            Err(ModelError::ModelNotFound(name)) => {
                assert_eq!(name, "nonexistent");
            }
            _ => panic!("Expected ModelNotFound error"),
        }
    }

    #[test]
    fn test_registry_list_registered_models() {
        let mut registry = ModelRegistry::new();
        registry.register("model1", create_test_model);
        registry.register("model2", create_test_model);

        let list = registry.list();
        assert!(list.contains(&"model1".to_string()));
        assert!(list.contains(&"model2".to_string()));
    }

    #[test]
    fn test_registry_contains() {
        let mut registry = ModelRegistry::new();
        registry.register("test", create_test_model);

        assert!(registry.contains("test"));
        assert!(!registry.contains("other"));
    }
}
