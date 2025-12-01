//! # flagd-evaluator
//!
//! A WebAssembly-based JSON Logic evaluator with custom operators for feature flag evaluation.
//!
//! This library is designed to work with Chicory (pure Java WebAssembly runtime) and other
//! WASM runtimes. It provides a minimal API for evaluating JSON Logic rules with support for
//! custom operators like `fractional` for A/B testing.
//!
//! ## Features
//!
//! - **JSON Logic Evaluation**: Full support for standard JSON Logic operations via `datalogic-rs`
//! - **Custom Operators**: Support for feature-flag specific operators like `fractional`
//! - **Memory Safe**: Clean memory management with explicit alloc/dealloc functions
//! - **Zero JNI**: Works with pure Java WASM runtimes like Chicory
//!
//! ## Exported Functions
//!
//! - `evaluate_logic`: Main evaluation function
//! - `wasm_alloc`: Allocate memory from WASM linear memory
//! - `wasm_dealloc`: Free allocated memory
//!
//! ## Example
//!
//! ```ignore
//! // From Java via Chicory:
//! // 1. Allocate memory for rule and data strings
//! // 2. Copy strings to WASM memory
//! // 3. Call evaluate_logic with packed pointers
//! // 4. Parse the returned JSON result
//! // 5. Free allocated memory
//! ```

pub mod error;
pub mod memory;
pub mod operators;

use datalogic_rs::DataLogic;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use error::{ErrorType, EvaluatorError};
pub use memory::{
    pack_ptr_len, string_from_memory, string_to_memory, unpack_ptr_len, wasm_alloc, wasm_dealloc,
};
pub use operators::fractional;

/// The response format for evaluation results.
///
/// This struct is always returned as JSON from `evaluate_logic`,
/// providing a consistent interface for both success and error cases.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationResponse {
    /// Whether the evaluation succeeded
    pub success: bool,
    /// The evaluation result (null if error)
    pub result: Option<Value>,
    /// Error message (null if success)
    pub error: Option<String>,
}

impl EvaluationResponse {
    /// Creates a successful response with the given result.
    pub fn success(result: Value) -> Self {
        Self {
            success: true,
            result: Some(result),
            error: None,
        }
    }

    /// Creates an error response with the given message.
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            success: false,
            result: None,
            error: Some(message.into()),
        }
    }

    /// Serializes the response to a JSON string.
    pub fn to_json_string(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|e| {
            format!(
                r#"{{"success":false,"result":null,"error":"Serialization failed: {}"}}"#,
                e
            )
        })
    }
}

/// Checks if a JSON value contains the custom "fractional" operator.
///
/// Returns the operator arguments if found, or None if not present.
fn extract_fractional_op(value: &Value) -> Option<&Value> {
    value.get("fractional")
}

/// Handles the custom fractional operator if present in the rule.
///
/// This function processes rules that use the "fractional" operator for A/B testing.
/// If the rule is a fractional operation, it evaluates it; otherwise, it returns None
/// to indicate standard JSON Logic evaluation should be used.
fn handle_fractional(rule: &Value, data: &Value) -> Option<Result<Value, String>> {
    let args = extract_fractional_op(rule)?;

    let args_array = match args {
        Value::Array(arr) if arr.len() >= 2 => arr,
        _ => {
            return Some(Err(
                "fractional operator requires an array with at least 2 elements".to_string(),
            ))
        }
    };

    // First argument is the bucket key (can be a value or a var reference)
    let bucket_key = match &args_array[0] {
        Value::String(s) => s.clone(),
        Value::Object(obj) if obj.contains_key("var") => {
            // Resolve variable reference from data
            let var_path = match obj.get("var") {
                Some(Value::String(s)) => s,
                _ => return Some(Err("var reference must be a string".to_string())),
            };

            // Navigate the data object using the variable path
            let mut current = data;
            for part in var_path.split('.') {
                current = match current.get(part) {
                    Some(v) => v,
                    None => return Some(Err(format!("Variable '{}' not found in data", var_path))),
                };
            }

            match current {
                Value::String(s) => s.clone(),
                Value::Number(n) => n.to_string(),
                _ => {
                    return Some(Err(format!(
                        "Variable '{}' must be a string or number",
                        var_path
                    )))
                }
            }
        }
        Value::Number(n) => n.to_string(),
        _ => {
            return Some(Err(
                "First argument must be a string, number, or var reference".to_string(),
            ))
        }
    };

    // Second argument is the buckets array
    let buckets = match &args_array[1] {
        Value::Array(arr) => arr.as_slice(),
        _ => {
            return Some(Err(
                "Second argument must be an array of bucket definitions".to_string(),
            ))
        }
    };

    match crate::operators::fractional(&bucket_key, buckets) {
        Ok(bucket_name) => Some(Ok(Value::String(bucket_name))),
        Err(e) => Some(Err(e)),
    }
}

