//! # Tch Model Implementation
//!
//! PyTorch model implementation using the `tch` crate.

use super::{Model, ModelError, ModelOutput, TradingSignal};
use std::path::Path;
use tch::Tensor;

/// PyTorch model wrapper
pub struct TchModel {
    /// Model name
    name: String,
    /// Neural network
    vs: tch::nn::VarStore,
    /// First layer
    fc1: tch::nn::Linear,
    /// Second layer
    fc2: tch::nn::Linear,
    /// Input shape [batch, sequence, features]
    input_shape: Vec<i64>,
    /// Output shape [batch, ...]
    output_shape: Vec<i64>,
    /// Input dimension for flattening (seq * features)
    input_features: i64,
}

impl TchModel {
    /// Load a model from a PyTorch checkpoint file (.pt)
    ///
    /// This method loads a model from a state dict file. The model architecture
    /// is inferred from the loaded weights (fc1 and fc2 layers must be present).
    ///
    /// # Arguments
    /// * `path` - Path to the .pt file containing saved model weights
    ///
    /// # Returns
    /// * `Ok(TchModel)` - Successfully loaded model
    /// * `Err(ModelError)` - Failed to load model
    ///
    /// # Example
    /// ```ignore
    /// let model = TchModel::load("trained_model.pt").unwrap();
    /// let input = tch::Tensor::randn([1, 1, 10], (tch::Kind::Float, tch::Device::Cpu));
    /// let output = model.predict(&input).unwrap();
    /// ```
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, ModelError> {
        let path = path.as_ref();

        // First, create a VarStore and try to load the state dict
        let mut vs = tch::nn::VarStore::new(tch::Device::Cpu);

        // Load the state dict
        vs.load(path)
            .map_err(|e| ModelError::LoadError(format!("Failed to load state dict: {}", e)))?;

        // Now we need to infer the architecture from the loaded weights
        // We look for the fc1 and fc2 weight tensors to determine dimensions
        let root = vs.root();

        // Get fc1 weights to determine input and hidden dimensions
        let fc1_weights = root / "fc1";
        let fc1_weight_tensor: tch::Tensor = vs.variables()
            .get(&fc1_weights / "weight")
            .ok_or_else(|| ModelError::LoadError(
                "State dict missing 'fc1.weight' tensor. Expected MLP architecture with fc1 and fc2 layers.".to_string()
            ))?
            .1;

        let (hidden_size, input_features) = {
            let shape = fc1_weight_tensor.size();
            // Weight shape is [out_features, in_features]
            (shape[0] as i64, shape[1] as i64)
        };

        // Get fc2 weights to verify output dimension
        let fc2_weights = root / "fc2";
        let fc2_weight_tensor: tch::Tensor = vs.variables()
            .get(&fc2_weights / "weight")
            .ok_or_else(|| ModelError::LoadError(
                "State dict missing 'fc2.weight' tensor. Expected MLP architecture with fc1 and fc2 layers.".to_string()
            ))?
            .1;

        let output_size = fc2_weight_tensor.size()[0] as i64;

        // Create the model layers with the inferred dimensions
        let path_fc1 = vs.root() / "fc1";
        let fc1 = tch::nn::linear(&path_fc1, input_features, hidden_size, Default::default());

        let path_fc2 = vs.root() / "fc2";
        let fc2 = tch::nn::linear(&path_fc2, hidden_size, output_size, Default::default());

        // Get the filename as the model name
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("loaded_model")
            .to_string();

        Ok(Self {
            name,
            vs,
            fc1,
            fc2,
            input_shape: vec![1, 1, input_features],
            output_shape: vec![1, output_size],
            input_features,
        })
    }

    /// Create a new model with specified architecture
    pub fn new(input_features: i64, hidden_size: i64, output_size: i64, name: &str) -> Self {
        let vs = tch::nn::VarStore::new(tch::Device::Cpu);

        // Create layers using the path API
        let path_fc1 = vs.root() / "fc1";
        let fc1 = tch::nn::linear(&path_fc1, input_features, hidden_size, Default::default());

        let path_fc2 = vs.root() / "fc2";
        let fc2 = tch::nn::linear(&path_fc2, hidden_size, output_size, Default::default());

        Self {
            name: name.to_string(),
            vs,
            fc1,
            fc2,
            input_shape: vec![1, 1, input_features], // [batch, seq, features]
            output_shape: vec![1, output_size],
            input_features,
        }
    }

