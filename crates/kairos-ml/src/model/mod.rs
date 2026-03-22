//! # Model Module
//!
//! This module provides model loading, inference, and registry functionality.

pub mod output;
pub mod registry;

#[cfg(feature = "tch")]
pub mod tch_impl;

pub use output::{ModelOutput, TradingSignal};
pub use registry::ModelRegistry;

/// Model trait for abstracting over different model implementations
pub trait Model: std::any::Any {
    /// Run inference on input tensor
    fn predict(&self, input: &Tensor) -> Result<ModelOutput, ModelError>;

    /// Get the expected input shape [batch, sequence, features]
    fn input_shape(&self) -> Vec<i64>;

    /// Get the output shape [batch, ...]
    fn output_shape(&self) -> Vec<i64>;

    /// Get model name/identifier
    fn name(&self) -> &str;
    
    /// Get as Any for downcasting
    fn as_any(&self) -> &dyn std::any::Any where Self: Sized {
        self
    }
}

// Tensor type - re-exported when tch feature is enabled, stubbed otherwise
#[cfg(feature = "tch")]
use tch::Tensor;

#[cfg(not(feature = "tch"))]
#[derive(Debug, Clone, Copy)]
/// Placeholder Tensor type when tch feature is disabled
pub struct Tensor;

#[cfg(not(feature = "tch"))]
impl Tensor {
    /// Placeholder method - not functional without tch
    pub fn new() -> Self {
        Self
    }
}

/// Model error types
#[derive(Debug, thiserror::Error)]
pub enum ModelError {
    #[error("Failed to load model: {0}")]
    LoadError(String),

    #[error("Inference error: {0}")]
    InferenceError(String),

    #[error("Invalid input shape: expected {expected:?}, got {actual:?}")]
    InvalidInputShape {
        expected: Vec<i64>,
        actual: Vec<i64>,
    },

    #[error("Model not found: {0}")]
    ModelNotFound(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),
}
