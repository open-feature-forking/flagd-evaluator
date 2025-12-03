//! String prefix matching operator.
//!
//! The starts_with operator checks if a string starts with a given prefix.

use datalogic_rs::{CustomOperator, DataArena, DataValue, EvalContext, LogicError};
use datalogic_rs::logic::Result as DataLogicResult;

use super::common::resolve_string_from_datavalue;

/// Custom operator for string prefix matching.
///
/// Checks if a string starts with a given prefix. The comparison is case-sensitive.
#[derive(Debug)]
pub struct StartsWithOperator;

impl CustomOperator for StartsWithOperator {
    fn evaluate<'a>(
        &self,
        args: &'a [DataValue<'a>],
        _context: &EvalContext<'a>,
        arena: &'a DataArena,
    ) -> DataLogicResult<&'a DataValue<'a>> {
        if args.len() < 2 {
            return Err(LogicError::Custom(
                "starts_with operator requires an array with at least 2 elements".to_string(),
            ));
        }

        let string_value = resolve_string_from_datavalue(&args[0])
            .map_err(|e| LogicError::Custom(format!("Failed to resolve string value: {}", e)))?;
        let prefix = resolve_string_from_datavalue(&args[1])
            .map_err(|e| LogicError::Custom(format!("Failed to resolve prefix: {}", e)))?;

        let result = starts_with(&string_value, &prefix);
        if result {
            Ok(arena.true_value())
        } else {
            Ok(arena.false_value())
        }
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
