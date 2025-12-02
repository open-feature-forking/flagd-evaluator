//! String prefix matching operator.
//!
//! The starts_with operator checks if a string starts with a given prefix.

use datalogic_rs::{ContextStack, Error as DataLogicError, Evaluator, Operator};
use serde_json::Value;

use super::common::{resolve_string_from_context, OperatorResult};

/// Custom operator for string prefix matching.
///
/// Checks if a string starts with a given prefix. The comparison is case-sensitive.
pub struct StartsWithOperator;

impl Operator for StartsWithOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        _evaluator: &dyn Evaluator,
    ) -> OperatorResult<Value> {
        if args.len() < 2 {
            return Err(DataLogicError::InvalidArguments(
                "starts_with operator requires an array with at least 2 elements".into(),
            ));
        }

        let string_value = resolve_string_from_context(&args[0], context)?;
        let prefix = resolve_string_from_context(&args[1], context)?;

        Ok(Value::Bool(starts_with(&string_value, &prefix)))
    }
}

/// Evaluates the starts_with operator for string prefix matching.
///
/// The starts_with operator checks if a string starts with a specific prefix.
/// The comparison is case-sensitive.
///
/// # Arguments
/// * `string_value` - The string to check
/// * `prefix` - The prefix to search for
///
/// # Returns
/// `true` if the string starts with the prefix, `false` otherwise
///
/// # Example
/// ```json
/// {"starts_with": [{"var": "email"}, "admin@"]}
/// ```
/// Returns `true` if email is "admin@example.com"
pub fn starts_with(string_value: &str, prefix: &str) -> bool {
    string_value.starts_with(prefix)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_starts_with_basic() {
        assert!(starts_with("hello world", "hello"));
        assert!(starts_with("admin@example.com", "admin@"));
        assert!(!starts_with("hello world", "world"));
    }

    #[test]
    fn test_starts_with_empty_prefix() {
        // Empty prefix should always return true
        assert!(starts_with("hello", ""));
        assert!(starts_with("", ""));
    }

    #[test]
    fn test_starts_with_empty_string() {
        // Non-empty prefix with empty string should return false
        assert!(!starts_with("", "hello"));
    }

    #[test]
    fn test_starts_with_case_sensitive() {
        assert!(starts_with("/api/users", "/api/"));
        assert!(!starts_with("/API/users", "/api/"));
    }

    #[test]
    fn test_starts_with_exact_match() {
        assert!(starts_with("hello", "hello"));
    }

    #[test]
    fn test_starts_with_prefix_longer_than_string() {
        assert!(!starts_with("hi", "hello"));
    }
}
