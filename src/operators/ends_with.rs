//! String suffix matching operator.
//!
//! The ends_with operator checks if a string ends with a given suffix.

use datalogic_rs::{CustomOperator, DataArena, DataValue, EvalContext, LogicError};
use datalogic_rs::logic::Result as DataLogicResult;

use super::common::resolve_string_from_datavalue;

/// Custom operator for string suffix matching.
///
/// Checks if a string ends with a given suffix. The comparison is case-sensitive.
#[derive(Debug)]
pub struct EndsWithOperator;

impl CustomOperator for EndsWithOperator {
    fn evaluate<'a>(
        &self,
        args: &'a [DataValue<'a>],
        _context: &EvalContext<'a>,
        arena: &'a DataArena,
    ) -> DataLogicResult<&'a DataValue<'a>> {
        if args.len() < 2 {
            return Err(LogicError::Custom(
                "ends_with operator requires an array with at least 2 elements".to_string(),
            ));
        }

        let string_value = resolve_string_from_datavalue(&args[0])
            .map_err(|e| LogicError::Custom(format!("Failed to resolve string value: {}", e)))?;
        let suffix = resolve_string_from_datavalue(&args[1])
            .map_err(|e| LogicError::Custom(format!("Failed to resolve suffix: {}", e)))?;

        let result = ends_with(&string_value, &suffix);
        if result {
            Ok(arena.true_value())
        } else {
            Ok(arena.false_value())
        }
    }
}

/// Evaluates the ends_with operator for string suffix matching.
///
/// The ends_with operator checks if a string ends with a specific suffix.
/// The comparison is case-sensitive.
///
/// # Arguments
/// * `string_value` - The string to check
/// * `suffix` - The suffix to search for
///
/// # Returns
/// `true` if the string ends with the suffix, `false` otherwise
///
/// # Example
/// ```json
/// {"ends_with": [{"var": "filename"}, ".pdf"]}
/// ```
/// Returns `true` if filename is "document.pdf"
pub fn ends_with(string_value: &str, suffix: &str) -> bool {
    string_value.ends_with(suffix)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ends_with_basic() {
        assert!(ends_with("hello world", "world"));
        assert!(ends_with("document.pdf", ".pdf"));
        assert!(!ends_with("hello world", "hello"));
    }

    #[test]
    fn test_ends_with_empty_suffix() {
        // Empty suffix should always return true
        assert!(ends_with("hello", ""));
        assert!(ends_with("", ""));
    }

    #[test]
    fn test_ends_with_empty_string() {
        // Non-empty suffix with empty string should return false
        assert!(!ends_with("", "hello"));
    }

    #[test]
    fn test_ends_with_case_sensitive() {
        assert!(ends_with("https://example.com", ".com"));
        assert!(!ends_with("https://example.COM", ".com"));
    }

    #[test]
    fn test_ends_with_exact_match() {
        assert!(ends_with("hello", "hello"));
    }

    #[test]
    fn test_ends_with_suffix_longer_than_string() {
        assert!(!ends_with("hi", "hello"));
    }
}
