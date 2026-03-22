//! # Training Loop
//!
//! Training loop implementation for ML models using tch (PyTorch bindings).
//! Supports MLP and LSTM models.

use super::config::{ModelType, TrainingConfig};
use super::dataset::{BatchIterator, Dataset};
use super::{TrainingMetrics, TrainingResult};

/// Training progress callback
pub trait TrainingCallback: Send + Sync {
    fn on_epoch_end(&self, metrics: &TrainingMetrics) -> bool;
}

/// Default callback that logs progress
pub struct LoggingCallback;

impl TrainingCallback for LoggingCallback {
    fn on_epoch_end(&self, metrics: &TrainingMetrics) -> bool {
        log::info!(
            "Epoch {}: train_loss={:.4}, val_loss={:?}, train_acc={:.4?}",
            metrics.epoch, metrics.train_loss, metrics.val_loss, metrics.train_accuracy
        );
        true
    }
}

/// Result of training including the trained model
pub struct TrainResult {
    /// Training result metrics
    pub result: TrainingResult,
    /// The trained VarStore (contains model weights)
    pub var_store: tch::nn::VarStore,
}

/// Run training loop and return trained model
pub fn train<C: TrainingCallback>(
    config: &TrainingConfig,
    dataset: &Dataset,
    callback: &C,
) -> TrainResult {
    let num_features = dataset.num_features();
    let lookback = dataset.lookback();
    let num_classes = 3;
    let batch_size = config.batch_size;

    // Determine device
    let device_str = if let Some(gpu_id) = config.gpu_device {
        if tch::Cuda::is_available() {
            format!("GPU {}", gpu_id)
        } else {
            "CPU (GPU unavailable)".to_string()
        }
    } else if tch::Cuda::is_available() {
        "GPU 0 (auto)".to_string()
    } else {
        "CPU".to_string()
    };
    println!("Device: {}", device_str);

    let is_lstm = matches!(config.model_type, ModelType::LSTM | ModelType::BiLSTM);
    println!(
        "Training: features={}, lookback={}, classes={}, architecture={:?}",
        num_features, lookback, num_classes, config.model_type
    );

    // Split dataset
    let (train_data, val_data) = if config.validation_split > 0.0 {
        dataset.split(config.validation_split)
    } else {
        (dataset.clone(), Dataset::new(vec![], vec![], vec![]))
    };

    // Training with MLP model
    if !is_lstm {
        return train_mlp(config, dataset, &train_data, &val_data, num_features, lookback, callback);
    }
    
    // Training with LSTM model
    train_lstm(config, dataset, &train_data, &val_data, num_features, lookback, callback)
}

