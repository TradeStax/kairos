//! Parameter space definitions for grid-search optimization.
//!
//! Provides [`ParameterRange`] to define the domain of a single
//! parameter and [`ParameterGrid`] to enumerate all combinations
//! across multiple parameters (full Cartesian product).

use kairos_study::ParameterValue;
use std::collections::HashMap;

/// Defines the set of candidate values for a single study parameter.
///
/// Ranges can be constructed from integer steps, floating-point
/// steps, or explicit value lists.
#[derive(Debug, Clone)]
pub struct ParameterRange {
    /// Parameter key matching the study's `ParameterDef` name.
    pub key: String,
    /// Ordered list of candidate values to test.
    pub values: Vec<ParameterValue>,
}

impl ParameterRange {
    /// Creates an integer range stepping from `min` to `max`
    /// (inclusive) by `step`.
    ///
    /// # Example
    ///
    /// `ParameterRange::integer("period", 5, 20, 5)` produces
    /// values `[5, 10, 15, 20]`.
    #[must_use]
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

    /// Creates a floating-point range stepping from `min` to `max`
    /// (inclusive, with half-step tolerance) by `step`.
    ///
    /// The upper bound includes `max` if `max` is within `step/2`
    /// of the last generated value to handle floating-point
    /// rounding.
    #[must_use]
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

    /// Creates a range from an explicit list of values.
    #[must_use]
    pub fn explicit(key: &str, values: Vec<ParameterValue>) -> Self {
        Self {
            key: key.to_string(),
            values,
        }
    }
}

/// Generates the full Cartesian product of multiple parameter
/// ranges (exhaustive grid search).
///
/// Given `N` ranges with sizes `[s1, s2, ..., sN]`, produces
/// `s1 * s2 * ... * sN` parameter combinations, each represented
/// as a `HashMap<String, ParameterValue>`.
pub struct ParameterGrid {
    /// The parameter ranges whose Cartesian product is enumerated.
    ranges: Vec<ParameterRange>,
}

impl ParameterGrid {
    /// Creates a new grid from the given parameter ranges.
    #[must_use]
    pub fn new(ranges: Vec<ParameterRange>) -> Self {
        Self { ranges }
    }

    /// Returns the total number of parameter combinations.
    ///
    /// This is the product of the sizes of all ranges. Returns 0
    /// if no ranges are defined.
    #[must_use]
    pub fn total_combinations(&self) -> usize {
        if self.ranges.is_empty() {
            return 0;
        }
        self.ranges.iter().map(|r| r.values.len()).product()
    }

    /// Enumerates all parameter combinations as a list of
    /// key-value maps.
    ///
    /// Uses an odometer-style index to iterate through the
    /// Cartesian product without recursion. Returns a single
    /// empty map if no ranges are defined.
    #[must_use]
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

            // Increment indices (odometer-style carry)
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
