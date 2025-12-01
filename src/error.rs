//! Error types for the flagd-evaluator library.
//!
//! This module provides structured error types that serialize to JSON
//! for consistent error handling across the WASM boundary.

use serde::{Deserialize, Serialize};

/// The type of error that occurred during evaluation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorType {
    /// Error parsing JSON input
    ParseError,
    /// Error during logic evaluation
    EvaluationError,
    /// Error in memory operations
    MemoryError,
    /// Invalid input provided
    InvalidInput,
}

/// Represents an error that occurred during evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluatorError {
    /// Human-readable error message
    pub message: String,
    /// Type classification of the error
    pub error_type: ErrorType,
}

impl EvaluatorError {
    /// Creates a new parse error.
    ///
    /// # Arguments
    /// * `message` - Description of what failed to parse
    ///
    /// # Example
    /// ```
    /// use flagd_evaluator::error::EvaluatorError;
    /// let err = EvaluatorError::parse_error("Invalid JSON syntax");
    /// ```
    pub fn parse_error(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            error_type: ErrorType::ParseError,
        }
    }

    /// Creates a new evaluation error.
    ///
    /// # Arguments
    /// * `message` - Description of what failed during evaluation
    pub fn evaluation_error(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            error_type: ErrorType::EvaluationError,
        }
    }

    /// Creates a new memory error.
    ///
    /// # Arguments
    /// * `message` - Description of the memory operation that failed
    pub fn memory_error(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            error_type: ErrorType::MemoryError,
        }
    }

    /// Creates a new invalid input error.
    ///
    /// # Arguments
    /// * `message` - Description of why the input was invalid
    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            error_type: ErrorType::InvalidInput,
        }
    }
}

impl std::fmt::Display for EvaluatorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.error_type, self.message)
    }
}

impl std::error::Error for EvaluatorError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_creation() {
        let err = EvaluatorError::parse_error("test error");
        assert_eq!(err.error_type, ErrorType::ParseError);
        assert_eq!(err.message, "test error");
    }

    #[test]
    fn test_error_display() {
        let err = EvaluatorError::evaluation_error("eval failed");
        let display = format!("{}", err);
        assert!(display.contains("eval failed"));
    }
}
