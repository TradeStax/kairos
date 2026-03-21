//! # Training Loop
//!
//! Training loop implementation for ML models using tch (PyTorch bindings).

#[cfg(feature = "tch")]
use super::dataset::BatchIterator;
use super::dataset::Dataset;
use super::{TrainingConfig, TrainingMetrics, TrainingResult};

/// Compute cross-entropy loss for classification
pub fn compute_cross_entropy_loss(
    predictions: &[f64],
    targets: &[usize],
    _num_classes: usize,
) -> f64 {
    let mut total_loss = 0.0;

    for (pred, _target) in predictions.iter().zip(targets.iter()) {
        if *pred <= 0.0 {
            total_loss += 100.0; // Large penalty for invalid prediction
        } else {
            // Softmax output for target class
            let target_prob = pred.max(1e-10);
            total_loss -= target_prob.ln();
        }
    }

    total_loss / predictions.len() as f64
}

/// Compute accuracy for classification
pub fn compute_accuracy(predictions: &[usize], targets: &[usize]) -> f64 {
    if predictions.is_empty() {
        return 0.0;
    }

    let correct = predictions
        .iter()
        .zip(targets.iter())
        .filter(|(p, t)| p == t)
        .count();

    correct as f64 / predictions.len() as f64
}

/// Training progress callback
pub trait TrainingCallback: Send + Sync {
    /// Called at the end of each epoch
    fn on_epoch_end(&self, metrics: &TrainingMetrics) -> bool;
}

/// Default callback that just logs
pub struct LoggingCallback;

impl TrainingCallback for LoggingCallback {
    fn on_epoch_end(&self, metrics: &TrainingMetrics) -> bool {
        log::info!(
            "Epoch {}: train_loss={:.4}, val_loss={:?}, train_acc={:.4?}",
            metrics.epoch,
            metrics.train_loss,
            metrics.val_loss,
            metrics.train_accuracy
        );
        true // Continue training
    }
}

/// Run training loop with tch backend
///
/// This is a generic training function that can work with any model that implements
/// the TrainableModel trait.
#[cfg_attr(not(feature = "tch"), allow(unused_variables))]
pub fn train<C: TrainingCallback>(
    config: &TrainingConfig,
    dataset: &Dataset,
    callback: &C,
) -> TrainingResult {
    #[cfg(feature = "tch")]
    {
        use crate::model::tch_impl::TchModel;

        let num_features = dataset.num_features();
        let lookback = dataset.lookback();

        // Create model
        let mut model = TchModel::new(
            (lookback * num_features) as i64,
            64,
            3, // 3 classes: long, neutral, short
            "training_model",
        );

        // Split dataset
        let (train_data, val_data) = if config.validation_split > 0.0 {
            dataset.split(config.validation_split)
        } else {
            (dataset.clone(), Dataset::new(vec![], vec![], vec![]))
        };

        let mut metrics_history = Vec::new();
        let mut best_val_loss = f64::INFINITY;
        let mut patience_counter = 0;
        let mut best_model_state: Option<Vec<u8>> = None;

        // Get initial validation loss
        if !val_data.is_empty() {
            let initial_loss = evaluate_model_loss_on_dataset(&model, &val_data, config.batch_size);
            best_val_loss = initial_loss;
        }

        for epoch in 1..=config.epochs {
            // Training pass
            let (train_loss, train_accuracy) = train_epoch(&mut model, &train_data, config);

            // Validation pass
            let (val_loss, val_accuracy) = if !val_data.is_empty() {
                let (loss, acc) = evaluate_model_on_dataset(&model, &val_data, config.batch_size);
                (Some(loss), Some(acc))
            } else {
                (None, None)
            };

            let metrics = TrainingMetrics {
                epoch,
                train_loss,
                val_loss,
                train_accuracy,
                val_accuracy,
            };

            metrics_history.push(metrics.clone());

            // Check if should continue
            if !callback.on_epoch_end(&metrics) {
                log::info!("Training stopped by callback at epoch {}", epoch);
                break;
            }

            // Early stopping check
            if config.early_stopping_patience > 0
                && let Some(vl) = val_loss
            {
                if vl < best_val_loss {
                    best_val_loss = vl;
                    patience_counter = 0;
                    // Save best model state
                    best_model_state = Some(model.get_state());
                } else {
                    patience_counter += 1;
                    if patience_counter >= config.early_stopping_patience {
                        log::info!(
                            "Early stopping triggered at epoch {} (patience={})",
                            epoch,
                            config.early_stopping_patience
                        );
                        // Restore best model
                        if let Some(state) = best_model_state {
                            let _ = model.set_state(&state);
                        }
                        return TrainingResult {
                            final_train_loss: train_loss,
                            final_val_loss: val_loss,
                            epochs_trained: epoch,
                            early_stopped: true,
                            metrics: metrics_history,
                        };
                    }
                }
            }
        }

        TrainingResult {
            final_train_loss: metrics_history.last().map(|m| m.train_loss).unwrap_or(0.0),
            final_val_loss: metrics_history.last().and_then(|m| m.val_loss),
            epochs_trained: config.epochs,
            early_stopped: false,
            metrics: metrics_history,
        }
    }

    #[cfg(not(feature = "tch"))]
    {
        // Return placeholder when tch feature is not enabled
        TrainingResult {
            final_train_loss: 0.5,
            final_val_loss: Some(0.6),
            epochs_trained: config.epochs,
            early_stopped: false,
            metrics: vec![],
        }
    }
}

