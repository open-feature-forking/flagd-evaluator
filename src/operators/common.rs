//! Common utilities and helper functions for custom operators.
//!
//! This module provides shared functionality used by all custom operators,
//! including variable resolution from the context stack.

use datalogic_rs::{ContextStack, Error as DataLogicError};
use serde_json::Value;
use std::borrow::Cow;

/// Type alias for operator results using datalogic_rs Error type.
pub type OperatorResult<T> = std::result::Result<T, DataLogicError>;

/// Resolves a variable path from the context data, or returns the string value directly.
///
/// Returns `Cow::Borrowed` when the value is already a string in context (avoiding allocation),
/// and `Cow::Owned` only when conversion (number→string) is needed.
pub fn resolve_string_from_context<'a>(
    value: &'a Value,
    context: &'a ContextStack,
) -> OperatorResult<Cow<'a, str>> {
    match value {
        Value::String(s) => Ok(Cow::Borrowed(s.as_str())),
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
                Value::String(s) => Ok(Cow::Owned(s.clone())),
                Value::Number(n) => Ok(Cow::Owned(n.to_string())),
                Value::Null => Ok(Cow::Borrowed("")),
                _ => Err(DataLogicError::TypeError(format!(
                    "Variable '{}' must be a string or number",
                    var_path
                ))),
            }
        }
        Value::Number(n) => Ok(Cow::Owned(n.to_string())),
        Value::Null => Ok(Cow::Borrowed("")),
        _ => Err(DataLogicError::InvalidArguments(
            "Value must be a string, number, null, or var reference".into(),
        )),
    }
}