/// Evaluates a JSON Logic rule against the provided data.
///
/// This is the main entry point for the library. It accepts JSON strings for both
/// the rule and data, evaluates the rule, and returns a JSON response string.
///
/// # Arguments
/// * `rule_ptr` - Pointer to the rule JSON string in WASM memory
/// * `rule_len` - Length of the rule JSON string
/// * `data_ptr` - Pointer to the data JSON string in WASM memory  
/// * `data_len` - Length of the data JSON string
///
/// # Returns
/// A packed u64 containing the pointer (upper 32 bits) and length (lower 32 bits)
/// of the response JSON string. The caller must free this memory using `wasm_dealloc`.
///
/// # Response Format
/// The response is always valid JSON with the following structure:
/// ```json
/// {
///   "success": true|false,
///   "result": <value>|null,
///   "error": null|"error message"
/// }
/// ```
///
/// # Safety
/// The caller must ensure:
/// - `rule_ptr` and `data_ptr` point to valid memory
/// - The memory regions do not overlap
/// - The strings are valid UTF-8
#[no_mangle]
pub extern "C" fn evaluate_logic(
    rule_ptr: *const u8,
    rule_len: u32,
    data_ptr: *const u8,
    data_len: u32,
) -> u64 {
    let response = evaluate_logic_internal(rule_ptr, rule_len, data_ptr, data_len);
    string_to_memory(&response.to_json_string())
}

/// Internal evaluation function that handles all the logic.
fn evaluate_logic_internal(
    rule_ptr: *const u8,
    rule_len: u32,
    data_ptr: *const u8,
    data_len: u32,
) -> EvaluationResponse {
    // SAFETY: The caller guarantees valid memory regions
    let rule_str = match unsafe { string_from_memory(rule_ptr, rule_len) } {
        Ok(s) => s,
        Err(e) => return EvaluationResponse::error(format!("Failed to read rule: {}", e)),
    };

    let data_str = match unsafe { string_from_memory(data_ptr, data_len) } {
        Ok(s) => s,
        Err(e) => return EvaluationResponse::error(format!("Failed to read data: {}", e)),
    };

    // Parse the rule JSON
    let rule: Value = match serde_json::from_str(&rule_str) {
        Ok(v) => v,
        Err(e) => return EvaluationResponse::error(format!("Failed to parse rule JSON: {}", e)),
    };

    // Parse the data JSON
    let data: Value = match serde_json::from_str(&data_str) {
        Ok(v) => v,
        Err(e) => return EvaluationResponse::error(format!("Failed to parse data JSON: {}", e)),
    };

    // Check for custom fractional operator first
    if let Some(result) = handle_fractional(&rule, &data) {
        return match result {
            Ok(value) => EvaluationResponse::success(value),
            Err(e) => EvaluationResponse::error(e),
        };
    }

    // Use datalogic-rs for standard JSON Logic evaluation
    let logic = DataLogic::new();
    match logic.evaluate_json(&rule_str, &data_str) {
        Ok(result) => EvaluationResponse::success(result),
        Err(e) => EvaluationResponse::error(format!("Evaluation error: {}", e)),
    }
}

/// Re-exports for external access to allocation functions.
///
/// These are the primary memory management functions that should be used
/// by the host runtime (e.g., Java via Chicory).
#[no_mangle]
pub extern "C" fn alloc(len: u32) -> *mut u8 {
    wasm_alloc(len)
}

