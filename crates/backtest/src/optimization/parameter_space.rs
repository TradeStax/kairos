use kairos_study::ParameterValue;
use std::collections::HashMap;

/// A range of values for a single parameter.
#[derive(Debug, Clone)]
pub struct ParameterRange {
    pub key: String,
    pub values: Vec<ParameterValue>,
}

impl ParameterRange {
    /// Create an integer range with step.
    pub fn integer(key: &str, min: i64, max: i64, step: i64) -> Self {
        let mut values = Vec::new();
        let mut v = min;
        while v <= max {
            values.push(ParameterValue::Integer(v));
            v += step;
        }
        Self {
            key: key.to_string(),
            values,
        }
    }

    /// Create a float range with step.
    pub fn float(key: &str, min: f64, max: f64, step: f64) -> Self {
        let mut values = Vec::new();
        let mut v = min;
        while v <= max + step * 0.5 {
            values.push(ParameterValue::Float(v));
            v += step;
        }
        Self {
            key: key.to_string(),
            values,
        }
    }

    /// Create from explicit values.
    pub fn explicit(key: &str, values: Vec<ParameterValue>) -> Self {
        Self {
            key: key.to_string(),
            values,
        }
    }
}

/// Generates all combinations of parameter values
/// (grid search).
pub struct ParameterGrid {
    ranges: Vec<ParameterRange>,
}

impl ParameterGrid {
    pub fn new(ranges: Vec<ParameterRange>) -> Self {
        Self { ranges }
    }

    /// Total number of parameter combinations.
    pub fn total_combinations(&self) -> usize {
        if self.ranges.is_empty() {
            return 0;
        }
        self.ranges.iter().map(|r| r.values.len()).product()
    }

    /// Generate all parameter combinations as a list of
    /// (key -> value) maps.
    pub fn combinations(&self) -> Vec<HashMap<String, ParameterValue>> {
        if self.ranges.is_empty() {
            return vec![HashMap::new()];
        }

        let total = self.total_combinations();
        let mut result = Vec::with_capacity(total);

        let mut indices = vec![0usize; self.ranges.len()];

        loop {
            let mut combo = HashMap::new();
            for (i, range) in self.ranges.iter().enumerate() {
                combo.insert(range.key.clone(), range.values[indices[i]].clone());
            }
            result.push(combo);

            // Increment indices (odometer-style)
            let mut carry = true;
            for i in (0..indices.len()).rev() {
                if carry {
                    indices[i] += 1;
                    if indices[i] >= self.ranges[i].values.len() {
                        indices[i] = 0;
                    } else {
                        carry = false;
                    }
                }
            }
            if carry {
                break;
            }
        }

        result
    }
}