fn train_mlp<C: TrainingCallback>(
    config: &TrainingConfig,
    _dataset: &Dataset,
    train_data: &Dataset,
    val_data: &Dataset,
    num_features: usize,
    lookback: usize,
    callback: &C,
) -> TrainResult {
    use tch::{Kind, Device, Tensor};
    use tch::nn::OptimizerConfig;

    let input_size = (lookback * num_features) as i64;
    let num_classes = 3;
    let batch_size = config.batch_size;

    let vs = tch::nn::VarStore::new(Device::Cpu);
    let root = vs.root();
    let fc1 = tch::nn::linear(&root / "fc1", input_size, 64, Default::default());
    let fc2 = tch::nn::linear(&root / "fc2", 64, num_classes, Default::default());

    let mut metrics_history = Vec::new();
    let mut best_val_loss = f64::INFINITY;
    let mut patience_counter = 0;
    let learning_rate = config.learning_rate;

    // Create optimizer once before the training loop
    let mut opt = tch::nn::Adam::default().build(&vs, learning_rate).unwrap();

    for epoch in 1..=config.epochs {
        // Training
        let mut total_loss = 0.0;
        let mut correct = 0;
        let mut total = 0;

        for batch in BatchIterator::new(train_data, batch_size) {
            // Convert f64 features to f32 for PyTorch
            let features_f32: Vec<f32> = batch.features.iter().map(|&x| x as f32).collect();
            let input = Tensor::from_slice(&features_f32).reshape([batch.num_samples as i64, input_size]);
            let target = Tensor::from_slice(&batch.labels.iter().map(|&l| l as i64).collect::<Vec<_>>());
            let logits = input.apply(&fc1).relu().apply(&fc2);
            let loss = logits.cross_entropy_for_logits(&target);
            total_loss += loss.double_value(&[]) * batch.num_samples as f64;
            let predictions = logits.argmax(1, false);
            let correct_batch = predictions.iter::<i64>().unwrap().zip(target.iter::<i64>().unwrap()).filter(|(p, t)| p == t).count();
            correct += correct_batch;
            total += batch.num_samples;
            loss.backward();
            opt.step();
            opt.zero_grad();
        }

        let train_loss = total_loss / total.max(1) as f64;
        let train_acc = if total > 0 { Some(correct as f64 / total as f64) } else { None };
        
        // Validation
        let (val_loss, val_acc) = if !val_data.is_empty() {
            let mut total_loss = 0.0;
            let mut correct = 0;
            let mut total = 0;

            for batch in BatchIterator::new(val_data, batch_size) {
                // Convert f64 features to f32 for PyTorch
                let features_f32: Vec<f32> = batch.features.iter().map(|&x| x as f32).collect();
                let input = Tensor::from_slice(&features_f32).reshape([batch.num_samples as i64, input_size]);
                let target = Tensor::from_slice(&batch.labels.iter().map(|&l| l as i64).collect::<Vec<_>>());
                let logits = input.apply(&fc1).relu().apply(&fc2);
                let loss = logits.cross_entropy_for_logits(&target);
                total_loss += loss.double_value(&[]) * batch.num_samples as f64;
                let predictions = logits.argmax(1, false);
                let correct_batch = predictions.iter::<i64>().unwrap().zip(target.iter::<i64>().unwrap()).filter(|(p, t)| p == t).count();
                correct += correct_batch;
                total += batch.num_samples;
            }

            (Some(total_loss / total.max(1) as f64), Some(correct as f64 / total.max(1) as f64))
        } else {
            (None, None)
        };

        let metrics = TrainingMetrics {
            epoch, train_loss, val_loss, train_accuracy: train_acc, val_accuracy: val_acc,
        };
        metrics_history.push(metrics.clone());
        
        if !callback.on_epoch_end(&metrics) {
            break;
        }
        
        if config.early_stopping_patience > 0 {
            if let Some(vl) = val_loss {
                if vl < best_val_loss {
                    best_val_loss = vl;
                    patience_counter = 0;
                } else {
                    patience_counter += 1;
                    if patience_counter >= config.early_stopping_patience {
                        println!("Early stopping at epoch {}", epoch);
                        return TrainResult {
                            result: TrainingResult {
                                final_train_loss: train_loss, final_val_loss: val_loss,
                                epochs_trained: epoch, early_stopped: true, metrics: metrics_history,
                            },
                            var_store: vs,
                        };
                    }
                }
            }
        }
    }

    TrainResult {
        result: TrainingResult {
            final_train_loss: metrics_history.last().map(|m| m.train_loss).unwrap_or(0.0),
            final_val_loss: metrics_history.last().and_then(|m| m.val_loss),
            epochs_trained: config.epochs, early_stopped: false, metrics: metrics_history,
        },
        var_store: vs,
    }
}

