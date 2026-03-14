//! Resource limits for the flagd evaluator.
//!
//! These constants define the maximum sizes and depths accepted by the evaluator
//! to prevent resource exhaustion from adversarial or accidentally oversized inputs.

/// Maximum size of a flag configuration payload passed to `update_state`.
///
/// 100 MB covers ~125,000 complex flags at ~800 bytes each — far more than any
/// real-world deployment. The limit prevents accidental or adversarial OOM.
pub const MAX_CONFIG_BYTES: usize = 100 * 1024 * 1024;

/// Maximum size of an evaluation context payload passed to `evaluate`.
///
/// 1 MB covers ~26,000 context fields at ~40 bytes each — far more than any
/// real evaluation context needs.
pub const MAX_CONTEXT_BYTES: usize = 1024 * 1024;

/// Maximum recursion depth for `$ref` resolution in evaluator rules.
///
/// 64 levels is more than any realistic evaluator chain will need.
pub const MAX_REF_DEPTH: usize = 64;

/// Maximum JSON nesting depth accepted by the parser.
///
/// 128 levels covers the deepest realistic targeting rules while preventing
/// stack overflows from malicious inputs.
pub const MAX_JSON_DEPTH: usize = 128;

/// Validates that a JSON string does not exceed `MAX_JSON_DEPTH` nesting levels.
///
/// Scans raw bytes before serde_json parsing to prevent stack overflows from
/// deeply nested JSON objects or arrays.
///
/// # Returns
/// `Ok(())` if the depth is within limits, `Err(String)` with a descriptive message otherwise.
pub fn check_json_depth(input: &str) -> Result<(), String> {
    let mut depth: usize = 0;
    let mut in_string = false;
    let mut escape = false;

    for byte in input.bytes() {
        if escape {
            escape = false;
            continue;
        }
        if in_string {
            match byte {
                b'\\' => escape = true,
                b'"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'{' | b'[' => {
                depth += 1;
                if depth > MAX_JSON_DEPTH {
                    return Err(format!(
                        "JSON nesting depth exceeds the maximum allowed depth of {} levels",
                        MAX_JSON_DEPTH
                    ));
                }
            }
            b'}' | b']' => {
                depth = depth.saturating_sub(1);
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_depth_check_accepts_shallow() {
        assert!(check_json_depth(r#"{"a": {"b": 1}}"#).is_ok());
    }

    #[test]
    fn test_depth_check_accepts_at_limit() {
        let nested = "[".repeat(MAX_JSON_DEPTH) + &"]".repeat(MAX_JSON_DEPTH);
        assert!(check_json_depth(&nested).is_ok());
    }

    #[test]
    fn test_depth_check_rejects_over_limit() {
        let nested = "[".repeat(MAX_JSON_DEPTH + 1) + &"]".repeat(MAX_JSON_DEPTH + 1);
        assert!(check_json_depth(&nested).is_err());
    }

    #[test]
    fn test_depth_check_ignores_brackets_in_strings() {
        let input = r#"{"key": "value with [[[ nested ]]] brackets"}"#;
        assert!(check_json_depth(input).is_ok());
    }
}
