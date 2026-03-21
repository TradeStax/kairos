//! # Model Output Types
//!
//! Defines the output types for ML models including trading signals and probabilities.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Trading signal direction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TradingSignal {
    /// Long position signal
    Long,
    /// Short position signal
    Short,
    /// No position / neutral
    Neutral,
}

impl fmt::Display for TradingSignal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TradingSignal::Long => write!(f, "long"),
            TradingSignal::Short => write!(f, "short"),
            TradingSignal::Neutral => write!(f, "neutral"),
        }
    }
}

impl TradingSignal {
    /// Convert signal to index (0=long, 1=neutral, 2=short)
    pub fn to_index(&self) -> usize {
        match self {
            TradingSignal::Long => 0,
            TradingSignal::Neutral => 1,
            TradingSignal::Short => 2,
        }
    }

    /// Create signal from index
    pub fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(TradingSignal::Long),
            1 => Some(TradingSignal::Neutral),
            2 => Some(TradingSignal::Short),
            _ => None,
        }
    }

    /// Check if signal is long
    pub fn is_long(&self) -> bool {
        matches!(self, TradingSignal::Long)
    }

    /// Check if signal is short
    pub fn is_short(&self) -> bool {
        matches!(self, TradingSignal::Short)
    }

    /// Check if signal is neutral
    pub fn is_neutral(&self) -> bool {
        matches!(self, TradingSignal::Neutral)
    }
}

/// Model output types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ModelOutput {
    /// Classification output with probabilities for each class
    Classification {
        /// Probabilities for [long, neutral, short]
        probabilities: [f64; 3],
        /// Predicted class
        prediction: TradingSignal,
    },
    /// Regression output with raw value
    Regression {
        /// Raw prediction value
        value: f64,
    },
}

impl ModelOutput {
    /// Get the trading signal from the output
    pub fn signal(&self) -> TradingSignal {
        match self {
            ModelOutput::Classification { prediction, .. } => *prediction,
            ModelOutput::Regression { value } => {
                if *value > 0.0 {
                    TradingSignal::Long
                } else if *value < 0.0 {
                    TradingSignal::Short
                } else {
                    TradingSignal::Neutral
                }
            }
        }
    }

    /// Get confidence score (probability for classification, absolute value for regression)
    pub fn confidence(&self) -> f64 {
        match self {
            ModelOutput::Classification {
                probabilities,
                prediction,
            } => {
                let idx = prediction.to_index();
                probabilities[idx]
            }
            ModelOutput::Regression { value } => value.abs().min(1.0),
        }
    }

    /// Check if confidence is above threshold
    pub fn is_confident(&self, threshold: f64) -> bool {
        self.confidence() >= threshold
    }
}

impl fmt::Display for ModelOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ModelOutput::Classification {
                probabilities,
                prediction,
            } => {
                write!(
                    f,
                    "Classification({}, {:.2}/{:.2}/{:.2})",
                    prediction, probabilities[0], probabilities[1], probabilities[2]
                )
            }
            ModelOutput::Regression { value } => {
                write!(f, "Regression({:.4})", value)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trading_signal_display() {
        assert_eq!(TradingSignal::Long.to_string(), "long");
        assert_eq!(TradingSignal::Short.to_string(), "short");
        assert_eq!(TradingSignal::Neutral.to_string(), "neutral");
    }

    #[test]
    fn test_trading_signal_index() {
        assert_eq!(TradingSignal::Long.to_index(), 0);
        assert_eq!(TradingSignal::Neutral.to_index(), 1);
        assert_eq!(TradingSignal::Short.to_index(), 2);

        assert_eq!(TradingSignal::from_index(0), Some(TradingSignal::Long));
        assert_eq!(TradingSignal::from_index(3), None);
    }

    #[test]
    fn test_model_output_classification_serialization() {
        let output = ModelOutput::Classification {
            probabilities: [0.7, 0.2, 0.1],
            prediction: TradingSignal::Long,
        };

        let json = serde_json::to_string(&output).unwrap();
        assert!(json.contains("long"));
        assert!(json.contains("classification"));

        let parsed: ModelOutput = serde_json::from_str(&json).unwrap();
        match parsed {
            ModelOutput::Classification {
                probabilities,
                prediction,
            } => {
                assert_eq!(probabilities, [0.7, 0.2, 0.1]);
                assert_eq!(prediction, TradingSignal::Long);
            }
            _ => panic!("Expected Classification"),
        }
    }

    #[test]
    fn test_model_output_regression_serialization() {
        let output = ModelOutput::Regression { value: 1.5 };

        let json = serde_json::to_string(&output).unwrap();
        assert!(json.contains("regression"));

        let parsed: ModelOutput = serde_json::from_str(&json).unwrap();
        match parsed {
            ModelOutput::Regression { value } => {
                assert!((value - 1.5).abs() < 0.001);
            }
            _ => panic!("Expected Regression"),
        }
    }

    #[test]
    fn test_model_output_signal() {
        let classification = ModelOutput::Classification {
            probabilities: [0.7, 0.2, 0.1],
            prediction: TradingSignal::Long,
        };
        assert_eq!(classification.signal(), TradingSignal::Long);

        let regression_pos = ModelOutput::Regression { value: 0.5 };
        assert_eq!(regression_pos.signal(), TradingSignal::Long);

        let regression_neg = ModelOutput::Regression { value: -0.5 };
        assert_eq!(regression_neg.signal(), TradingSignal::Short);

        let regression_zero = ModelOutput::Regression { value: 0.0 };
        assert_eq!(regression_zero.signal(), TradingSignal::Neutral);
    }

    #[test]
    fn test_model_output_confidence() {
        let high_conf = ModelOutput::Classification {
            probabilities: [0.9, 0.05, 0.05],
            prediction: TradingSignal::Long,
        };
        assert!((high_conf.confidence() - 0.9).abs() < 0.001);
        assert!(high_conf.is_confident(0.85));
        assert!(!high_conf.is_confident(0.95));
    }
}
