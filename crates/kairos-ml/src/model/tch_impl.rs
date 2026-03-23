//! # Tch Model Implementation
//!
//! PyTorch model implementation using the `tch` crate.
//! Supports MLP and LSTM models with proper save/load.

use super::{Model, ModelError, ModelOutput, TradingSignal};
use serde::{Deserialize, Serialize};
use tch::{nn, Tensor, Kind, Device};
use tch::nn::RNN;

/// Model architecture metadata (stored alongside weights)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMetadata {
    /// Model type: "mlp" or "lstm"
    pub model_type: String,
    /// Number of input features
    pub num_features: i64,
    /// Sequence length (lookback) - only for LSTM
    pub lookback: i64,
    /// Hidden layer size
    pub hidden_size: i64,
    /// Number of output classes
    pub num_classes: i64,
    /// Number of LSTM layers
    pub num_layers: i64,
    /// Dropout probability
    pub dropout: f64,
    /// Whether LSTM is bidirectional
    pub bidirectional: bool,
    /// Model name
    pub name: String,
}

impl Default for ModelMetadata {
    fn default() -> Self {
        Self {
            model_type: "lstm".to_string(),
            num_features: 12,
            lookback: 20,
            hidden_size: 64,
            num_classes: 3,
            num_layers: 2,
            dropout: 0.2,
            bidirectional: false,
            name: "unnamed_model".to_string(),
        }
    }
}

/// Internal model variants
enum TchModelVariant {
    /// MLP model with flattened input
    Mlp {
        fc1: nn::Linear,
        fc2: nn::Linear,
        input_features: i64,
    },
    /// LSTM model with sequence input
    Lstm {
        lstm: nn::LSTM,
        fc_out: nn::Linear,
        hidden_size: i64,
        bidirectional: bool,
    },
}

/// PyTorch model wrapper supporting both MLP and LSTM
pub struct TchModel {
    name: String,
    vs: nn::VarStore,
    variant: TchModelVariant,
    input_shape: Vec<i64>,
    output_shape: Vec<i64>,
    metadata: ModelMetadata,
}

impl TchModel {
    /// Create a new MLP model
    pub fn new(input_features: i64, hidden_size: i64, output_size: i64, name: &str) -> Self {
        let vs = nn::VarStore::new(Device::Cpu);
        let root = vs.root();
        let fc1 = nn::linear(&root / "fc1", input_features, hidden_size, Default::default());
        let fc2 = nn::linear(&root / "fc2", hidden_size, output_size, Default::default());

        let metadata = ModelMetadata {
            model_type: "mlp".to_string(),
            num_features: input_features,
            lookback: 1,
            hidden_size,
            num_classes: output_size,
            num_layers: 1,
            dropout: 0.0,
            bidirectional: false,
            name: name.to_string(),
        };

        Self {
            name: name.to_string(),
            vs,
            variant: TchModelVariant::Mlp {
                fc1,
                fc2,
                input_features,
            },
            input_shape: vec![1, 1, input_features],
            output_shape: vec![1, output_size],
            metadata,
        }
    }

    /// Create a new LSTM model
    pub fn new_lstm(
        num_features: i64,
        lookback: i64,
        hidden_size: i64,
        num_layers: i64,
        dropout: f64,
        bidirectional: bool,
        output_size: i64,
        name: &str,
    ) -> Self {
        let vs = nn::VarStore::new(Device::Cpu);
        let root = vs.root();

        let lstm_cfg = tch::nn::RNNConfig {
            dropout,
            num_layers,
            bidirectional,
            ..Default::default()
        };

        let lstm = tch::nn::lstm(&root / "lstm", num_features, hidden_size, lstm_cfg);
        let fc_hidden = if bidirectional { hidden_size * 2 } else { hidden_size };
        let fc_out = nn::linear(&root / "fc_out", fc_hidden, output_size, Default::default());

        let metadata = ModelMetadata {
            model_type: "lstm".to_string(),
            num_features,
            lookback,
            hidden_size,
            num_classes: output_size,
            num_layers,
            dropout,
            bidirectional,
            name: name.to_string(),
        };

        Self {
            name: name.to_string(),
            vs,
            variant: TchModelVariant::Lstm {
                lstm,
                fc_out,
                hidden_size,
                bidirectional,
            },
            input_shape: vec![1, lookback, num_features],
            output_shape: vec![1, output_size],
            metadata,
        }
    }

    /// Get model metadata
    pub fn metadata(&self) -> &ModelMetadata {
        &self.metadata
    }

    /// Load model from file with metadata
    pub fn load(path: &std::path::Path) -> Result<Self, ModelError> {
        // Try to load metadata first
        let json_path = path.with_extension("json");
        let metadata = if json_path.exists() {
            let json_content = std::fs::read_to_string(&json_path)
                .map_err(|e| ModelError::LoadError(format!("Failed to read metadata: {}", e)))?;
            serde_json::from_str(&json_content)
                .map_err(|e| ModelError::LoadError(format!("Failed to parse metadata: {}", e)))?
        } else {
            // No metadata file - try to infer from weights or use defaults
            ModelMetadata::default()
        };

        let name = metadata.name.clone();

        // Create model based on metadata
        let mut model = if metadata.model_type == "lstm" {
            Self::new_lstm(
                metadata.num_features,
                metadata.lookback,
                metadata.hidden_size,
                metadata.num_layers,
                metadata.dropout,
                metadata.bidirectional,
                metadata.num_classes,
                &name,
            )
        } else {
            Self::new(metadata.num_features, metadata.hidden_size, metadata.num_classes, &name)
        };

        // Try to load weights from .safetensors file
        let safetensors_path = path.with_extension("safetensors");
        let original_path = path;
        
        if safetensors_path.exists() {
            match model.vs.load(&safetensors_path) {
                Ok(_) => {
                    println!("Loaded model weights from {}", safetensors_path.display());
                }
                Err(e) => {
                    eprintln!("Warning: Could not load model weights: {}", e);
                    eprintln!("  Model architecture loaded but weights are random.");
                }
            }
        } else if original_path.exists() {
            match model.vs.load(original_path) {
                Ok(_) => {
                    println!("Loaded model weights from {}", original_path.display());
                }
                Err(e) => {
                    eprintln!("Warning: Could not load model weights: {}", e);
                    eprintln!("  Model architecture loaded but weights are random.");
                }
            }
        }

        Ok(model)
    }