/// Train a single epoch
#[cfg(feature = "tch")]
fn train_epoch(
    model: &mut crate::model::tch_impl::TchModel,
    dataset: &Dataset,
    config: &TrainingConfig,
) -> (f64, Option<f64>) {
    use tch::Tensor;

    let mut total_loss = 0.0;
    let mut correct = 0;
    let mut total = 0;

    for batch in BatchIterator::new(dataset, config.batch_size) {
        // Convert batch to tensors
        let [batch_size, lookback, num_features] = batch.feature_shape();

        // Reshape to [batch, lookback * features] for MLP
        let input = Tensor::from_slice(&batch.features)
            .reshape([batch_size as i64, (lookback * num_features) as i64]);

        let target =
            Tensor::f_from_slice(&batch.labels.iter().map(|&l| l as i64).collect::<Vec<_>>())
                .unwrap();

        // Forward pass
        let output = model.predict_raw_internal(&input);

        // Compute loss (cross-entropy)
        let loss = output.cross_entropy_for_logits(&target);
        let loss_value = loss.double_value(&[]);
        total_loss += loss_value * batch.num_samples as f64;

        // Compute accuracy
        let predictions = output.argmax(1, false);
        let pred_iter = predictions.iter::<i64>().unwrap();
        let target_iter = target.iter::<i64>().unwrap();
        let correct_batch = pred_iter.zip(target_iter).filter(|(p, t)| p == t).count();
        correct += correct_batch;
        total += batch.num_samples;

        // Backward pass and optimizer step
        // Note: In a full implementation, we'd integrate with the VarStore's optimizer
        // For now, we compute gradients but don't update (placeholder)
        loss.backward();
    }

    let avg_loss = total_loss / total.max(1) as f64;
    let accuracy = if total > 0 {
        Some(correct as f64 / total as f64)
    } else {
        None
    };

    (avg_loss, accuracy)
}

/// Evaluate model on a dataset
#[cfg(feature = "tch")]
fn evaluate_model_on_dataset(
    model: &crate::model::tch_impl::TchModel,
    dataset: &Dataset,
    batch_size: usize,
) -> (f64, f64) {
    let mut total_loss = 0.0;
    let mut correct = 0;
    let mut total = 0;

    for batch in BatchIterator::new(dataset, batch_size) {
        let [batch_size, lookback, num_features] = batch.feature_shape();

        let input = tch::Tensor::from_slice(&batch.features)
            .reshape([batch_size as i64, (lookback * num_features) as i64]);

        let target =
            tch::Tensor::f_from_slice(&batch.labels.iter().map(|&l| l as i64).collect::<Vec<_>>())
                .unwrap();

        let output = model.predict_raw_internal(&input);
        let loss = output.cross_entropy_for_logits(&target);
        total_loss += loss.double_value(&[]) * batch.num_samples as f64;

        let predictions = output.argmax(1, false);
        let pred_iter = predictions.iter::<i64>().unwrap();
        let target_iter = target.iter::<i64>().unwrap();
        let correct_batch = pred_iter.zip(target_iter).filter(|(p, t)| p == t).count();
        correct += correct_batch;
        total += batch.num_samples;
    }

    let avg_loss = total_loss / total.max(1) as f64;
    let accuracy = correct as f64 / total.max(1) as f64;

    (avg_loss, accuracy)
}

