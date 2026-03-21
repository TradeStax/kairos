//! # Dataset Module
//!
//! Dataset structures and utilities for training.

use serde::{Deserialize, Serialize};

/// Training dataset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dataset {
    /// Feature tensor [num_samples, lookback, num_features]
    pub features: Vec<Vec<Vec<f64>>>,
    /// Label tensor [num_samples]
    pub labels: Vec<usize>,
    /// Timestamps for each sample
    pub timestamps: Vec<i64>,
}

impl Dataset {
    /// Create a new dataset
    pub fn new(features: Vec<Vec<Vec<f64>>>, labels: Vec<usize>, timestamps: Vec<i64>) -> Self {
        assert_eq!(features.len(), labels.len());
        assert_eq!(features.len(), timestamps.len());

        Self {
            features,
            labels,
            timestamps,
        }
    }

    /// Get the number of samples
    pub fn len(&self) -> usize {
        self.features.len()
    }

    /// Check if dataset is empty
    pub fn is_empty(&self) -> bool {
        self.features.is_empty()
    }

    /// Get number of features
    pub fn num_features(&self) -> usize {
        self.features
            .first()
            .and_then(|f| f.first())
            .map(|f| f.len())
            .unwrap_or(0)
    }

    /// Get lookback period
    pub fn lookback(&self) -> usize {
        self.features.first().map(|f| f.len()).unwrap_or(0)
    }

    /// Split dataset into training and validation sets
    pub fn split(&self, validation_ratio: f64) -> (Dataset, Dataset) {
        let split_idx = ((1.0 - validation_ratio) * self.len() as f64) as usize;
        let split_idx = split_idx.max(1).min(self.len().saturating_sub(1));

        let (train_features, val_features) = self.features.split_at(split_idx);
        let (train_labels, val_labels) = self.labels.split_at(split_idx);
        let (train_timestamps, val_timestamps) = self.timestamps.split_at(split_idx);

        (
            Dataset::new(
                train_features.to_vec(),
                train_labels.to_vec(),
                train_timestamps.to_vec(),
            ),
            Dataset::new(
                val_features.to_vec(),
                val_labels.to_vec(),
                val_timestamps.to_vec(),
            ),
        )
    }
}

/// Batch iterator for training
pub struct BatchIterator<'a> {
    dataset: &'a Dataset,
    batch_size: usize,
    current_idx: usize,
}

impl<'a> BatchIterator<'a> {
    /// Create a new batch iterator
    pub fn new(dataset: &'a Dataset, batch_size: usize) -> Self {
        Self {
            dataset,
            batch_size,
            current_idx: 0,
        }
    }
}

impl<'a> Iterator for BatchIterator<'a> {
    type Item = Batch;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_idx >= self.dataset.len() {
            return None;
        }

        let end_idx = (self.current_idx + self.batch_size).min(self.dataset.len());

        // Flatten samples: [sample][lookback][features] -> [sample*lookback*features]
        let mut features: Vec<f64> = Vec::new();
        for sample_features in &self.dataset.features[self.current_idx..end_idx] {
            for time_step in sample_features {
                features.extend_from_slice(time_step);
            }
        }

        let labels: Vec<usize> = self.dataset.labels[self.current_idx..end_idx].to_vec();

        let num_samples = end_idx - self.current_idx;
        let lookback = self.dataset.lookback();
        let num_features = self.dataset.num_features();

        self.current_idx = end_idx;

        Some(Batch {
            features,
            labels,
            num_samples,
            lookback,
            num_features,
        })
    }
}

/// A single batch of data
#[derive(Debug)]
pub struct Batch {
    /// Flattened feature data [num_samples * lookback * num_features]
    pub features: Vec<f64>,
    /// Labels [num_samples]
    pub labels: Vec<usize>,
    /// Number of samples in batch
    pub num_samples: usize,
    /// Lookback period
    pub lookback: usize,
    /// Number of features
    pub num_features: usize,
}

impl Batch {
    /// Get the shape of the feature tensor [batch, lookback, features]
    pub fn feature_shape(&self) -> [usize; 3] {
        [self.num_samples, self.lookback, self.num_features]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_dataset() -> Dataset {
        let features = vec![
            vec![vec![1.0, 2.0], vec![3.0, 4.0]],    // sample 0
            vec![vec![5.0, 6.0], vec![7.0, 8.0]],    // sample 1
            vec![vec![9.0, 10.0], vec![11.0, 12.0]], // sample 2
        ];
        let labels = vec![0, 1, 2];
        let timestamps = vec![1, 2, 3];

        Dataset::new(features, labels, timestamps)
    }

    #[test]
    fn test_dataset_shapes_match() {
        let dataset = create_test_dataset();

        assert_eq!(dataset.len(), 3);
        assert_eq!(dataset.num_features(), 2);
        assert_eq!(dataset.lookback(), 2);
    }

    #[test]
    fn test_dataset_splits_into_train_validation() {
        let dataset = create_test_dataset();

        let (train, val) = dataset.split(0.33);

        assert!(train.len() >= 2);
        assert!(val.len() >= 1);
        assert_eq!(train.len() + val.len(), dataset.len());
    }

    #[test]
    fn test_batch_iterator() {
        let dataset = create_test_dataset();

        let mut iter = BatchIterator::new(&dataset, 2);

        let batch1 = iter.next().unwrap();
        assert_eq!(batch1.num_samples, 2);
        assert_eq!(batch1.lookback, 2);
        assert_eq!(batch1.num_features, 2);
        assert_eq!(batch1.labels, vec![0, 1]);

        let batch2 = iter.next().unwrap();
        assert_eq!(batch2.num_samples, 1);
        assert_eq!(batch2.labels, vec![2]);

        assert!(iter.next().is_none());
    }

    #[test]
    fn test_batch_feature_shape() {
        let dataset = create_test_dataset();
        let batch = BatchIterator::new(&dataset, 2).next().unwrap();

        assert_eq!(batch.feature_shape(), [2, 2, 2]);
    }
}
