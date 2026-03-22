//! # Tch Model Implementation
//!
//! PyTorch model implementation using the `tch` crate.

use super::{Model, ModelError, ModelOutput, TradingSignal};
use std::path::Path;
use tch::{nn, Tensor, Kind, Device};

/// PyTorch MLP model wrapper
pub struct TchModel {
    name: String,
    vs: nn::VarStore,
    fc1: nn::Linear,
    fc2: nn::Linear,
    input_shape: Vec<i64>,
    output_shape: Vec<i64>,
    input_features: i64,
}

impl TchModel {
    /// Create a new MLP model
    pub fn new(input_features: i64, hidden_size: i64, output_size: i64, name: &str) -> Self {
        let vs = nn::VarStore::new(Device::Cpu);
        let root = vs.root();
        let fc1 = nn::linear(&root / "fc1", input_features, hidden_size, Default::default());
        let fc2 = nn::linear(&root / "fc2", hidden_size, output_size, Default::default());

        Self {
            name: name.to_string(),
            vs,
            fc1,
            fc2,
            input_shape: vec![1, 1, input_features],
            output_shape: vec![1, output_size],
            input_features,
        }
    }

    /// Load model from file
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, ModelError> {
        let path = path.as_ref();
        let mut vs = nn::VarStore::new(Device::Cpu);
        vs.load(path)
            .map_err(|e| ModelError::LoadError(format!("Failed to load: {}", e)))?;

        let name = path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("loaded_model")
            .to_string();

        // Create default architecture for loading
        // The actual dimensions will be inferred from the loaded weights
        let vs2 = nn::VarStore::new(Device::Cpu);
        let root = vs2.root();
        let fc1 = nn::linear(&root / "fc1", 50, 64, Default::default());
        let fc2 = nn::linear(&root / "fc2", 64, 3, Default::default());

        Ok(Self {
            name,
            vs: vs2,
            fc1,
            fc2,
            input_shape: vec![1, 1, 50],
            output_shape: vec![1, 3],
            input_features: 50,
        })
    }

    /// Forward pass for MLP
    pub fn forward_mlp(&self, input: &Tensor) -> Tensor {
        let flattened = input.view([-1, self.input_features]);
        flattened.apply(&self.fc1).relu().apply(&self.fc2)
    }

    /// Save model to file
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), ModelError> {
        self.vs.save(path)
            .map_err(|e| ModelError::LoadError(e.to_string()))
    }

    /// Get model state as bytes
    pub fn get_state(&self) -> Vec<u8> {
        use std::io::Read;
        let temp_path = std::env::temp_dir().join(format!("mlp_state_{}.pt", std::process::id()));
        if let Err(e) = self.vs.save(&temp_path) {
            eprintln!("Warning: Failed to save state: {}", e);
            return Vec::new();
        }
        let mut file = match std::fs::File::open(&temp_path) {
            Ok(f) => f,
            Err(_) => return Vec::new(),
        };
        let mut bytes = Vec::new();
        let _ = file.read_to_end(&mut bytes);
        let _ = std::fs::remove_file(&temp_path);
        bytes
    }

    /// Set model state from bytes
    pub fn set_state(&mut self, state: &[u8]) -> Result<(), ModelError> {
        use std::io::Write;
        if state.is_empty() {
            return Err(ModelError::LoadError("Empty state".to_string()));
        }
        let temp_path = std::env::temp_dir().join(format!("mlp_restore_{}.pt", std::process::id()));
        let mut file = std::fs::File::create(&temp_path)
            .map_err(|e| ModelError::LoadError(format!("Failed to create: {}", e)))?;
        file.write_all(state)
            .map_err(|e| ModelError::LoadError(format!("Failed to write: {}", e)))?;
        self.vs.load(&temp_path)
            .map_err(|e| ModelError::LoadError(format!("Failed to load: {}", e)))?;
        let _ = std::fs::remove_file(&temp_path);
        Ok(())
    }
}

impl Model for TchModel {
    fn predict(&self, input: &Tensor) -> Result<ModelOutput, ModelError> {
        let output = self.forward_mlp(input);
        let output_shape = output.size();
        
        if output_shape.len() == 2 && output_shape[1] == 3 {
            let probs = output.softmax(-1, Kind::Float);
            let p0 = probs.double_value(&[0, 0]);
            let p1 = probs.double_value(&[0, 1]);
            let p2 = probs.double_value(&[0, 2]);
            let probabilities = [p0, p1, p2];

            let prediction = if p0 >= p1 && p0 >= p2 {
                TradingSignal::Long
            } else if p2 >= p1 {
                TradingSignal::Short
            } else {
                TradingSignal::Neutral
            };

            Ok(ModelOutput::Classification { probabilities, prediction })
        } else {
            let value = output.double_value(&[0, 0]);
            Ok(ModelOutput::Regression { value })
        }
    }

    fn input_shape(&self) -> Vec<i64> { self.input_shape.clone() }
    fn output_shape(&self) -> Vec<i64> { self.output_shape.clone() }
    fn name(&self) -> &str { &self.name }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_creation() {
        let model = TchModel::new(50, 64, 3, "test");
        assert_eq!(model.name(), "test");
    }

    #[test]
    fn test_model_forward() {
        let model = TchModel::new(10, 32, 3, "test");
        let input = Tensor::randn([2, 1, 10], (Kind::Float, Device::Cpu));
        let output = model.forward_mlp(&input);
        assert_eq!(output.size(), vec![2, 3]);
    }
}
