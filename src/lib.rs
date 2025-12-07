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
//! - **Custom Operators**: Support for feature-flag specific operators like `fractional`, `starts_with`,
//!   `ends_with`, and `sem_ver` - all registered via the `datalogic_rs::Operator` trait
//! - **Feature Flag Evaluation**: State-based flag evaluation following the flagd provider specification
//! - **Memory Safe**: Clean memory management with explicit alloc/dealloc functions
//! - **Zero JNI**: Works with pure Java WASM runtimes like Chicory
//!
//! ## Exported Functions
//!
//! - `evaluate_logic`: Evaluates JSON Logic rules directly
//! - `update_state`: Updates the feature flag configuration state
//! - `evaluate`: Evaluates a feature flag against context (requires prior `update_state` call)
//! - `wasm_alloc`: Allocate memory from WASM linear memory
//! - `wasm_dealloc`: Free allocated memory
//!
//! ## Example
//!
//! ```ignore
//! // From Java via Chicory:
//! // 1. Update state with flag configuration
//! // 2. Allocate memory for flag key and context strings
//! // 3. Copy strings to WASM memory
//! // 4. Call evaluate with pointers
//! // 5. Parse the returned JSON result
//! // 6. Free allocated memory
//! ```

pub mod error;
pub mod evaluation;
pub mod memory;
pub mod model;
pub mod operators;
pub mod storage;
pub mod validation;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use error::{ErrorType, EvaluatorError};
pub use evaluation::{
    evaluate_bool_flag, evaluate_flag, evaluate_float_flag, evaluate_int_flag,
    evaluate_object_flag, evaluate_string_flag, ErrorCode, EvaluationResult, ResolutionReason,
};
pub use memory::{
    pack_ptr_len, string_from_memory, string_to_memory, unpack_ptr_len, wasm_alloc, wasm_dealloc,
};
pub use model::{FeatureFlag, ParsingResult};
pub use operators::{create_evaluator, ends_with, fractional, sem_ver, starts_with};
pub use storage::{clear_flag_state, get_flag_state, update_flag_state, set_validation_mode, ValidationMode};
pub use validation::{validate_flags_config, ValidationError, ValidationResult};

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
///
/// Uses the DataLogic engine with all custom operators registered via
/// the `Operator` trait for unified evaluation.
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

    // Use datalogic-rs with custom operators registered
    let logic = create_evaluator();
    match logic.evaluate_json(&rule_str, &data_str) {
        Ok(result) => EvaluationResponse::success(result),
        Err(e) => EvaluationResponse::error(format!("{}", e)),
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

/// Updates the feature flag state with a new configuration.
///
/// This function parses the provided JSON configuration and stores it in
/// thread-local storage for later evaluation.
///
/// # Arguments
/// * `config_ptr` - Pointer to the JSON configuration string in WASM memory
/// * `config_len` - Length of the JSON configuration string
///
/// # Returns
/// A packed u64 containing the pointer (upper 32 bits) and length (lower 32 bits)
/// of the response JSON string. The response indicates success or failure.
///
/// # Response Format
/// ```json
/// {
///   "success": true|false,
///   "error": null|"error message"
/// }
/// ```
///
/// # Safety
/// The caller must ensure:
/// - `config_ptr` points to valid memory
/// - The memory region is valid UTF-8
/// - The caller will free the returned memory using `dealloc`
#[no_mangle]
pub extern "C" fn update_state(config_ptr: *const u8, config_len: u32) -> u64 {
    let response = update_state_internal(config_ptr, config_len);
    string_to_memory(&response)
}

/// Internal implementation of update_state.
fn update_state_internal(config_ptr: *const u8, config_len: u32) -> String {
    // SAFETY: The caller guarantees valid memory regions
    let config_str = match unsafe { string_from_memory(config_ptr, config_len) } {
        Ok(s) => s,
        Err(e) => {
            return serde_json::json!({
                "success": false,
                "error": format!("Failed to read configuration: {}", e)
            })
            .to_string()
        }
    };

    // Parse and store the configuration using the storage module
    match update_flag_state(&config_str) {
        Ok(()) => serde_json::json!({
            "success": true,
            "error": null
        })
        .to_string(),
        Err(e) => serde_json::json!({
            "success": false,
            "error": e
        })
        .to_string(),
    }
}

/// Evaluates a feature flag against the provided context.
///
/// This function retrieves a flag from the previously stored state (set via `update_state`)
/// and evaluates it against the provided context.
///
/// # Arguments
/// * `flag_key_ptr` - Pointer to the flag key string in WASM memory
/// * `flag_key_len` - Length of the flag key string
/// * `context_ptr` - Pointer to the evaluation context JSON string in WASM memory
/// * `context_len` - Length of the evaluation context JSON string
///
/// # Returns
/// A packed u64 containing the pointer (upper 32 bits) and length (lower 32 bits)
/// of the EvaluationResult JSON string.
///
/// # Response Format
/// The response matches the flagd provider specification:
/// ```json
/// {
///   "value": <resolved_value>,
///   "variant": "variant_name",
///   "reason": "STATIC"|"DEFAULT"|"TARGETING_MATCH"|"DISABLED"|"ERROR"|"FLAG_NOT_FOUND",
///   "errorCode": "FLAG_NOT_FOUND"|"PARSE_ERROR"|"TYPE_MISMATCH"|"GENERAL",
///   "errorMessage": "error description"
/// }
/// ```
///
/// # Safety
/// The caller must ensure:
/// - `flag_key_ptr` and `context_ptr` point to valid memory
/// - The memory regions are valid UTF-8
/// - The caller will free the returned memory using `dealloc`
#[no_mangle]
pub extern "C" fn evaluate(
    flag_key_ptr: *const u8,
    flag_key_len: u32,
    context_ptr: *const u8,
    context_len: u32,
) -> u64 {
    let result = evaluate_internal(flag_key_ptr, flag_key_len, context_ptr, context_len);
    string_to_memory(&result.to_json_string())
}

/// Internal implementation of evaluate.
fn evaluate_internal(
    flag_key_ptr: *const u8,
    flag_key_len: u32,
    context_ptr: *const u8,
    context_len: u32,
) -> EvaluationResult {
    // SAFETY: The caller guarantees valid memory regions
    let flag_key = match unsafe { string_from_memory(flag_key_ptr, flag_key_len) } {
        Ok(s) => s,
        Err(e) => {
            return EvaluationResult::error(
                ErrorCode::ParseError,
                format!("Failed to read flag key: {}", e),
            )
        }
    };

    let context_str = match unsafe { string_from_memory(context_ptr, context_len) } {
        Ok(s) => s,
        Err(e) => {
            return EvaluationResult::error(
                ErrorCode::ParseError,
                format!("Failed to read context: {}", e),
            )
        }
    };

    // Parse the context JSON
    let context: Value = match serde_json::from_str(&context_str) {
        Ok(v) => v,
        Err(e) => {
            return EvaluationResult::error(
                ErrorCode::ParseError,
                format!("Failed to parse context JSON: {}", e),
            )
        }
    };

    // Retrieve the flag from state
    let flag_state = match get_flag_state() {
        Some(state) => state,
        None => {
            return EvaluationResult::error(
                ErrorCode::General,
                "Flag state not initialized. Call update_state first.",
            )
        }
    };

    let flag = match flag_state.flags.get(&flag_key) {
        Some(f) => f.clone(),
        None => return EvaluationResult::flag_not_found(&flag_key),
    };

    // Evaluate the flag
    evaluate_flag(&flag, &context)
}

/// Evaluates a boolean feature flag against the provided context.
///
/// This function retrieves a flag from the previously stored state (set via `update_state`)
/// and evaluates it as a boolean flag. If the resolved value is not a boolean, it returns
/// a TYPE_MISMATCH error.
///
/// # Arguments
/// * `flag_key_ptr` - Pointer to the flag key string in WASM memory
/// * `flag_key_len` - Length of the flag key string
/// * `context_ptr` - Pointer to the evaluation context JSON string in WASM memory
/// * `context_len` - Length of the evaluation context JSON string
///
/// # Returns
/// A packed u64 containing the pointer (upper 32 bits) and length (lower 32 bits)
/// of the EvaluationResult JSON string.
///
/// # Safety
/// The caller must ensure:
/// - `flag_key_ptr` and `context_ptr` point to valid memory
/// - The memory regions are valid UTF-8
/// - The caller will free the returned memory using `dealloc`
#[no_mangle]
pub extern "C" fn evaluate_boolean(
    flag_key_ptr: *const u8,
    flag_key_len: u32,
    context_ptr: *const u8,
    context_len: u32,
) -> u64 {
    let result = evaluate_typed_internal(
        flag_key_ptr,
        flag_key_len,
        context_ptr,
        context_len,
        evaluate_bool_flag,
    );
    string_to_memory(&result.to_json_string())
}

/// Evaluates a string feature flag against the provided context.
///
/// This function retrieves a flag from the previously stored state (set via `update_state`)
/// and evaluates it as a string flag. If the resolved value is not a string, it returns
/// a TYPE_MISMATCH error.
///
/// # Arguments
/// * `flag_key_ptr` - Pointer to the flag key string in WASM memory
/// * `flag_key_len` - Length of the flag key string
/// * `context_ptr` - Pointer to the evaluation context JSON string in WASM memory
/// * `context_len` - Length of the evaluation context JSON string
///
/// # Returns
/// A packed u64 containing the pointer (upper 32 bits) and length (lower 32 bits)
/// of the EvaluationResult JSON string.
///
/// # Safety
/// The caller must ensure:
/// - `flag_key_ptr` and `context_ptr` point to valid memory
/// - The memory regions are valid UTF-8
/// - The caller will free the returned memory using `dealloc`
#[no_mangle]
pub extern "C" fn evaluate_string(
    flag_key_ptr: *const u8,
    flag_key_len: u32,
    context_ptr: *const u8,
    context_len: u32,
) -> u64 {
    let result = evaluate_typed_internal(
        flag_key_ptr,
        flag_key_len,
        context_ptr,
        context_len,
        evaluate_string_flag,
    );
    string_to_memory(&result.to_json_string())
}

/// Evaluates an integer feature flag against the provided context.
///
/// This function retrieves a flag from the previously stored state (set via `update_state`)
/// and evaluates it as an integer flag. If the resolved value is not an integer, it returns
/// a TYPE_MISMATCH error.
///
/// # Arguments
/// * `flag_key_ptr` - Pointer to the flag key string in WASM memory
/// * `flag_key_len` - Length of the flag key string
/// * `context_ptr` - Pointer to the evaluation context JSON string in WASM memory
/// * `context_len` - Length of the evaluation context JSON string
///
/// # Returns
/// A packed u64 containing the pointer (upper 32 bits) and length (lower 32 bits)
/// of the EvaluationResult JSON string.
///
/// # Safety
/// The caller must ensure:
/// - `flag_key_ptr` and `context_ptr` point to valid memory
/// - The memory regions are valid UTF-8
/// - The caller will free the returned memory using `dealloc`
#[no_mangle]
pub extern "C" fn evaluate_integer(
    flag_key_ptr: *const u8,
    flag_key_len: u32,
    context_ptr: *const u8,
    context_len: u32,
) -> u64 {
    let result = evaluate_typed_internal(
        flag_key_ptr,
        flag_key_len,
        context_ptr,
        context_len,
        evaluate_int_flag,
    );
    string_to_memory(&result.to_json_string())
}

/// Evaluates a float feature flag against the provided context.
///
/// This function retrieves a flag from the previously stored state (set via `update_state`)
/// and evaluates it as a float flag. If the resolved value is not a number, it returns
/// a TYPE_MISMATCH error.
///
/// # Arguments
/// * `flag_key_ptr` - Pointer to the flag key string in WASM memory
/// * `flag_key_len` - Length of the flag key string
/// * `context_ptr` - Pointer to the evaluation context JSON string in WASM memory
/// * `context_len` - Length of the evaluation context JSON string
///
/// # Returns
/// A packed u64 containing the pointer (upper 32 bits) and length (lower 32 bits)
/// of the EvaluationResult JSON string.
///
/// # Safety
/// The caller must ensure:
/// - `flag_key_ptr` and `context_ptr` point to valid memory
/// - The memory regions are valid UTF-8
/// - The caller will free the returned memory using `dealloc`
#[no_mangle]
pub extern "C" fn evaluate_float(
    flag_key_ptr: *const u8,
    flag_key_len: u32,
    context_ptr: *const u8,
    context_len: u32,
) -> u64 {
    let result = evaluate_typed_internal(
        flag_key_ptr,
        flag_key_len,
        context_ptr,
        context_len,
        evaluate_float_flag,
    );
    string_to_memory(&result.to_json_string())
}

/// Evaluates an object feature flag against the provided context.
///
/// This function retrieves a flag from the previously stored state (set via `update_state`)
/// and evaluates it as an object flag. If the resolved value is not an object, it returns
/// a TYPE_MISMATCH error.
///
/// # Arguments
/// * `flag_key_ptr` - Pointer to the flag key string in WASM memory
/// * `flag_key_len` - Length of the flag key string
/// * `context_ptr` - Pointer to the evaluation context JSON string in WASM memory
/// * `context_len` - Length of the evaluation context JSON string
///
/// # Returns
/// A packed u64 containing the pointer (upper 32 bits) and length (lower 32 bits)
/// of the EvaluationResult JSON string.
///
/// # Safety
/// The caller must ensure:
/// - `flag_key_ptr` and `context_ptr` point to valid memory
/// - The memory regions are valid UTF-8
/// - The caller will free the returned memory using `dealloc`
#[no_mangle]
pub extern "C" fn evaluate_object(
    flag_key_ptr: *const u8,
    flag_key_len: u32,
    context_ptr: *const u8,
    context_len: u32,
) -> u64 {
    let result = evaluate_typed_internal(
        flag_key_ptr,
        flag_key_len,
        context_ptr,
        context_len,
        evaluate_object_flag,
    );
    string_to_memory(&result.to_json_string())
}

/// Internal helper function for type-specific evaluation.
///
/// This function handles the common logic for all typed evaluation functions:
/// parsing inputs, retrieving the flag, and calling the type-specific evaluator.
///
/// # Arguments
/// * `flag_key_ptr` - Pointer to the flag key string
/// * `flag_key_len` - Length of the flag key string
/// * `context_ptr` - Pointer to the context JSON string
/// * `context_len` - Length of the context JSON string
/// * `evaluator` - The type-specific evaluation function to use
fn evaluate_typed_internal<F>(
    flag_key_ptr: *const u8,
    flag_key_len: u32,
    context_ptr: *const u8,
    context_len: u32,
    evaluator: F,
) -> EvaluationResult
where
    F: Fn(&FeatureFlag, &Value) -> EvaluationResult,
{
    // SAFETY: The caller guarantees valid memory regions
    let flag_key = match unsafe { string_from_memory(flag_key_ptr, flag_key_len) } {
        Ok(s) => s,
        Err(e) => {
            return EvaluationResult::error(
                ErrorCode::ParseError,
                format!("Failed to read flag key: {}", e),
            )
        }
    };

    let context_str = match unsafe { string_from_memory(context_ptr, context_len) } {
        Ok(s) => s,
        Err(e) => {
            return EvaluationResult::error(
                ErrorCode::ParseError,
                format!("Failed to read context: {}", e),
            )
        }
    };

    // Parse the context JSON
    let context: Value = match serde_json::from_str(&context_str) {
        Ok(v) => v,
        Err(e) => {
            return EvaluationResult::error(
                ErrorCode::ParseError,
                format!("Failed to parse context JSON: {}", e),
            )
        }
    };

    // Retrieve the flag from state
    let flag_state = match get_flag_state() {
        Some(state) => state,
        None => {
            return EvaluationResult::error(
                ErrorCode::General,
                "Flag state not initialized. Call update_state first.",
            )
        }
    };

    let flag = match flag_state.flags.get(&flag_key) {
        Some(f) => f.clone(),
        None => return EvaluationResult::flag_not_found(&flag_key),
    };

    // Use the type-specific evaluator
    evaluator(&flag, &context)
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
        let error_msg = result.error.unwrap();
        // Error message from datalogic_rs uses "Parse error"
        assert!(
            error_msg.to_lowercase().contains("parse"),
            "Expected error to contain 'parse', got: {}",
            error_msg
        );
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

    // ============================================================================
    // starts_with operator tests
    // ============================================================================

    #[test]
    fn test_starts_with_operator_basic() {
        let rule = r#"{"starts_with": [{"var": "email"}, "admin@"]}"#;
        let data = r#"{"email": "admin@example.com"}"#;
        let result = evaluate_json(rule, data);
        assert!(result.success);
        assert_eq!(result.result, Some(json!(true)));
    }

    #[test]
    fn test_starts_with_operator_false() {
        let rule = r#"{"starts_with": [{"var": "email"}, "admin@"]}"#;
        let data = r#"{"email": "user@example.com"}"#;
        let result = evaluate_json(rule, data);
        assert!(result.success);
        assert_eq!(result.result, Some(json!(false)));
    }

    #[test]
    fn test_starts_with_operator_literal_values() {
        let rule = r#"{"starts_with": ["/api/users", "/api/"]}"#;
        let result = evaluate_json(rule, "{}");
        assert!(result.success);
        assert_eq!(result.result, Some(json!(true)));
    }

    #[test]
    fn test_starts_with_operator_empty_prefix() {
        let rule = r#"{"starts_with": ["hello", ""]}"#;
        let result = evaluate_json(rule, "{}");
        assert!(result.success);
        assert_eq!(result.result, Some(json!(true)));
    }

    // ============================================================================
    // ends_with operator tests
    // ============================================================================

    #[test]
    fn test_ends_with_operator_basic() {
        let rule = r#"{"ends_with": [{"var": "filename"}, ".pdf"]}"#;
        let data = r#"{"filename": "document.pdf"}"#;
        let result = evaluate_json(rule, data);
        assert!(result.success);
        assert_eq!(result.result, Some(json!(true)));
    }

    #[test]
    fn test_ends_with_operator_false() {
        let rule = r#"{"ends_with": [{"var": "filename"}, ".pdf"]}"#;
        let data = r#"{"filename": "document.docx"}"#;
        let result = evaluate_json(rule, data);
        assert!(result.success);
        assert_eq!(result.result, Some(json!(false)));
    }

    #[test]
    fn test_ends_with_operator_literal_values() {
        let rule = r#"{"ends_with": ["https://example.com", ".com"]}"#;
        let result = evaluate_json(rule, "{}");
        assert!(result.success);
        assert_eq!(result.result, Some(json!(true)));
    }

    #[test]
    fn test_ends_with_operator_empty_suffix() {
        let rule = r#"{"ends_with": ["hello", ""]}"#;
        let result = evaluate_json(rule, "{}");
        assert!(result.success);
        assert_eq!(result.result, Some(json!(true)));
    }

    // ============================================================================
    // sem_ver operator tests
    // ============================================================================

    #[test]
    fn test_sem_ver_operator_equal() {
        let rule = r#"{"sem_ver": [{"var": "version"}, "=", "1.2.3"]}"#;
        let data = r#"{"version": "1.2.3"}"#;
        let result = evaluate_json(rule, data);
        assert!(result.success);
        assert_eq!(result.result, Some(json!(true)));
    }

    #[test]
    fn test_sem_ver_operator_greater_than() {
        let rule = r#"{"sem_ver": [{"var": "version"}, ">", "1.0.0"]}"#;
        let data = r#"{"version": "2.0.0"}"#;
        let result = evaluate_json(rule, data);
        assert!(result.success);
        assert_eq!(result.result, Some(json!(true)));
    }

    #[test]
    fn test_sem_ver_operator_greater_than_or_equal() {
        let rule = r#"{"sem_ver": [{"var": "version"}, ">=", "2.0.0"]}"#;
        let data = r#"{"version": "2.0.0"}"#;
        let result = evaluate_json(rule, data);
        assert!(result.success);
        assert_eq!(result.result, Some(json!(true)));
    }

    #[test]
    fn test_sem_ver_operator_caret_range() {
        let rule = r#"{"sem_ver": [{"var": "version"}, "^", "1.2.3"]}"#;

        // Should match 1.2.5 (patch update)
        let result = evaluate_json(rule, r#"{"version": "1.2.5"}"#);
        assert!(result.success);
        assert_eq!(result.result, Some(json!(true)));

        // Should match 1.9.0 (minor update)
        let result = evaluate_json(rule, r#"{"version": "1.9.0"}"#);
        assert!(result.success);
        assert_eq!(result.result, Some(json!(true)));

        // Should not match 2.0.0 (major update)
        let result = evaluate_json(rule, r#"{"version": "2.0.0"}"#);
        assert!(result.success);
        assert_eq!(result.result, Some(json!(false)));
    }

    #[test]
    fn test_sem_ver_operator_tilde_range() {
        let rule = r#"{"sem_ver": [{"var": "version"}, "~", "1.2.3"]}"#;

        // Should match 1.2.9 (patch update)
        let result = evaluate_json(rule, r#"{"version": "1.2.9"}"#);
        assert!(result.success);
        assert_eq!(result.result, Some(json!(true)));

        // Should not match 1.3.0 (minor update)
        let result = evaluate_json(rule, r#"{"version": "1.3.0"}"#);
        assert!(result.success);
        assert_eq!(result.result, Some(json!(false)));
    }

    #[test]
    fn test_sem_ver_operator_literal_values() {
        let rule = r#"{"sem_ver": ["2.0.0", ">=", "1.0.0"]}"#;
        let result = evaluate_json(rule, "{}");
        assert!(result.success);
        assert_eq!(result.result, Some(json!(true)));
    }

    #[test]
    fn test_sem_ver_operator_invalid_version() {
        let rule = r#"{"sem_ver": [{"var": "version"}, "=", "1.2.3"]}"#;
        let data = r#"{"version": "not.a.version"}"#;
        let result = evaluate_json(rule, data);
        assert!(!result.success);
        assert!(result.error.is_some());
    }

    // ============================================================================
    // update_state and evaluate function tests
    // ============================================================================

    #[test]
    fn test_update_state_and_evaluate_bool() {
        clear_flag_state();

        let config = r#"{
            "flags": {
                "boolFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "on": true,
                        "off": false
                    },
                    "defaultVariant": "off"
                }
            }
        }"#;

        let config_bytes = config.as_bytes();
        let update_response =
            update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32);
        let update_json: Value = serde_json::from_str(&update_response).unwrap();
        assert_eq!(update_json["success"], true);

        let context = "{}";
        let context_bytes = context.as_bytes();
        let flag_key = "boolFlag";
        let flag_key_bytes = flag_key.as_bytes();

        let result = evaluate_internal(
            flag_key_bytes.as_ptr(),
            flag_key_bytes.len() as u32,
            context_bytes.as_ptr(),
            context_bytes.len() as u32,
        );

        assert_eq!(result.value, json!(false));
        assert_eq!(result.variant, Some("off".to_string()));
        assert_eq!(result.reason, ResolutionReason::Static);
    }

    #[test]
    fn test_evaluate_int_flag() {
        clear_flag_state();

        let config = r#"{
            "flags": {
                "intFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "small": 10,
                        "large": 100
                    },
                    "defaultVariant": "small"
                }
            }
        }"#;

        let config_bytes = config.as_bytes();
        update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32);

        let context = "{}";
        let context_bytes = context.as_bytes();
        let flag_key = "intFlag";
        let flag_key_bytes = flag_key.as_bytes();

        let result = evaluate_internal(
            flag_key_bytes.as_ptr(),
            flag_key_bytes.len() as u32,
            context_bytes.as_ptr(),
            context_bytes.len() as u32,
        );

        assert_eq!(result.value, json!(10));
        assert_eq!(result.variant, Some("small".to_string()));
    }

    #[test]
    fn test_evaluate_float_flag() {
        clear_flag_state();

        let config = r#"{
            "flags": {
                "floatFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "low": 1.5,
                        "high": 9.99
                    },
                    "defaultVariant": "low"
                }
            }
        }"#;

        let config_bytes = config.as_bytes();
        update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32);

        let context = "{}";
        let context_bytes = context.as_bytes();
        let flag_key = "floatFlag";
        let flag_key_bytes = flag_key.as_bytes();

        let result = evaluate_internal(
            flag_key_bytes.as_ptr(),
            flag_key_bytes.len() as u32,
            context_bytes.as_ptr(),
            context_bytes.len() as u32,
        );

        assert_eq!(result.value, json!(1.5));
        assert_eq!(result.variant, Some("low".to_string()));
    }

    #[test]
    fn test_evaluate_string_flag() {
        clear_flag_state();

        let config = r#"{
            "flags": {
                "stringFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "red": "crimson",
                        "blue": "azure"
                    },
                    "defaultVariant": "red"
                }
            }
        }"#;

        let config_bytes = config.as_bytes();
        update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32);

        let context = "{}";
        let context_bytes = context.as_bytes();
        let flag_key = "stringFlag";
        let flag_key_bytes = flag_key.as_bytes();

        let result = evaluate_internal(
            flag_key_bytes.as_ptr(),
            flag_key_bytes.len() as u32,
            context_bytes.as_ptr(),
            context_bytes.len() as u32,
        );

        assert_eq!(result.value, json!("crimson"));
        assert_eq!(result.variant, Some("red".to_string()));
    }

    #[test]
    fn test_evaluate_object_flag() {
        clear_flag_state();

        let config = r#"{
            "flags": {
                "objectFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "config1": {"timeout": 30, "retries": 3},
                        "config2": {"timeout": 60, "retries": 5}
                    },
                    "defaultVariant": "config1"
                }
            }
        }"#;

        let config_bytes = config.as_bytes();
        update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32);

        let context = "{}";
        let context_bytes = context.as_bytes();
        let flag_key = "objectFlag";
        let flag_key_bytes = flag_key.as_bytes();

        let result = evaluate_internal(
            flag_key_bytes.as_ptr(),
            flag_key_bytes.len() as u32,
            context_bytes.as_ptr(),
            context_bytes.len() as u32,
        );

        assert_eq!(result.value, json!({"timeout": 30, "retries": 3}));
        assert_eq!(result.variant, Some("config1".to_string()));
    }

    #[test]
    fn test_evaluate_with_targeting() {
        clear_flag_state();

        let config = r#"{
            "flags": {
                "targetedFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "on": true,
                        "off": false
                    },
                    "defaultVariant": "off",
                    "targeting": {
                        "if": [
                            {"==": [{"var": "email"}, "admin@example.com"]},
                            "on",
                            "off"
                        ]
                    }
                }
            }
        }"#;

        let config_bytes = config.as_bytes();
        update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32);

        // Test matching context
        let context = r#"{"email": "admin@example.com"}"#;
        let context_bytes = context.as_bytes();
        let flag_key = "targetedFlag";
        let flag_key_bytes = flag_key.as_bytes();

        let result = evaluate_internal(
            flag_key_bytes.as_ptr(),
            flag_key_bytes.len() as u32,
            context_bytes.as_ptr(),
            context_bytes.len() as u32,
        );

        assert_eq!(result.value, json!(true));
        assert_eq!(result.variant, Some("on".to_string()));
        assert_eq!(result.reason, ResolutionReason::TargetingMatch);

        // Test non-matching context
        let context = r#"{"email": "user@example.com"}"#;
        let context_bytes = context.as_bytes();

        let result = evaluate_internal(
            flag_key_bytes.as_ptr(),
            flag_key_bytes.len() as u32,
            context_bytes.as_ptr(),
            context_bytes.len() as u32,
        );

        assert_eq!(result.value, json!(false));
        assert_eq!(result.variant, Some("off".to_string()));
        assert_eq!(result.reason, ResolutionReason::TargetingMatch);
    }

    #[test]
    fn test_evaluate_disabled_flag() {
        clear_flag_state();

        let config = r#"{
            "flags": {
                "disabledFlag": {
                    "state": "DISABLED",
                    "variants": {
                        "on": true,
                        "off": false
                    },
                    "defaultVariant": "off"
                }
            }
        }"#;

        let config_bytes = config.as_bytes();
        update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32);

        let context = "{}";
        let context_bytes = context.as_bytes();
        let flag_key = "disabledFlag";
        let flag_key_bytes = flag_key.as_bytes();

        let result = evaluate_internal(
            flag_key_bytes.as_ptr(),
            flag_key_bytes.len() as u32,
            context_bytes.as_ptr(),
            context_bytes.len() as u32,
        );

        assert_eq!(result.value, json!(false));
        assert_eq!(result.reason, ResolutionReason::Disabled);
    }

    #[test]
    fn test_evaluate_flag_not_found() {
        clear_flag_state();

        let config = r#"{
            "flags": {
                "existingFlag": {
                    "state": "ENABLED",
                    "variants": {"on": true},
                    "defaultVariant": "on"
                }
            }
        }"#;

        let config_bytes = config.as_bytes();
        update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32);

        let context = "{}";
        let context_bytes = context.as_bytes();
        let flag_key = "nonexistentFlag";
        let flag_key_bytes = flag_key.as_bytes();

        let result = evaluate_internal(
            flag_key_bytes.as_ptr(),
            flag_key_bytes.len() as u32,
            context_bytes.as_ptr(),
            context_bytes.len() as u32,
        );

        assert_eq!(result.reason, ResolutionReason::FlagNotFound);
        assert_eq!(result.error_code, Some(ErrorCode::FlagNotFound));
        assert!(result.error_message.is_some());
    }

    #[test]
    fn test_update_state_invalid_json() {
        clear_flag_state();

        let config = "not valid json";
        let config_bytes = config.as_bytes();
        let response = update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32);
        let json: Value = serde_json::from_str(&response).unwrap();

        assert_eq!(json["success"], false);
        assert!(json["error"].is_string());
    }

    #[test]
    fn test_evaluate_invalid_context_json() {
        clear_flag_state();

        let config = r#"{
            "flags": {
                "testFlag": {
                    "state": "ENABLED",
                    "variants": {"on": true},
                    "defaultVariant": "on"
                }
            }
        }"#;

        let config_bytes = config.as_bytes();
        update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32);

        let context = "not valid json";
        let context_bytes = context.as_bytes();
        let flag_key = "testFlag";
        let flag_key_bytes = flag_key.as_bytes();

        let result = evaluate_internal(
            flag_key_bytes.as_ptr(),
            flag_key_bytes.len() as u32,
            context_bytes.as_ptr(),
            context_bytes.len() as u32,
        );

        assert_eq!(result.reason, ResolutionReason::Error);
        assert_eq!(result.error_code, Some(ErrorCode::ParseError));
    }

    #[test]
    fn test_evaluate_with_fractional_targeting() {
        clear_flag_state();

        let config = r#"{
            "flags": {
                "abTestFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "control": "control-experience",
                        "treatment": "treatment-experience"
                    },
                    "defaultVariant": "control",
                    "targeting": {
                        "fractional": [
                            {"var": "userId"},
                            ["control", 50, "treatment", 50]
                        ]
                    }
                }
            }
        }"#;

        let config_bytes = config.as_bytes();
        update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32);

        let context = r#"{"userId": "user-123"}"#;
        let context_bytes = context.as_bytes();
        let flag_key = "abTestFlag";
        let flag_key_bytes = flag_key.as_bytes();

        let result = evaluate_internal(
            flag_key_bytes.as_ptr(),
            flag_key_bytes.len() as u32,
            context_bytes.as_ptr(),
            context_bytes.len() as u32,
        );

        // Result should be one of the variants
        assert!(
            result.value == json!("control-experience")
                || result.value == json!("treatment-experience")
        );
        assert_eq!(result.reason, ResolutionReason::TargetingMatch);
    }

    #[test]
    fn test_evaluation_result_serialization() {
        let result = EvaluationResult::static_result(json!(42), "variant1".to_string());
        let json_str = result.to_json_string();

        let parsed: Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed["value"], 42);
        assert_eq!(parsed["variant"], "variant1");
        assert_eq!(parsed["reason"], "STATIC");
    }

    // ============================================================================
    // Edge case tests: missing targeting key, unknown variant, malformed expressions
    // ============================================================================

    #[test]
    fn test_edge_case_missing_targeting_key() {
        clear_flag_state();

        // Flag that uses targetingKey for fractional bucketing
        let config = r#"{
            "flags": {
                "testFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "a": "variant-a",
                        "b": "variant-b"
                    },
                    "defaultVariant": "a",
                    "targeting": {
                        "fractional": [
                            {"var": "targetingKey"},
                            ["a", 50, "b", 50]
                        ]
                    }
                }
            }
        }"#;

        let config_bytes = config.as_bytes();
        update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32);

        // Context without targetingKey - should use empty string as default
        let context = r#"{}"#;
        let context_bytes = context.as_bytes();
        let flag_key = "testFlag";
        let flag_key_bytes = flag_key.as_bytes();

        let result = evaluate_internal(
            flag_key_bytes.as_ptr(),
            flag_key_bytes.len() as u32,
            context_bytes.as_ptr(),
            context_bytes.len() as u32,
        );

        // Should succeed with one of the variants (using empty string as key)
        assert_eq!(result.reason, ResolutionReason::TargetingMatch);
        assert!(
            result.value == json!("variant-a") || result.value == json!("variant-b"),
            "Expected variant-a or variant-b, got: {:?}",
            result.value
        );
    }

    #[test]
    fn test_edge_case_unknown_variant_from_targeting() {
        clear_flag_state();

        // Targeting rule returns a variant name that doesn't exist
        let config = r#"{
            "flags": {
                "testFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "on": true,
                        "off": false
                    },
                    "defaultVariant": "off",
                    "targeting": {
                        "if": [
                            {"==": [{"var": "user"}, "admin"]},
                            "unknown_variant",
                            "off"
                        ]
                    }
                }
            }
        }"#;

        let config_bytes = config.as_bytes();
        update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32);

        let context = r#"{"user": "admin"}"#;
        let context_bytes = context.as_bytes();
        let flag_key = "testFlag";
        let flag_key_bytes = flag_key.as_bytes();

        let result = evaluate_internal(
            flag_key_bytes.as_ptr(),
            flag_key_bytes.len() as u32,
            context_bytes.as_ptr(),
            context_bytes.len() as u32,
        );

        // Should fall back to default variant when unknown variant is returned
        assert_eq!(result.reason, ResolutionReason::Default);
        assert_eq!(result.value, json!(false));
        assert_eq!(result.variant, Some("off".to_string()));
    }

    #[test]
    fn test_edge_case_malformed_targeting_expression() {
        clear_flag_state();

        // Invalid JSON Logic expression (missing closing bracket)
        let config = r#"{
            "flags": {
                "testFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "on": true,
                        "off": false
                    },
                    "defaultVariant": "off",
                    "targeting": {
                        "invalid_operator": [1, 2, 3]
                    }
                }
            }
        }"#;

        let config_bytes = config.as_bytes();
        update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32);

        let context = r#"{}"#;
        let context_bytes = context.as_bytes();
        let flag_key = "testFlag";
        let flag_key_bytes = flag_key.as_bytes();

        let result = evaluate_internal(
            flag_key_bytes.as_ptr(),
            flag_key_bytes.len() as u32,
            context_bytes.as_ptr(),
            context_bytes.len() as u32,
        );

        // Should return error for malformed/unknown operator
        assert_eq!(result.reason, ResolutionReason::Error);
        assert_eq!(result.error_code, Some(ErrorCode::ParseError));
        assert!(result.error_message.is_some());
    }

    #[test]
    fn test_edge_case_fractional_with_targetingkey_context() {
        clear_flag_state();

        // Use targetingKey for consistent bucketing
        let config = r#"{
            "flags": {
                "featureRollout": {
                    "state": "ENABLED",
                    "variants": {
                        "enabled": true,
                        "disabled": false
                    },
                    "defaultVariant": "disabled",
                    "targeting": {
                        "fractional": [
                            {"var": "targetingKey"},
                            ["enabled", 10, "disabled", 90]
                        ]
                    }
                }
            }
        }"#;

        let config_bytes = config.as_bytes();
        update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32);

        // Test with explicit targetingKey
        let context1 = r#"{"targetingKey": "user-001"}"#;
        let context_bytes1 = context1.as_bytes();
        let flag_key = "featureRollout";
        let flag_key_bytes = flag_key.as_bytes();

        let result1 = evaluate_internal(
            flag_key_bytes.as_ptr(),
            flag_key_bytes.len() as u32,
            context_bytes1.as_ptr(),
            context_bytes1.len() as u32,
        );

        // Same targetingKey should give same result (consistency)
        let result2 = evaluate_internal(
            flag_key_bytes.as_ptr(),
            flag_key_bytes.len() as u32,
            context_bytes1.as_ptr(),
            context_bytes1.len() as u32,
        );

        assert_eq!(result1.value, result2.value);
        assert_eq!(result1.variant, result2.variant);
        assert_eq!(result1.reason, ResolutionReason::TargetingMatch);
    }

    #[test]
    fn test_edge_case_targeting_with_flag_key_reference() {
        clear_flag_state();

        // Targeting rule that uses the flagKey field
        let config = r#"{
            "flags": {
                "debugFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "on": true,
                        "off": false
                    },
                    "defaultVariant": "off",
                    "targeting": {
                        "if": [
                            {"==": [{"var": "flagKey"}, "debugFlag"]},
                            "on",
                            "off"
                        ]
                    }
                }
            }
        }"#;

        let config_bytes = config.as_bytes();
        update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32);

        let context = r#"{}"#;
        let context_bytes = context.as_bytes();
        let flag_key = "debugFlag";
        let flag_key_bytes = flag_key.as_bytes();

        let result = evaluate_internal(
            flag_key_bytes.as_ptr(),
            flag_key_bytes.len() as u32,
            context_bytes.as_ptr(),
            context_bytes.len() as u32,
        );

        // Should match because flagKey is enriched in context
        assert_eq!(result.value, json!(true));
        assert_eq!(result.variant, Some("on".to_string()));
        assert_eq!(result.reason, ResolutionReason::TargetingMatch);
    }

    #[test]
    fn test_edge_case_complex_targeting_with_all_operators() {
        clear_flag_state();

        // Complex rule using multiple custom operators
        let config = r#"{
            "flags": {
                "complexFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "premium": "premium-tier",
                        "standard": "standard-tier",
                        "basic": "basic-tier"
                    },
                    "defaultVariant": "basic",
                    "targeting": {
                        "if": [
                            {"starts_with": [{"var": "email"}, "admin@"]},
                            "premium",
                            {
                                "if": [
                                    {"sem_ver": [{"var": "appVersion"}, ">=", "2.0.0"]},
                                    "standard",
                                    "basic"
                                ]
                            }
                        ]
                    }
                }
            }
        }"#;

        let config_bytes = config.as_bytes();
        update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32);

        // Test admin email - should get premium
        let context1 = r#"{"email": "admin@example.com", "appVersion": "1.0.0"}"#;
        let context_bytes1 = context1.as_bytes();
        let flag_key = "complexFlag";
        let flag_key_bytes = flag_key.as_bytes();

        let result1 = evaluate_internal(
            flag_key_bytes.as_ptr(),
            flag_key_bytes.len() as u32,
            context_bytes1.as_ptr(),
            context_bytes1.len() as u32,
        );

        assert_eq!(result1.value, json!("premium-tier"));
        assert_eq!(result1.variant, Some("premium".to_string()));

        // Test non-admin with new version - should get standard
        let context2 = r#"{"email": "user@example.com", "appVersion": "2.1.0"}"#;
        let context_bytes2 = context2.as_bytes();

        let result2 = evaluate_internal(
            flag_key_bytes.as_ptr(),
            flag_key_bytes.len() as u32,
            context_bytes2.as_ptr(),
            context_bytes2.len() as u32,
        );

        assert_eq!(result2.value, json!("standard-tier"));
        assert_eq!(result2.variant, Some("standard".to_string()));

        // Test non-admin with old version - should get basic
        let context3 = r#"{"email": "user@example.com", "appVersion": "1.5.0"}"#;
        let context_bytes3 = context3.as_bytes();

        let result3 = evaluate_internal(
            flag_key_bytes.as_ptr(),
            flag_key_bytes.len() as u32,
            context_bytes3.as_ptr(),
            context_bytes3.len() as u32,
        );

        assert_eq!(result3.value, json!("basic-tier"));
        assert_eq!(result3.variant, Some("basic".to_string()));
    }

    // ============================================================================
    // Type-specific WASM evaluation tests
    // ============================================================================

    fn evaluate_boolean_internal(flag_key: &str, context: &str) -> EvaluationResult {
        let flag_key_bytes = flag_key.as_bytes();
        let context_bytes = context.as_bytes();
        evaluate_typed_internal(
            flag_key_bytes.as_ptr(),
            flag_key_bytes.len() as u32,
            context_bytes.as_ptr(),
            context_bytes.len() as u32,
            evaluate_bool_flag,
        )
    }

    fn evaluate_string_internal(flag_key: &str, context: &str) -> EvaluationResult {
        let flag_key_bytes = flag_key.as_bytes();
        let context_bytes = context.as_bytes();
        evaluate_typed_internal(
            flag_key_bytes.as_ptr(),
            flag_key_bytes.len() as u32,
            context_bytes.as_ptr(),
            context_bytes.len() as u32,
            evaluate_string_flag,
        )
    }

    fn evaluate_integer_internal(flag_key: &str, context: &str) -> EvaluationResult {
        let flag_key_bytes = flag_key.as_bytes();
        let context_bytes = context.as_bytes();
        evaluate_typed_internal(
            flag_key_bytes.as_ptr(),
            flag_key_bytes.len() as u32,
            context_bytes.as_ptr(),
            context_bytes.len() as u32,
            evaluate_int_flag,
        )
    }

    fn evaluate_float_internal(flag_key: &str, context: &str) -> EvaluationResult {
        let flag_key_bytes = flag_key.as_bytes();
        let context_bytes = context.as_bytes();
        evaluate_typed_internal(
            flag_key_bytes.as_ptr(),
            flag_key_bytes.len() as u32,
            context_bytes.as_ptr(),
            context_bytes.len() as u32,
            evaluate_float_flag,
        )
    }

    fn evaluate_object_internal(flag_key: &str, context: &str) -> EvaluationResult {
        let flag_key_bytes = flag_key.as_bytes();
        let context_bytes = context.as_bytes();
        evaluate_typed_internal(
            flag_key_bytes.as_ptr(),
            flag_key_bytes.len() as u32,
            context_bytes.as_ptr(),
            context_bytes.len() as u32,
            evaluate_object_flag,
        )
    }

    #[test]
    fn test_evaluate_boolean_success() {
        clear_flag_state();

        let config = r#"{
            "flags": {
                "boolFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "on": true,
                        "off": false
                    },
                    "defaultVariant": "on"
                }
            }
        }"#;

        let config_bytes = config.as_bytes();
        update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32);

        let result = evaluate_boolean_internal("boolFlag", "{}");
        assert_eq!(result.value, json!(true));
        assert_eq!(result.reason, ResolutionReason::Static);
        assert!(result.error_code.is_none());
    }

    #[test]
    fn test_evaluate_boolean_type_mismatch() {
        clear_flag_state();

        let config = r#"{
            "flags": {
                "stringFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "on": "yes",
                        "off": "no"
                    },
                    "defaultVariant": "on"
                }
            }
        }"#;

        let config_bytes = config.as_bytes();
        update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32);

        let result = evaluate_boolean_internal("stringFlag", "{}");
        assert_eq!(result.reason, ResolutionReason::Error);
        assert_eq!(result.error_code, Some(ErrorCode::TypeMismatch));
        assert!(result.error_message.unwrap().contains("Expected boolean"));
    }

    #[test]
    fn test_evaluate_string_success() {
        clear_flag_state();

        let config = r#"{
            "flags": {
                "stringFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "red": "crimson",
                        "blue": "azure"
                    },
                    "defaultVariant": "red"
                }
            }
        }"#;

        let config_bytes = config.as_bytes();
        update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32);

        let result = evaluate_string_internal("stringFlag", "{}");
        assert_eq!(result.value, json!("crimson"));
        assert_eq!(result.reason, ResolutionReason::Static);
        assert!(result.error_code.is_none());
    }

    #[test]
    fn test_evaluate_string_type_mismatch() {
        clear_flag_state();

        let config = r#"{
            "flags": {
                "intFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "small": 10,
                        "large": 100
                    },
                    "defaultVariant": "small"
                }
            }
        }"#;

        let config_bytes = config.as_bytes();
        update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32);

        let result = evaluate_string_internal("intFlag", "{}");
        assert_eq!(result.reason, ResolutionReason::Error);
        assert_eq!(result.error_code, Some(ErrorCode::TypeMismatch));
        assert!(result.error_message.unwrap().contains("Expected string"));
    }

    #[test]
    fn test_evaluate_integer_success() {
        clear_flag_state();

        let config = r#"{
            "flags": {
                "intFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "small": 10,
                        "large": 100
                    },
                    "defaultVariant": "small"
                }
            }
        }"#;

        let config_bytes = config.as_bytes();
        update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32);

        let result = evaluate_integer_internal("intFlag", "{}");
        assert_eq!(result.value, json!(10));
        assert_eq!(result.reason, ResolutionReason::Static);
        assert!(result.error_code.is_none());
    }

    #[test]
    fn test_evaluate_integer_type_mismatch_float() {
        clear_flag_state();

        let config = r#"{
            "flags": {
                "floatFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "low": 1.5,
                        "high": 9.99
                    },
                    "defaultVariant": "low"
                }
            }
        }"#;

        let config_bytes = config.as_bytes();
        update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32);

        let result = evaluate_integer_internal("floatFlag", "{}");
        assert_eq!(result.reason, ResolutionReason::Error);
        assert_eq!(result.error_code, Some(ErrorCode::TypeMismatch));
        assert!(result.error_message.unwrap().contains("Expected integer"));
    }

    #[test]
    fn test_evaluate_float_success() {
        clear_flag_state();

        let config = r#"{
            "flags": {
                "floatFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "low": 1.5,
                        "high": 9.99
                    },
                    "defaultVariant": "low"
                }
            }
        }"#;

        let config_bytes = config.as_bytes();
        update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32);

        let result = evaluate_float_internal("floatFlag", "{}");
        assert_eq!(result.value, json!(1.5));
        assert_eq!(result.reason, ResolutionReason::Static);
        assert!(result.error_code.is_none());
    }

    #[test]
    fn test_evaluate_float_accepts_integer() {
        clear_flag_state();

        let config = r#"{
            "flags": {
                "intFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "small": 10,
                        "large": 100
                    },
                    "defaultVariant": "small"
                }
            }
        }"#;

        let config_bytes = config.as_bytes();
        update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32);

        let result = evaluate_float_internal("intFlag", "{}");
        assert_eq!(result.value, json!(10));
        assert_eq!(result.reason, ResolutionReason::Static);
        assert!(result.error_code.is_none());
    }

    #[test]
    fn test_evaluate_float_type_mismatch() {
        clear_flag_state();

        let config = r#"{
            "flags": {
                "stringFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "red": "crimson"
                    },
                    "defaultVariant": "red"
                }
            }
        }"#;

        let config_bytes = config.as_bytes();
        update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32);

        let result = evaluate_float_internal("stringFlag", "{}");
        assert_eq!(result.reason, ResolutionReason::Error);
        assert_eq!(result.error_code, Some(ErrorCode::TypeMismatch));
        assert!(result.error_message.unwrap().contains("Expected float"));
    }

    #[test]
    fn test_evaluate_object_success() {
        clear_flag_state();

        let config = r#"{
            "flags": {
                "objectFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "config1": {"timeout": 30, "retries": 3},
                        "config2": {"timeout": 60, "retries": 5}
                    },
                    "defaultVariant": "config1"
                }
            }
        }"#;

        let config_bytes = config.as_bytes();
        update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32);

        let result = evaluate_object_internal("objectFlag", "{}");
        assert_eq!(result.value, json!({"timeout": 30, "retries": 3}));
        assert_eq!(result.reason, ResolutionReason::Static);
        assert!(result.error_code.is_none());
    }

    #[test]
    fn test_evaluate_object_type_mismatch() {
        clear_flag_state();

        let config = r#"{
            "flags": {
                "stringFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "red": "crimson"
                    },
                    "defaultVariant": "red"
                }
            }
        }"#;

        let config_bytes = config.as_bytes();
        update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32);

        let result = evaluate_object_internal("stringFlag", "{}");
        assert_eq!(result.reason, ResolutionReason::Error);
        assert_eq!(result.error_code, Some(ErrorCode::TypeMismatch));
        assert!(result.error_message.unwrap().contains("Expected object"));
    }

    #[test]
    fn test_all_type_evaluators_flag_not_found() {
        clear_flag_state();

        let config = r#"{
            "flags": {
                "existingFlag": {
                    "state": "ENABLED",
                    "variants": {"on": true},
                    "defaultVariant": "on"
                }
            }
        }"#;

        let config_bytes = config.as_bytes();
        update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32);

        // All type-specific evaluators should return FLAG_NOT_FOUND for missing flags
        let bool_result = evaluate_boolean_internal("missingFlag", "{}");
        assert_eq!(bool_result.reason, ResolutionReason::FlagNotFound);
        assert_eq!(bool_result.error_code, Some(ErrorCode::FlagNotFound));

        let string_result = evaluate_string_internal("missingFlag", "{}");
        assert_eq!(string_result.reason, ResolutionReason::FlagNotFound);
        assert_eq!(string_result.error_code, Some(ErrorCode::FlagNotFound));

        let int_result = evaluate_integer_internal("missingFlag", "{}");
        assert_eq!(int_result.reason, ResolutionReason::FlagNotFound);
        assert_eq!(int_result.error_code, Some(ErrorCode::FlagNotFound));

        let float_result = evaluate_float_internal("missingFlag", "{}");
        assert_eq!(float_result.reason, ResolutionReason::FlagNotFound);
        assert_eq!(float_result.error_code, Some(ErrorCode::FlagNotFound));

        let object_result = evaluate_object_internal("missingFlag", "{}");
        assert_eq!(object_result.reason, ResolutionReason::FlagNotFound);
        assert_eq!(object_result.error_code, Some(ErrorCode::FlagNotFound));
    }

    #[test]
    fn test_type_evaluators_with_targeting() {
        clear_flag_state();

        let config = r#"{
            "flags": {
                "boolFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "on": true,
                        "off": false
                    },
                    "defaultVariant": "off",
                    "targeting": {
                        "if": [
                            {"==": [{"var": "email"}, "admin@example.com"]},
                            "on",
                            "off"
                        ]
                    }
                },
                "intFlag": {
                    "state": "ENABLED",
                    "variants": {
                        "small": 10,
                        "large": 100
                    },
                    "defaultVariant": "small",
                    "targeting": {
                        "if": [
                            {">": [{"var": "age"}, 18]},
                            "large",
                            "small"
                        ]
                    }
                }
            }
        }"#;

        let config_bytes = config.as_bytes();
        update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32);

        // Test boolean with targeting match
        let bool_result =
            evaluate_boolean_internal("boolFlag", r#"{"email": "admin@example.com"}"#);
        assert_eq!(bool_result.value, json!(true));
        assert_eq!(bool_result.reason, ResolutionReason::TargetingMatch);

        // Test boolean with targeting no match
        let bool_result = evaluate_boolean_internal("boolFlag", r#"{"email": "user@example.com"}"#);
        assert_eq!(bool_result.value, json!(false));
        assert_eq!(bool_result.reason, ResolutionReason::TargetingMatch);

        // Test integer with targeting
        let int_result = evaluate_integer_internal("intFlag", r#"{"age": 25}"#);
        assert_eq!(int_result.value, json!(100));
        assert_eq!(int_result.reason, ResolutionReason::TargetingMatch);
    }

    #[test]
    fn test_type_evaluators_with_disabled_flags() {
        clear_flag_state();

        let config = r#"{
            "flags": {
                "disabledBool": {
                    "state": "DISABLED",
                    "variants": {
                        "on": true,
                        "off": false
                    },
                    "defaultVariant": "off"
                },
                "disabledString": {
                    "state": "DISABLED",
                    "variants": {
                        "red": "crimson",
                        "blue": "azure"
                    },
                    "defaultVariant": "blue"
                }
            }
        }"#;

        let config_bytes = config.as_bytes();
        update_state_internal(config_bytes.as_ptr(), config_bytes.len() as u32);

        let bool_result = evaluate_boolean_internal("disabledBool", "{}");
        assert_eq!(bool_result.value, json!(false));
        assert_eq!(bool_result.reason, ResolutionReason::Disabled);

        let string_result = evaluate_string_internal("disabledString", "{}");
        assert_eq!(string_result.value, json!("azure"));
        assert_eq!(string_result.reason, ResolutionReason::Disabled);
    }
}