fn train_lstm<C: TrainingCallback>(
    config: &TrainingConfig,
    _dataset: &Dataset,
    train_data: &Dataset,
    val_data: &Dataset,
    num_features: usize,
    lookback: usize,
    callback: &C,
) -> TrainResult {
    use tch::{Kind, Device, Tensor};
    use tch::nn::{RNN, OptimizerConfig};

    let num_features_i64 = num_features as i64;
    let lookback_i64 = lookback as i64;
    let hidden_size = config.lstm_config.hidden_size as i64;
    let dropout = config.lstm_config.dropout;
    let bidirectional = config.lstm_config.bidirectional;
    let num_classes = 3;
    let batch_size = config.batch_size;

    let vs = tch::nn::VarStore::new(Device::Cpu);
    let root = vs.root();
    let lstm_cfg = tch::nn::RNNConfig { dropout, num_layers: config.lstm_config.num_layers as i64, bidirectional, ..Default::default() };
    let lstm = tch::nn::lstm(&root / "lstm", num_features_i64, hidden_size, lstm_cfg);
    let fc_hidden = if bidirectional { hidden_size * 2 } else { hidden_size };
    let fc_out = tch::nn::linear(&root / "fc_out", fc_hidden, num_classes, Default::default());

    let mut metrics_history = Vec::new();
    let mut best_val_loss = f64::INFINITY;
    let mut patience_counter = 0;
    let learning_rate = config.learning_rate;

    // Create optimizer once before the training loop
    let mut opt = tch::nn::Adam::default().build(&vs, learning_rate).unwrap();

    for epoch in 1..=config.epochs {
        // Training
        let mut total_loss = 0.0;
        let mut correct = 0;
        let mut total = 0;

        for batch in BatchIterator::new(train_data, batch_size) {
            let features_f32: Vec<f32> = batch.features.iter().map(|&x| x as f32).collect();
            let input = Tensor::from_slice(&features_f32).reshape([batch.num_samples as i64, lookback_i64, num_features_i64]);
            let target = Tensor::from_slice(&batch.labels.iter().map(|&l| l as i64).collect::<Vec<_>>());
            let (output, _) = lstm.seq(&input);
            let seq_len = output.size()[1];
            let last = output.narrow(1, seq_len - 1, 1).squeeze();
            let logits = last.apply(&fc_out);
            let loss = logits.cross_entropy_for_logits(&target);
            total_loss += loss.double_value(&[]) * batch.num_samples as f64;
            let predictions = logits.argmax(1, false);
            let correct_batch = predictions.iter::<i64>().unwrap().zip(target.iter::<i64>().unwrap()).filter(|(p, t)| p == t).count();
            correct += correct_batch;
            total += batch.num_samples;
            loss.backward();
            opt.step();
            opt.zero_grad();
        }

        let train_loss = total_loss / total.max(1) as f64;
        let train_acc = if total > 0 { Some(correct as f64 / total as f64) } else { None };
        
        // Validation
        let (val_loss, val_acc) = if !val_data.is_empty() {
            let mut total_loss = 0.0;
            let mut correct = 0;
            let mut total = 0;

            for batch in BatchIterator::new(val_data, batch_size) {
                let features_f32: Vec<f32> = batch.features.iter().map(|&x| x as f32).collect();
                let input = Tensor::from_slice(&features_f32).reshape([batch.num_samples as i64, lookback_i64, num_features_i64]);
                let target = Tensor::from_slice(&batch.labels.iter().map(|&l| l as i64).collect::<Vec<_>>());
                let (output, _) = lstm.seq(&input);
                let seq_len = output.size()[1];
                let last = output.narrow(1, seq_len - 1, 1).squeeze();
                let logits = last.apply(&fc_out);
                let loss = logits.cross_entropy_for_logits(&target);
                total_loss += loss.double_value(&[]) * batch.num_samples as f64;
                let predictions = logits.argmax(1, false);
                let correct_batch = predictions.iter::<i64>().unwrap().zip(target.iter::<i64>().unwrap()).filter(|(p, t)| p == t).count();
                correct += correct_batch;
                total += batch.num_samples;
            }

            (Some(total_loss / total.max(1) as f64), Some(correct as f64 / total.max(1) as f64))
        } else {
            (None, None)
        };

        let metrics = TrainingMetrics {
            epoch, train_loss, val_loss, train_accuracy: train_acc, val_accuracy: val_acc,
        };
        metrics_history.push(metrics.clone());
        
        if !callback.on_epoch_end(&metrics) {
            break;
        }
        
        if config.early_stopping_patience > 0 {
            if let Some(vl) = val_loss {
                if vl < best_val_loss {
                    best_val_loss = vl;
                    patience_counter = 0;
                } else {
                    patience_counter += 1;
                    if patience_counter >= config.early_stopping_patience {
                        println!("Early stopping at epoch {}", epoch);
                        return TrainResult {
                            result: TrainingResult {
                                final_train_loss: train_loss, final_val_loss: val_loss,
                                epochs_trained: epoch, early_stopped: true, metrics: metrics_history,
                            },
                            var_store: vs,
                        };
                    }
                }
            }
        }
    }

    TrainResult {
        result: TrainingResult {
            final_train_loss: metrics_history.last().map(|m| m.train_loss).unwrap_or(0.0),
            final_val_loss: metrics_history.last().and_then(|m| m.val_loss),
            epochs_trained: config.epochs, early_stopped: false, metrics: metrics_history,
        },
        var_store: vs,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_training_config_defaults() {
        let config = TrainingConfig::default();
        assert_eq!(config.model_type, ModelType::LSTM);
        assert_eq!(config.learning_rate, 0.001);
    }
}
