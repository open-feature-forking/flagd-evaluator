//! Common utilities and helper functions for custom operators.
//!
//! This module provides shared functionality used by all custom operators,
//! including variable resolution from the context stack.

use datalogic_rs::{ContextStack, Error as DataLogicError};
use serde_json::Value;

/// Type alias for operator results using datalogic_rs Error type.
pub type OperatorResult<T> = std::result::Result<T, DataLogicError>;

/// Resolves a variable path from the context data, or returns the string value directly.
///
/// This helper function handles both direct string values and variable references
/// (like `{"var": "path.to.value"}`) for the custom operators.
pub fn resolve_string_from_context(
    value: &Value,
    context: &ContextStack,
) -> OperatorResult<String> {
    match value {
        Value::String(s) => Ok(s.clone()),
        Value::Object(obj) if obj.contains_key("var") => {
            let var_path = obj.get("var").and_then(|v| v.as_str()).ok_or_else(|| {
                DataLogicError::InvalidArguments("var reference must be a string".into())
            })?;

            // Get root data and navigate the path
            let root_ref = context.root();
            let data = root_ref.data();
            let mut current = data;
            for part in var_path.split('.') {
                current = current.get(part).ok_or_else(|| {
                    DataLogicError::VariableNotFound(format!(
                        "Variable '{}' not found in data",
                        var_path
                    ))
                })?;
            }

            match current {
                Value::String(s) => Ok(s.clone()),
                Value::Number(n) => Ok(n.to_string()),
                Value::Null => Ok(String::new()),
                _ => Err(DataLogicError::TypeError(format!(
                    "Variable '{}' must be a string or number",
                    var_path
                ))),
            }
        }
        Value::Number(n) => Ok(n.to_string()),
        Value::Null => Ok(String::new()),
        _ => Err(DataLogicError::InvalidArguments(
            "Value must be a string, number, null, or var reference".into(),
        )),
    }
}