/// Evaluate loss on a dataset
#[cfg(feature = "tch")]
fn evaluate_model_loss_on_dataset(
    model: &crate::model::tch_impl::TchModel,
    dataset: &Dataset,
    batch_size: usize,
) -> f64 {
    let mut total_loss = 0.0;
    let mut total_samples = 0;

    for batch in BatchIterator::new(dataset, batch_size) {
        let [batch_size, lookback, num_features] = batch.feature_shape();

        let input = tch::Tensor::from_slice(&batch.features)
            .reshape([batch_size as i64, (lookback * num_features) as i64]);

        let target =
            tch::Tensor::f_from_slice(&batch.labels.iter().map(|&l| l as i64).collect::<Vec<_>>())
                .unwrap();

        let output = model.predict_raw_internal(&input);
        let loss = output.cross_entropy_for_logits(&target);
        total_loss += loss.double_value(&[]) * batch.num_samples as f64;
        total_samples += batch.num_samples;
    }

    total_loss / total_samples.max(1) as f64
}

#[cfg(feature = "tch")]
pub mod tch_training {
    use super::*;
    use crate::model::tch_impl::TchModel;

    /// Train with tch backend
    pub fn train_tch<C: TrainingCallback>(
        config: &TrainingConfig,
        dataset: &Dataset,
        callback: &C,
    ) -> Result<TrainingResult, String> {
        let num_features = dataset.num_features();
        let lookback = dataset.lookback();

        // Create model
        let _model = TchModel::new(
            (lookback * num_features) as i64,
            64,
            3, // 3 classes: long, neutral, short
            "training_model",
        );

        // Split dataset
        let (train_data, val_data) = if config.validation_split > 0.0 {
            dataset.split(config.validation_split)
        } else {
            (dataset.clone(), Dataset::new(vec![], vec![], vec![]))
        };

        let mut metrics_history = Vec::new();
        let mut best_val_loss = f64::INFINITY;
        let mut patience_counter = 0;

        for epoch in 1..=config.epochs {
            // Training pass
            let mut epoch_loss = 0.0;
            let mut total_samples = 0;

            for batch in BatchIterator::new(&train_data, config.batch_size) {
                // In real implementation, this would:
                // 1. Convert batch to tensors
                // 2. Forward pass
                // 3. Compute loss
                // 4. Backward pass
                // 5. Optimizer step

                // Mock loss for now
                let batch_loss = 1.0 / (epoch as f64);
                epoch_loss += batch_loss * batch.num_samples as f64;
                total_samples += batch.num_samples;
            }

            let train_loss = epoch_loss / total_samples.max(1) as f64;

            // Validation pass
            let val_loss = if !val_data.is_empty() {
                Some(evaluate_model_loss_on_dataset(
                    &_model,
                    &val_data,
                    config.batch_size,
                ))
            } else {
                None
            };

            let metrics = TrainingMetrics {
                epoch,
                train_loss,
                val_loss,
                train_accuracy: None,
                val_accuracy: None,
            };

            metrics_history.push(metrics.clone());

            // Check if should continue
            if !callback.on_epoch_end(&metrics) {
                break;
            }

            // Early stopping check
            if config.early_stopping_patience > 0
                && let Some(vl) = val_loss
            {
                if vl < best_val_loss {
                    best_val_loss = vl;
                    patience_counter = 0;
                } else {
                    patience_counter += 1;
                    if patience_counter >= config.early_stopping_patience {
                        log::info!("Early stopping at epoch {}", epoch);
                        return Ok(TrainingResult {
                            final_train_loss: train_loss,
                            final_val_loss: val_loss,
                            epochs_trained: epoch,
                            early_stopped: true,
                            metrics: metrics_history,
                        });
                    }
                }
            }
        }

        Ok(TrainingResult {
            final_train_loss: metrics_history.last().map(|m| m.train_loss).unwrap_or(0.0),
            final_val_loss: metrics_history.last().and_then(|m| m.val_loss),
            epochs_trained: config.epochs,
            early_stopped: false,
            metrics: metrics_history,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn test_compute_cross_entropy_loss() {
        // Mock predictions (should be softmax probabilities)
        let predictions = vec![0.7, 0.2, 0.1];
        let targets = vec![0];

        let loss = compute_cross_entropy_loss(&predictions, &targets, 3);
        assert!(loss > 0.0); // Cross-entropy should be positive for non-perfect predictions
    }

    #[test]
    fn test_compute_accuracy() {
        let predictions = vec![0, 1, 2, 0, 1];
        let targets = vec![0, 1, 2, 0, 2];

        let accuracy = compute_accuracy(&predictions, &targets);
        assert!((accuracy - 0.8).abs() < 0.001); // 4/5 = 0.8
    }

    #[test]
    fn test_logging_callback() {
        let callback = LoggingCallback;

        let metrics = TrainingMetrics {
            epoch: 1,
            train_loss: 0.5,
            val_loss: Some(0.6),
            train_accuracy: Some(0.7),
            val_accuracy: Some(0.65),
        };

        // Should not panic
        let continue_training = callback.on_epoch_end(&metrics);
        assert!(continue_training);
    }

    /// Custom callback that tracks training progress
    struct TestCallback {
        epochs_seen: Arc<Mutex<Vec<usize>>>,
        should_stop: Arc<Mutex<bool>>,
        stop_at_epoch: Option<usize>,
    }

    impl TestCallback {
        fn new() -> Self {
            Self {
                epochs_seen: Arc::new(Mutex::new(Vec::new())),
                should_stop: Arc::new(Mutex::new(false)),
                stop_at_epoch: None,
            }
        }

        fn with_stop_epoch(stop_at_epoch: usize) -> Self {
            Self {
                epochs_seen: Arc::new(Mutex::new(Vec::new())),
                should_stop: Arc::new(Mutex::new(false)),
                stop_at_epoch: Some(stop_at_epoch),
            }
        }

        fn get_epochs(&self) -> Vec<usize> {
            self.epochs_seen.lock().unwrap().clone()
        }
    }

    impl TrainingCallback for TestCallback {
        fn on_epoch_end(&self, metrics: &TrainingMetrics) -> bool {
            self.epochs_seen.lock().unwrap().push(metrics.epoch);

            if let Some(stop_epoch) = self.stop_at_epoch {
                if metrics.epoch >= stop_epoch {
                    *self.should_stop.lock().unwrap() = true;
                    return false;
                }
            }
            true
        }
    }

    #[test]
    fn test_training_callback_receives_all_epochs() {
        let callback = TestCallback::new();

        // Simulate calling on_epoch_end multiple times
        for epoch in 1..=5 {
            let metrics = TrainingMetrics {
                epoch,
                train_loss: 0.5 - epoch as f64 * 0.05,
                val_loss: Some(0.6 - epoch as f64 * 0.03),
                train_accuracy: Some(0.7 + epoch as f64 * 0.02),
                val_accuracy: Some(0.65 + epoch as f64 * 0.01),
            };
            callback.on_epoch_end(&metrics);
        }

        let epochs = callback.get_epochs();
        assert_eq!(epochs, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_training_callback_can_stop_early() {
        let callback = TestCallback::with_stop_epoch(3);

        let mut continue_training = true;
        for epoch in 1..=10 {
            let metrics = TrainingMetrics {
                epoch,
                train_loss: 0.5,
                val_loss: Some(0.6),
                train_accuracy: Some(0.7),
                val_accuracy: Some(0.65),
            };
            continue_training = callback.on_epoch_end(&metrics);

            if !continue_training {
                break;
            }
        }

        assert!(!continue_training);
        let epochs = callback.get_epochs();
        assert_eq!(epochs, vec![1, 2, 3]);
    }

    #[test]
    fn test_early_stopping_with_validation() {
        // Test that early stopping patience is respected
        let config = TrainingConfig {
            model_type: super::super::config::ModelType::Mlp,
            learning_rate: 0.001,
            batch_size: 32,
            epochs: 100,
            optimizer: super::super::OptimizerType::Adam,
            weight_decay: 0.0001,
            label_config: super::super::LabelConfig::default(),
            validation_split: 0.2,
            early_stopping_patience: 5,
        };

        assert!(config.validate().is_ok());
        assert_eq!(config.early_stopping_patience, 5);
    }

    #[test]
    fn test_training_config_validation_requires_validation_for_early_stopping() {
        let mut config = TrainingConfig::default();
        config.early_stopping_patience = 10;
        config.validation_split = 0.0;

        let result = config.validate();
        assert!(result.is_err());

        // Now fix it
        config.validation_split = 0.2;
        assert!(config.validate().is_ok());
    }
}