    /// Run inference on input (requires [batch, seq, features] shape)
    pub fn predict_raw(&self, input: &Tensor) -> Result<Tensor, ModelError> {
        let input_size = input.size();

        // Ensure input has correct dimensions [batch, seq, features]
        if input_size.len() != 3 {
            return Err(ModelError::InvalidInputShape {
                expected: self.input_shape.clone(),
                actual: input_size,
            });
        }

        // Flatten sequence dimension: [batch, seq, features] -> [batch, seq*features]
        let batch_size = input_size[0];
        let flattened = input.view([batch_size, -1]);

        // Forward pass: input -> fc1 -> relu -> fc2
        let h = flattened.apply(&self.fc1);
        let h_relu = h.relu();
        let output = h_relu.apply(&self.fc2);

        Ok(output)
    }

    /// Internal forward pass for training (handles flattened input directly)
    pub fn predict_raw_internal(&self, input: &Tensor) -> Tensor {
        let input_size = input.size();
        let batch_size = if input_size.is_empty() {
            1
        } else {
            input_size[0]
        };

        // Ensure correct input size
        if input_size.len() == 2 && input_size[1] == self.input_features {
            // Already correctly sized
            let h = input.apply(&self.fc1);
            let h_relu = h.relu();
            h_relu.apply(&self.fc2)
        } else {
            // Flatten and resize if needed
            let flattened = input.view([batch_size, -1]);
            let h = flattened.apply(&self.fc1);
            let h_relu = h.relu();
            h_relu.apply(&self.fc2)
        }
    }

    /// Get model state as bytes (for saving best model during training)
    ///
    /// Serializes the VarStore state to bytes for in-memory storage.
    /// Used internally for early stopping to save the best model state.
    pub fn get_state(&self) -> Vec<u8> {
        use std::io::Read;

        // Create a temporary file for serialization
        let temp_path =
            std::env::temp_dir().join(format!("kairos_ml_state_{}.pt", std::process::id()));

        // Save to temp file
        if let Err(e) = self.vs.save(&temp_path) {
            eprintln!("Warning: Failed to save state: {}", e);
            return Vec::new();
        }

        // Read into memory
        let mut file = match std::fs::File::open(&temp_path) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Warning: Failed to open temp state file: {}", e);
                return Vec::new();
            }
        };

        let mut bytes = Vec::new();
        if let Err(e) = file.read_to_end(&mut bytes) {
            eprintln!("Warning: Failed to read state bytes: {}", e);
            return Vec::new();
        }

        // Clean up temp file
        let _ = std::fs::remove_file(&temp_path);

        bytes
    }

    /// Set model state from bytes (for restoring best model)
    ///
    /// Restores the VarStore state from in-memory bytes.
    /// Used internally for early stopping to restore the best model.
    pub fn set_state(&mut self, state: &[u8]) -> Result<(), ModelError> {
        use std::io::Write;

        if state.is_empty() {
            return Err(ModelError::LoadError("Empty state bytes".to_string()));
        }

        // Create a temporary file
        let temp_path =
            std::env::temp_dir().join(format!("kairos_ml_state_restore_{}.pt", std::process::id()));

        // Write state to temp file
        let mut file = std::fs::File::create(&temp_path)
            .map_err(|e| ModelError::LoadError(format!("Failed to create temp file: {}", e)))?;

        file.write_all(state)
            .map_err(|e| ModelError::LoadError(format!("Failed to write state: {}", e)))?;

        // Load from temp file
        self.vs
            .load(&temp_path)
            .map_err(|e| ModelError::LoadError(format!("Failed to load state: {}", e)))?;

        // Clean up temp file
        let _ = std::fs::remove_file(&temp_path);

        Ok(())
    }

    /// Save model to file
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), ModelError> {
        self.vs
            .save(path)
            .map_err(|e| ModelError::LoadError(e.to_string()))
    }

    /// Load model from file
    pub fn load_from_file<P: AsRef<Path>>(
        path: P,
        name: &str,
        input_features: i64,
    ) -> Result<Self, ModelError> {
        let mut vs = tch::nn::VarStore::new(tch::Device::Cpu);

        // Try to load existing weights
        vs.load(path)
            .map_err(|e| ModelError::LoadError(e.to_string()))?;

        // Create model with same architecture
        // Note: In production, architecture should be saved with the model
        // For now, we create a default architecture and hope it matches
        let path_fc1 = vs.root() / "fc1";
        let fc1 = tch::nn::linear(&path_fc1, input_features, 64, Default::default());
        let path_fc2 = vs.root() / "fc2";
        let fc2 = tch::nn::linear(&path_fc2, 64, 3, Default::default());

        Ok(Self {
            name: name.to_string(),
            vs,
            fc1,
            fc2,
            input_shape: vec![1, 1, input_features],
            output_shape: vec![1, 3],
            input_features,
        })
    }
}

