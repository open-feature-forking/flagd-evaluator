//! Common utilities and helper functions for custom operators.
//!
//! This module provides shared functionality used by all custom operators,
//! including variable resolution from the context stack.

use datalogic_rs::{DataValue, LogicError};

/// Type alias for operator results using datalogic_rs Error type.
pub type OperatorResult<T> = std::result::Result<T, LogicError>;

/// Resolves a variable path from the context data, or returns the string value directly.
///
/// This helper function handles string values from DataValue types
/// for the custom operators.
pub fn resolve_string_from_datavalue<'a>(value: &DataValue<'a>) -> OperatorResult<String> {
    match value {
        DataValue::String(s) => Ok(s.to_string()),
        DataValue::Number(n) => Ok(n.to_string()),
        DataValue::Null => Ok(String::new()),
        _ => Err(LogicError::Custom(
            "Value must be a string, number, or null".to_string(),
        )),
    }
}