#[no_mangle]
pub extern "C" fn dealloc(ptr: *mut u8, len: u32) {
    wasm_dealloc(ptr, len)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn evaluate_json(rule: &str, data: &str) -> EvaluationResponse {
        let rule_bytes = rule.as_bytes();
        let data_bytes = data.as_bytes();
        evaluate_logic_internal(
            rule_bytes.as_ptr(),
            rule_bytes.len() as u32,
            data_bytes.as_ptr(),
            data_bytes.len() as u32,
        )
    }

    #[test]
    fn test_basic_equality() {
        let result = evaluate_json(r#"{"==": [1, 1]}"#, "{}");
        assert!(result.success);
        assert_eq!(result.result, Some(json!(true)));
    }

    #[test]
    fn test_variable_access() {
        let result = evaluate_json(r#"{"var": "name"}"#, r#"{"name": "Alice"}"#);
        assert!(result.success);
        assert_eq!(result.result, Some(json!("Alice")));
    }

    #[test]
    fn test_comparison() {
        let result = evaluate_json(r#"{">": [{"var": "age"}, 18]}"#, r#"{"age": 25}"#);
        assert!(result.success);
        assert_eq!(result.result, Some(json!(true)));
    }

    #[test]
    fn test_conditional() {
        let rule = r#"{"if": [{"<": [{"var": "temp"}, 0]}, "freezing", "not freezing"]}"#;
        let data = r#"{"temp": -5}"#;
        let result = evaluate_json(rule, data);
        assert!(result.success);
        assert_eq!(result.result, Some(json!("freezing")));
    }

    #[test]
    fn test_invalid_rule_json() {
        let result = evaluate_json("not valid json", "{}");
        assert!(!result.success);
        assert!(result.error.is_some());
        assert!(result.error.unwrap().contains("parse"));
    }

    #[test]
    fn test_invalid_data_json() {
        let result = evaluate_json("{}", "not valid json");
        assert!(!result.success);
        assert!(result.error.is_some());
    }

    #[test]
    fn test_fractional_operator() {
        let rule = r#"{"fractional": ["user-123", ["control", 50, "treatment", 50]]}"#;
        let result = evaluate_json(rule, "{}");
        assert!(result.success);
        let bucket = result.result.unwrap();
        assert!(bucket == json!("control") || bucket == json!("treatment"));
    }

    #[test]
    fn test_fractional_with_var() {
        let rule = r#"{"fractional": [{"var": "user.id"}, ["a", 50, "b", 50]]}"#;
        let data = r#"{"user": {"id": "test-user-42"}}"#;
        let result = evaluate_json(rule, data);
        assert!(result.success);
    }

    #[test]
    fn test_fractional_consistency() {
        let rule = r#"{"fractional": ["consistent-key", ["bucket1", 50, "bucket2", 50]]}"#;

        // Same input should always produce same output
        let result1 = evaluate_json(rule, "{}");
        let result2 = evaluate_json(rule, "{}");

        assert!(result1.success);
        assert!(result2.success);
        assert_eq!(result1.result, result2.result);
    }

    #[test]
    fn test_empty_data() {
        let result = evaluate_json(r#"{"==": [1, 1]}"#, "{}");
        assert!(result.success);
    }

    #[test]
    fn test_null_value() {
        let result = evaluate_json(r#"{"var": "missing"}"#, "{}");
        assert!(result.success);
        assert_eq!(result.result, Some(json!(null)));
    }

    #[test]
    fn test_nested_operations() {
        let rule = r#"{"and": [{"<": [{"var": "a"}, 10]}, {">": [{"var": "b"}, 5]}]}"#;
        let data = r#"{"a": 5, "b": 10}"#;
        let result = evaluate_json(rule, data);
        assert!(result.success);
        assert_eq!(result.result, Some(json!(true)));
    }

    #[test]
    fn test_array_operations() {
        let rule = r#"{"in": ["world", {"var": "greeting"}]}"#;
        let data = r#"{"greeting": "hello world"}"#;
        let result = evaluate_json(rule, data);
        assert!(result.success);
        assert_eq!(result.result, Some(json!(true)));
    }

    #[test]
    fn test_response_serialization() {
        let response = EvaluationResponse::success(json!(42));
        let json_str = response.to_json_string();
        assert!(json_str.contains("\"success\":true"));
        assert!(json_str.contains("\"result\":42"));
    }

    #[test]
    fn test_error_response() {
        let response = EvaluationResponse::error("test error");
        assert!(!response.success);
        assert_eq!(response.error, Some("test error".to_string()));
        assert_eq!(response.result, None);
    }
}