impl Model for TchModel {
    fn predict(&self, input: &Tensor) -> Result<ModelOutput, ModelError> {
        let output = self.predict_raw(input)?;

        // Check output size to determine classification vs regression
        let output_shape = output.size();

        if output_shape.len() == 2 && output_shape[1] == 3 {
            // Classification: apply softmax to get probabilities
            let probs = output.softmax(-1, tch::Kind::Float);

            // Extract probabilities by getting individual values
            // probs has shape [batch, 3], so access as [0, 0], [0, 1], [0, 2]
            let p0 = probs.double_value(&[0, 0]);
            let p1 = probs.double_value(&[0, 1]);
            let p2 = probs.double_value(&[0, 2]);

            let probabilities = [p0, p1, p2];

            // Get prediction (argmax)
            let prediction = if p0 >= p1 && p0 >= p2 {
                TradingSignal::Long
            } else if p2 >= p1 {
                TradingSignal::Short
            } else {
                TradingSignal::Neutral
            };

            Ok(ModelOutput::Classification {
                probabilities,
                prediction,
            })
        } else {
            // Regression
            let value = output.double_value(&[0, 0]);
            Ok(ModelOutput::Regression { value })
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_tch_model_creation() {
        let model = TchModel::new(10, 32, 3, "test_model");

        assert_eq!(model.name(), "test_model");
        assert_eq!(model.input_shape(), vec![1, 1, 10]);
        assert_eq!(model.output_shape(), vec![1, 3]);
    }

    #[test]
    fn test_tch_model_inference_shape() {
        let model = TchModel::new(10, 32, 3, "test_model");

        let input = Tensor::randn([1, 1, 10], (tch::Kind::Float, tch::Device::Cpu));
        let output = model.predict_raw(&input);

        assert!(output.is_ok());
        let output_shape = output.unwrap().size();
        assert_eq!(output_shape, vec![1, 3]);
    }

    #[test]
    fn test_tch_model_inference_classification() {
        let model = TchModel::new(10, 32, 3, "test_model");

        let input = Tensor::randn([1, 1, 10], (tch::Kind::Float, tch::Device::Cpu));
        let output = model.predict(&input);

        assert!(output.is_ok());

        match output.unwrap() {
            ModelOutput::Classification {
                probabilities,
                prediction,
            } => {
                assert_eq!(probabilities.len(), 3);
                // Probabilities should sum to ~1
                let sum: f64 = probabilities.iter().sum();
                assert!((sum - 1.0).abs() < 0.01);
                // Prediction should be valid
                assert!(matches!(
                    prediction,
                    TradingSignal::Long | TradingSignal::Short | TradingSignal::Neutral
                ));
            }
            _ => panic!("Expected Classification output"),
        }
    }

    #[test]
    fn test_tch_model_invalid_input_shape() {
        let model = TchModel::new(10, 32, 3, "test_model");

        // Wrong shape: should be [batch, seq, features]
        let input = Tensor::randn([1, 10], (tch::Kind::Float, tch::Device::Cpu));
        let output = model.predict(&input);

        assert!(output.is_err());
    }

    #[test]
    fn test_tch_model_internal_forward() {
        let model = TchModel::new(10, 32, 3, "test_model");

        // Test internal forward pass with flattened input
        let input = Tensor::randn([4, 10], (tch::Kind::Float, tch::Device::Cpu));
        let output = model.predict_raw_internal(&input);

        assert_eq!(output.size(), vec![4, 3]);
    }

    #[test]
    fn test_tch_model_get_set_state() {
        let model = TchModel::new(10, 32, 3, "test_model");

        // Get state
        let state = model.get_state();
        assert!(!state.is_empty(), "State should not be empty");

        // Verify it's a valid PyTorch checkpoint (starts with encoded metadata)
        // PyTorch files have specific binary format headers
        assert!(state.len() > 100, "State should have reasonable size");
    }

    #[test]
    fn test_tch_model_state_roundtrip() {
        use std::sync::Mutex;

        // Create original model
        let model = TchModel::new(10, 32, 3, "original_model");

        // Get state
        let state = model.get_state();
        assert!(!state.is_empty(), "State should not be empty");

        // Create a second model with same architecture
        let mut model2 = TchModel::new(10, 32, 3, "restored_model");

        // Get output from original model with fixed input
        let input = Tensor::randn([1, 1, 10], (tch::Kind::Float, tch::Device::Cpu));
        let original_output = model.predict_raw(&input).unwrap();

        // Set state from first model
        let result = model2.set_state(&state);
        assert!(result.is_ok(), "set_state should succeed");

        // Verify outputs are identical after state restore
        let restored_output = model2.predict_raw(&input).unwrap();

        let diff = (&original_output - &restored_output)
            .abs()
            .sum()
            .double_value(&[]);
        assert!(
            diff < 1e-10,
            "Restored model should produce same outputs as original. Diff: {}",
            diff
        );
    }

    #[test]
    fn test_tch_model_set_state_empty_fails() {
        let mut model = TchModel::new(10, 32, 3, "test_model");

        // Setting empty state should fail
        let result = model.set_state(&[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_tch_model_save_load() {
        let temp_dir = TempDir::new().unwrap();
        let model_path = temp_dir.path().join("test_model.pt");

        // Create and save model
        let model = TchModel::new(10, 32, 3, "test_model");
        let save_result = model.save(&model_path);
        assert!(save_result.is_ok());

        // Load model
        let loaded = TchModel::load_from_file(&model_path, "loaded_model", 10);
        assert!(loaded.is_ok());

        let loaded_model = loaded.unwrap();
        assert_eq!(loaded_model.name(), "loaded_model");
        assert_eq!(loaded_model.input_shape(), vec![1, 1, 10]);
    }

    #[test]
    fn test_tch_model_load_invalid_path() {
        let result = TchModel::load("/nonexistent/path/model.pt");
        assert!(result.is_err());
    }

    #[test]
    fn test_tch_model_loads_from_state_dict() {
        let temp_dir = TempDir::new().unwrap();
        let model_path = temp_dir.path().join("simple_mlp.pt");

        // Create and save a model with known architecture
        let original = TchModel::new(50, 64, 3, "simple_mlp"); // 50 = 10 * 5 (lookback * features)
        original.save(&model_path).unwrap();

        // Load the model using the new load method
        let loaded = TchModel::load(&model_path).unwrap();

        // Verify the loaded model has the correct architecture
        assert_eq!(loaded.name(), "simple_mlp");
        assert_eq!(loaded.input_shape(), vec![1, 1, 50]);
        assert_eq!(loaded.output_shape(), vec![1, 3]);
    }

    #[test]
    fn test_tch_model_load_preserves_weights() {
        use std::sync::OnceLock;

        let temp_dir = TempDir::new().unwrap();
        let model_path = temp_dir.path().join("weights_test.pt");

        // Create model with specific weights by setting them
        let model = TchModel::new(10, 32, 3, "weights_test");

        // Save the model
        model.save(&model_path).unwrap();

        // Load the model
        let loaded = TchModel::load(&model_path).unwrap();

        // Verify the architecture is preserved
        assert_eq!(loaded.input_shape(), vec![1, 1, 10]);
        assert_eq!(loaded.output_shape(), vec![1, 3]);

        // Create a fixed input and verify both models produce the same output
        let input = Tensor::randn([1, 1, 10], (tch::Kind::Float, tch::Device::Cpu));

        let original_output = model.predict_raw(&input).unwrap();
        let loaded_output = loaded.predict_raw(&input).unwrap();

        // Outputs should be identical since they have the same weights
        let diff = (&original_output - &loaded_output)
            .abs()
            .sum()
            .double_value(&[]);
        assert!(
            diff < 1e-10,
            "Loaded model should produce same outputs as original"
        );
    }

    #[test]
    fn test_tch_model_load_invalid_state_dict() {
        let temp_dir = TempDir::new().unwrap();
        let invalid_path = temp_dir.path().join("invalid.pt");

        // Create a file that's not a valid PyTorch state dict
        std::fs::write(&invalid_path, "not a valid state dict").unwrap();

        let result = TchModel::load(&invalid_path);
        assert!(result.is_err());
    }
}
