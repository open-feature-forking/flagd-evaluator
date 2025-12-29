//! Error types for the flagd-evaluator library.
//!
//! This module provides structured error types using thiserror for consistent
//! error handling across the library and WASM boundary.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// The type of error that occurred during evaluation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
    /// Flag not found
    FlagNotFound,
    /// Type mismatch in flag value
    TypeMismatch,
    /// Configuration validation error
    ValidationError,
}

/// Represents an error that occurred during evaluation.
#[derive(Debug, Clone, Error, Serialize, Deserialize)]
#[error("{error_type:?}: {message}")]
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

    /// Creates a new flag not found error.
    ///
    /// # Arguments
    /// * `flag_key` - The key of the flag that was not found
    pub fn flag_not_found(flag_key: impl Into<String>) -> Self {
        Self {
            message: format!("Flag not found: {}", flag_key.into()),
            error_type: ErrorType::FlagNotFound,
        }
    }

    /// Creates a new type mismatch error.
    ///
    /// # Arguments
    /// * `message` - Description of the type mismatch
    pub fn type_mismatch(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            error_type: ErrorType::TypeMismatch,
        }
    }

    /// Creates a new validation error.
    ///
    /// # Arguments
    /// * `message` - Description of the validation failure
    pub fn validation_error(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            error_type: ErrorType::ValidationError,
        }
    }

    /// Converts the error to a JSON string.
    pub fn to_json_string(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| {
            format!(
                r#"{{"error_type":"{}","message":"{}"}}"#,
                serde_json::to_string(&self.error_type).unwrap_or_default(),
                self.message
            )
        })
    }
}

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
        assert!(display.contains("EvaluationError"));
    }

    #[test]
    fn test_flag_not_found_error() {
        let err = EvaluatorError::flag_not_found("my-flag");
        assert_eq!(err.error_type, ErrorType::FlagNotFound);
        assert!(err.message.contains("my-flag"));
    }

    #[test]
    fn test_type_mismatch_error() {
        let err = EvaluatorError::type_mismatch("expected boolean, got string");
        assert_eq!(err.error_type, ErrorType::TypeMismatch);
    }

    #[test]
    fn test_validation_error() {
        let err = EvaluatorError::validation_error("invalid config");
        assert_eq!(err.error_type, ErrorType::ValidationError);
    }

    #[test]
    fn test_error_to_json() {
        let err = EvaluatorError::parse_error("test");
        let json = err.to_json_string();
        assert!(json.contains("parse_error"));
        assert!(json.contains("test"));
    }
}