    /// Forward pass for MLP
    fn forward_mlp(&self, input: &Tensor) -> Tensor {
        let TchModelVariant::Mlp { fc1, fc2, input_features } = &self.variant else {
            panic!("Called forward_mlp on non-MLP model");
        };
        let flattened = input.view([-1, *input_features]);
        flattened.apply(fc1).relu().apply(fc2)
    }

    /// Forward pass for LSTM
    fn forward_lstm(&self, input: &Tensor) -> Tensor {
        let TchModelVariant::Lstm { lstm, fc_out, .. } = &self.variant else {
            panic!("Called forward_lstm on non-LSTM model");
        };
        // input shape: [batch, seq, features]
        let (output, _) = lstm.seq(input);
        // Get last timestep
        let seq_len = output.size()[1];
        let last = output.narrow(1, seq_len - 1, 1);
        // Use squeeze_dims to only remove the sequence dimension (dim 1), keep batch dimension
        let last = last.squeeze_dims(&[1]);
        // Project to classes
        last.apply(fc_out)
    }

    /// Save model weights to file using PyTorch serialization
    pub fn save(&self, path: &std::path::Path) -> Result<(), ModelError> {
        // Determine extension - use .safetensors for better compatibility
        let save_path = if path.extension().and_then(|e| e.to_str()) == Some("safetensors") {
            path.to_path_buf()
        } else {
            path.with_extension("safetensors")
        };
        
        self.vs.save(&save_path)
            .map_err(|e| ModelError::LoadError(format!("Failed to save model: {}", e)))
    }

    /// Save model with metadata (creates .safetensors file + .json file)
    pub fn save_with_metadata(&self, path: &std::path::Path) -> Result<(), ModelError> {
        // Save weights as .safetensors file
        let weights_path = if path.extension().and_then(|e| e.to_str()) == Some("safetensors") {
            path.to_path_buf()
        } else {
            path.with_extension("safetensors")
        };
        
        self.vs.save(&weights_path)
            .map_err(|e| ModelError::LoadError(format!("Failed to save weights: {}", e)))?;

        // Save metadata as .json file with same base name
        let json_path = weights_path.with_extension("json");
        let json = serde_json::to_string_pretty(&self.metadata)
            .map_err(|e| ModelError::LoadError(format!("Failed to serialize metadata: {}", e)))?;
        std::fs::write(&json_path, json)
            .map_err(|e| ModelError::LoadError(format!("Failed to write metadata: {}", e)))?;

        Ok(())
    }
    
    /// Load model weights from file
    pub fn load_weights(&mut self, path: &std::path::Path) -> Result<(), ModelError> {
        // Try with safetensors first, then fall back to original path
        let safetensors_path = path.with_extension("safetensors");
        
        // Try loading
        if safetensors_path.exists() {
            self.vs.load(&safetensors_path)
                .map_err(|e| ModelError::LoadError(format!("Failed to load weights: {}", e)))
        } else if path.exists() {
            self.vs.load(path)
                .map_err(|e| ModelError::LoadError(format!("Failed to load weights: {}", e)))
        } else {
            Err(ModelError::LoadError(format!("File not found: {:?}", path)))
        }
    }
    
    /// Get reference to the VarStore for merging
    pub fn var_store(&self) -> &nn::VarStore {
        &self.vs
    }
}

impl Model for TchModel {
    fn predict(&self, input: &Tensor) -> Result<ModelOutput, ModelError> {
        // Use correct forward method based on model type
        let output = match &self.variant {
            TchModelVariant::Mlp { .. } => self.forward_mlp(input),
            TchModelVariant::Lstm { .. } => self.forward_lstm(input),
        };

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

    #[test]
    fn test_mlp_model_creation() {
        let model = TchModel::new(50, 64, 3, "test_mlp");
        assert_eq!(model.name(), "test_mlp");
        assert_eq!(model.metadata().model_type, "mlp");
    }

    #[test]
    fn test_lstm_model_creation() {
        let model = TchModel::new_lstm(12, 20, 64, 2, 0.2, false, 3, "test_lstm");
        assert_eq!(model.name(), "test_lstm");
        assert_eq!(model.metadata().model_type, "lstm");
    }

    #[test]
    fn test_mlp_forward() {
        let model = TchModel::new(10, 32, 3, "test");
        let input = Tensor::randn([2, 1, 10], (Kind::Float, Device::Cpu));
        let output = model.forward_mlp(&input);
        assert_eq!(output.size(), vec![2, 3]);
    }

    #[test]
    fn test_lstm_forward() {
        let model = TchModel::new_lstm(12, 20, 64, 2, 0.2, false, 3, "test_lstm");
        let input = Tensor::randn([2, 20, 12], (Kind::Float, Device::Cpu));
        let output = model.forward_lstm(&input);
        assert_eq!(output.size(), vec![2, 3]);
    }
}
