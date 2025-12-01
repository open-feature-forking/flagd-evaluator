//! Integration tests for the flagd-evaluator library.
//!
//! These tests verify the complete evaluation flow including memory management,
//! JSON parsing, custom operators, and error handling.
//!
//! Note: The `evaluate_logic` function with packed pointers is designed for WASM
//! environments where pointers are 32 bits. In native 64-bit tests, we test the
//! internal logic directly.

use flagd_evaluator::{alloc, dealloc, pack_ptr_len, unpack_ptr_len, EvaluationResponse};
use serde_json::json;

/// Helper function to resolve a string value from a JSON value.
/// Handles both direct string values and variable references.
fn resolve_string_value(
    value: &serde_json::Value,
    data: &serde_json::Value,
) -> Result<String, String> {
    match value {
        serde_json::Value::String(s) => Ok(s.clone()),
        serde_json::Value::Object(obj) if obj.contains_key("var") => {
            let var_path = match obj.get("var").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => return Err("var reference must be a string".to_string()),
            };

            let mut current = data;
            for part in var_path.split('.') {
                current = match current.get(part) {
                    Some(v) => v,
                    None => return Err(format!("Variable '{}' not found", var_path)),
                };
            }

            match current {
                serde_json::Value::String(s) => Ok(s.clone()),
                serde_json::Value::Number(n) => Ok(n.to_string()),
                serde_json::Value::Null => Ok(String::new()),
                _ => Err(format!(
                    "Variable '{}' must be a string or number",
                    var_path
                )),
            }
        }
        serde_json::Value::Number(n) => Ok(n.to_string()),
        serde_json::Value::Null => Ok(String::new()),
        _ => Err("Value must be a string, number, null, or var reference".to_string()),
    }
}

/// Helper function to evaluate JSON Logic and get the response.
/// This tests the internal logic without going through the WASM pointer packing
/// which doesn't work correctly on 64-bit native systems.
fn evaluate(rule: &str, data: &str) -> EvaluationResponse {
    // Parse the response JSON that would be returned
    // We simulate what the WASM boundary would do by calling the internal logic
    let rule_value: serde_json::Value = match serde_json::from_str(rule) {
        Ok(v) => v,
        Err(e) => return EvaluationResponse::error(format!("Failed to parse rule JSON: {}", e)),
    };

    let data_value: serde_json::Value = match serde_json::from_str(data) {
        Ok(v) => v,
        Err(e) => return EvaluationResponse::error(format!("Failed to parse data JSON: {}", e)),
    };

    // Check for custom fractional operator first
    if let Some(fractional_args) = rule_value.get("fractional") {
        if let Some(args_array) = fractional_args.as_array() {
            if args_array.len() >= 2 {
                // Extract bucket key
                let bucket_key = match resolve_string_value(&args_array[0], &data_value) {
                    Ok(s) => s,
                    Err(e) => return EvaluationResponse::error(e),
                };

                // Extract buckets
                if let Some(buckets) = args_array[1].as_array() {
                    match flagd_evaluator::fractional(&bucket_key, buckets) {
                        Ok(bucket_name) => {
                            return EvaluationResponse::success(serde_json::Value::String(
                                bucket_name,
                            ))
                        }
                        Err(e) => return EvaluationResponse::error(e),
                    }
                }
            }
        }
        return EvaluationResponse::error(
            "fractional operator requires an array with at least 2 elements",
        );
    }

    // Check for custom starts_with operator
    if let Some(args) = rule_value.get("starts_with") {
        if let Some(args_array) = args.as_array() {
            if args_array.len() >= 2 {
                let string_value = match resolve_string_value(&args_array[0], &data_value) {
                    Ok(s) => s,
                    Err(e) => return EvaluationResponse::error(e),
                };
                let prefix = match resolve_string_value(&args_array[1], &data_value) {
                    Ok(s) => s,
                    Err(e) => return EvaluationResponse::error(e),
                };
                return EvaluationResponse::success(serde_json::Value::Bool(
                    flagd_evaluator::starts_with(&string_value, &prefix),
                ));
            }
        }
        return EvaluationResponse::error(
            "starts_with operator requires an array with at least 2 elements",
        );
    }

    // Check for custom ends_with operator
    if let Some(args) = rule_value.get("ends_with") {
        if let Some(args_array) = args.as_array() {
            if args_array.len() >= 2 {
                let string_value = match resolve_string_value(&args_array[0], &data_value) {
                    Ok(s) => s,
                    Err(e) => return EvaluationResponse::error(e),
                };
                let suffix = match resolve_string_value(&args_array[1], &data_value) {
                    Ok(s) => s,
                    Err(e) => return EvaluationResponse::error(e),
                };
                return EvaluationResponse::success(serde_json::Value::Bool(
                    flagd_evaluator::ends_with(&string_value, &suffix),
                ));
            }
        }
        return EvaluationResponse::error(
            "ends_with operator requires an array with at least 2 elements",
        );
    }

    // Check for custom sem_ver operator
    if let Some(args) = rule_value.get("sem_ver") {
        if let Some(args_array) = args.as_array() {
            if args_array.len() >= 3 {
                let version = match resolve_string_value(&args_array[0], &data_value) {
                    Ok(s) => s,
                    Err(e) => return EvaluationResponse::error(e),
                };
                let operator = match args_array[1].as_str() {
                    Some(s) => s,
                    None => return EvaluationResponse::error("sem_ver operator must be a string"),
                };
                let target = match resolve_string_value(&args_array[2], &data_value) {
                    Ok(s) => s,
                    Err(e) => return EvaluationResponse::error(e),
                };
                match flagd_evaluator::sem_ver(&version, operator, &target) {
                    Ok(result) => {
                        return EvaluationResponse::success(serde_json::Value::Bool(result))
                    }
                    Err(e) => return EvaluationResponse::error(e),
                }
            }
        }
        return EvaluationResponse::error(
            "sem_ver operator requires an array with at least 3 elements",
        );
    }

    // Use datalogic-rs for standard JSON Logic
    let engine = datalogic_rs::DataLogic::new();
    match engine.evaluate_json(rule, data) {
        Ok(result) => EvaluationResponse::success(result),
        Err(e) => EvaluationResponse::error(format!("Evaluation error: {}", e)),
    }
}

// ============================================================================
// Basic JSON Logic Operations
// ============================================================================

#[test]
fn test_equality() {
    let response = evaluate(r#"{"==": [1, 1]}"#, "{}");
    assert!(response.success);
    assert_eq!(response.result, Some(json!(true)));

    let response = evaluate(r#"{"==": [1, 2]}"#, "{}");
    assert!(response.success);
    assert_eq!(response.result, Some(json!(false)));
}

#[test]
fn test_strict_equality() {
    let response = evaluate(r#"{"===": [1, 1]}"#, "{}");
    assert!(response.success);
    assert_eq!(response.result, Some(json!(true)));
}

#[test]
fn test_comparison_operators() {
    // Greater than
    let response = evaluate(r#"{">": [5, 3]}"#, "{}");
    assert!(response.success);
    assert_eq!(response.result, Some(json!(true)));

    // Less than
    let response = evaluate(r#"{"<": [3, 5]}"#, "{}");
    assert!(response.success);
    assert_eq!(response.result, Some(json!(true)));

    // Greater than or equal
    let response = evaluate(r#"{">=": [5, 5]}"#, "{}");
    assert!(response.success);
    assert_eq!(response.result, Some(json!(true)));

    // Less than or equal
    let response = evaluate(r#"{"<=": [5, 5]}"#, "{}");
    assert!(response.success);
    assert_eq!(response.result, Some(json!(true)));
}

#[test]
fn test_boolean_operations() {
    // AND
    let response = evaluate(r#"{"and": [true, true]}"#, "{}");
    assert!(response.success);
    assert_eq!(response.result, Some(json!(true)));

    // OR
    let response = evaluate(r#"{"or": [false, true]}"#, "{}");
    assert!(response.success);
    assert_eq!(response.result, Some(json!(true)));

    // NOT
    let response = evaluate(r#"{"!": true}"#, "{}");
    assert!(response.success);
    assert_eq!(response.result, Some(json!(false)));
}

#[test]
fn test_if_then_else() {
    let response = evaluate(r#"{"if": [true, "yes", "no"]}"#, "{}");
    assert!(response.success);
    assert_eq!(response.result, Some(json!("yes")));

    let response = evaluate(r#"{"if": [false, "yes", "no"]}"#, "{}");
    assert!(response.success);
    assert_eq!(response.result, Some(json!("no")));
}

#[test]
fn test_variable_access() {
    let response = evaluate(r#"{"var": "name"}"#, r#"{"name": "John"}"#);
    assert!(response.success);
    assert_eq!(response.result, Some(json!("John")));
}

#[test]
fn test_nested_variable_access() {
    let response = evaluate(
        r#"{"var": "user.profile.name"}"#,
        r#"{"user": {"profile": {"name": "Jane"}}}"#,
    );
    assert!(response.success);
    assert_eq!(response.result, Some(json!("Jane")));
}

#[test]
fn test_missing_variable() {
    let response = evaluate(r#"{"var": "nonexistent"}"#, "{}");
    assert!(response.success);
    assert_eq!(response.result, Some(json!(null)));
}

#[test]
fn test_default_variable_value() {
    let response = evaluate(r#"{"var": ["missing", "default"]}"#, "{}");
    assert!(response.success);
    assert_eq!(response.result, Some(json!("default")));
}

// ============================================================================
// Array Operations
// ============================================================================

#[test]
fn test_in_operator() {
    let response = evaluate(r#"{"in": ["a", ["a", "b", "c"]]}"#, "{}");
    assert!(response.success);
    assert_eq!(response.result, Some(json!(true)));

    let response = evaluate(r#"{"in": ["x", ["a", "b", "c"]]}"#, "{}");
    assert!(response.success);
    assert_eq!(response.result, Some(json!(false)));
}

#[test]
fn test_merge_operator() {
    let response = evaluate(r#"{"merge": [[1, 2], [3, 4]]}"#, "{}");
    assert!(response.success);
    assert_eq!(response.result, Some(json!([1, 2, 3, 4])));
}

// ============================================================================
// Arithmetic Operations
// ============================================================================

#[test]
fn test_arithmetic_operations() {
    // Addition
    let response = evaluate(r#"{"+": [1, 2]}"#, "{}");
    assert!(response.success);
    assert_eq!(response.result, Some(json!(3)));

    // Subtraction
    let response = evaluate(r#"{"-": [5, 3]}"#, "{}");
    assert!(response.success);
    assert_eq!(response.result, Some(json!(2)));

    // Multiplication
    let response = evaluate(r#"{"*": [3, 4]}"#, "{}");
    assert!(response.success);
    assert_eq!(response.result, Some(json!(12)));

    // Division
    let response = evaluate(r#"{"/": [10, 2]}"#, "{}");
    assert!(response.success);
    assert_eq!(response.result, Some(json!(5)));

    // Modulo
    let response = evaluate(r#"{"%": [7, 3]}"#, "{}");
    assert!(response.success);
    assert_eq!(response.result, Some(json!(1)));
}

// ============================================================================
// Custom Fractional Operator
// ============================================================================

#[test]
fn test_fractional_operator_basic() {
    let response = evaluate(
        r#"{"fractional": ["user-123", ["control", 50, "treatment", 50]]}"#,
        "{}",
    );
    assert!(response.success);
    let result = response.result.unwrap();
    assert!(result == json!("control") || result == json!("treatment"));
}

#[test]
fn test_fractional_operator_consistency() {
    let rule = r#"{"fractional": ["stable-key", ["a", 33, "b", 33, "c", 34]]}"#;

    let response1 = evaluate(rule, "{}");
    let response2 = evaluate(rule, "{}");
    let response3 = evaluate(rule, "{}");

    assert!(response1.success);
    assert!(response2.success);
    assert!(response3.success);

    // Same key should always produce same result
    assert_eq!(response1.result, response2.result);
    assert_eq!(response2.result, response3.result);
}

#[test]
fn test_fractional_operator_with_var() {
    let response = evaluate(
        r#"{"fractional": [{"var": "userId"}, ["bucket1", 50, "bucket2", 50]]}"#,
        r#"{"userId": "test-user-42"}"#,
    );
    assert!(response.success);
    let result = response.result.unwrap();
    assert!(result == json!("bucket1") || result == json!("bucket2"));
}

#[test]
fn test_fractional_operator_with_nested_var() {
    let response = evaluate(
        r#"{"fractional": [{"var": "user.id"}, ["A", 50, "B", 50]]}"#,
        r#"{"user": {"id": "nested-user-123"}}"#,
    );
    assert!(response.success);
}

#[test]
fn test_fractional_operator_single_bucket() {
    let response = evaluate(r#"{"fractional": ["any-key", ["only-option", 100]]}"#, "{}");
    assert!(response.success);
    assert_eq!(response.result, Some(json!("only-option")));
}

#[test]
fn test_fractional_operator_numeric_key() {
    let response = evaluate(r#"{"fractional": [12345, ["a", 50, "b", 50]]}"#, "{}");
    assert!(response.success);
}

#[test]
fn test_fractional_distribution() {
    // Test that distribution is roughly correct over many iterations
    let mut counts = std::collections::HashMap::new();

    for i in 0..1000 {
        let rule = format!(
            r#"{{"fractional": ["user-{}", ["small", 20, "large", 80]]}}"#,
            i
        );
        let response = evaluate(&rule, "{}");
        assert!(response.success);

        let bucket = response.result.unwrap().as_str().unwrap().to_string();
        *counts.entry(bucket).or_insert(0) += 1;
    }

    let small_count = *counts.get("small").unwrap_or(&0);
    let large_count = *counts.get("large").unwrap_or(&0);

    // Allow 10% tolerance for randomness
    assert!(small_count > 100, "small bucket too few: {}", small_count);
    assert!(small_count < 300, "small bucket too many: {}", small_count);
    assert!(large_count > 600, "large bucket too few: {}", large_count);
}

// ============================================================================
// Error Handling
// ============================================================================

#[test]
fn test_invalid_json_rule() {
    let response = evaluate("not valid json", "{}");
    assert!(!response.success);
    assert!(response.error.is_some());
    assert!(response.error.as_ref().unwrap().contains("parse"));
}

#[test]
fn test_invalid_json_data() {
    let response = evaluate(r#"{"var": "x"}"#, "not valid json");
    assert!(!response.success);
    assert!(response.error.is_some());
}

#[test]
fn test_fractional_missing_buckets() {
    let response = evaluate(r#"{"fractional": ["key"]}"#, "{}");
    assert!(!response.success);
    assert!(response.error.is_some());
}

#[test]
fn test_fractional_empty_buckets() {
    let response = evaluate(r#"{"fractional": ["key", []]}"#, "{}");
    assert!(!response.success);
    assert!(response.error.is_some());
}

#[test]
fn test_fractional_missing_var() {
    let response = evaluate(
        r#"{"fractional": [{"var": "missing"}, ["a", 50, "b", 50]]}"#,
        "{}",
    );
    assert!(!response.success);
    assert!(response.error.is_some());
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_empty_rule() {
    let response = evaluate("{}", "{}");
    assert!(response.success);
}

#[test]
fn test_empty_data() {
    let response = evaluate(r#"{"==": [1, 1]}"#, "{}");
    assert!(response.success);
}

#[test]
fn test_null_values() {
    let response = evaluate(r#"{"==": [null, null]}"#, "{}");
    assert!(response.success);
    assert_eq!(response.result, Some(json!(true)));
}

#[test]
fn test_unicode_strings() {
    let response = evaluate(r#"{"var": "greeting"}"#, r#"{"greeting": "こんにちは"}"#);
    assert!(response.success);
    assert_eq!(response.result, Some(json!("こんにちは")));
}

#[test]
fn test_large_numbers() {
    let response = evaluate(r#"{"+": [9999999999, 1]}"#, "{}");
    assert!(response.success);
}

#[test]
fn test_deeply_nested_data() {
    let data = r#"{
        "level1": {
            "level2": {
                "level3": {
                    "level4": {
                        "value": 42
                    }
                }
            }
        }
    }"#;

    let response = evaluate(r#"{"var": "level1.level2.level3.level4.value"}"#, data);
    assert!(response.success);
    assert_eq!(response.result, Some(json!(42)));
}

#[test]
fn test_complex_nested_rule() {
    let rule = r#"{
        "if": [
            {"and": [
                {">=": [{"var": "age"}, 18]},
                {"<": [{"var": "age"}, 65]}
            ]},
            "working age",
            {"if": [
                {"<": [{"var": "age"}, 18]},
                "minor",
                "senior"
            ]}
        ]
    }"#;

    let response = evaluate(rule, r#"{"age": 30}"#);
    assert!(response.success);
    assert_eq!(response.result, Some(json!("working age")));

    let response = evaluate(rule, r#"{"age": 10}"#);
    assert!(response.success);
    assert_eq!(response.result, Some(json!("minor")));

    let response = evaluate(rule, r#"{"age": 70}"#);
    assert!(response.success);
    assert_eq!(response.result, Some(json!("senior")));
}

// ============================================================================
// Memory Management
// ============================================================================

#[test]
fn test_alloc_dealloc() {
    let ptr = alloc(100);
    assert!(!ptr.is_null());
    dealloc(ptr, 100);
}

#[test]
fn test_alloc_zero_bytes() {
    let ptr = alloc(0);
    assert!(ptr.is_null());
}

#[test]
fn test_multiple_allocations() {
    let mut pointers = Vec::new();

    for size in [10, 100, 1000, 10000] {
        let ptr = alloc(size);
        assert!(!ptr.is_null());
        pointers.push((ptr, size));
    }

    for (ptr, size) in pointers {
        dealloc(ptr, size);
    }
}

#[test]
fn test_pack_unpack_ptr_len() {
    let original_ptr = 0x12345678 as *const u8;
    let original_len = 999u32;

    let packed = pack_ptr_len(original_ptr, original_len);
    let (unpacked_ptr, unpacked_len) = unpack_ptr_len(packed);

    assert_eq!(unpacked_ptr, original_ptr);
    assert_eq!(unpacked_len, original_len);
}

// ============================================================================
// Response Format
// ============================================================================

#[test]
fn test_success_response_format() {
    let response = evaluate(r#"{"==": [1, 1]}"#, "{}");

    assert!(response.success);
    assert!(response.result.is_some());
    assert!(response.error.is_none());
}

#[test]
fn test_error_response_format() {
    let response = evaluate("invalid json", "{}");

    assert!(!response.success);
    assert!(response.result.is_none());
    assert!(response.error.is_some());
}

#[test]
fn test_response_serialization() {
    use flagd_evaluator::EvaluationResponse;

    let success = EvaluationResponse::success(json!(42));
    let json_str = success.to_json_string();
    assert!(json_str.contains(r#""success":true"#));
    assert!(json_str.contains(r#""result":42"#));

    let error = EvaluationResponse::error("test error");
    let json_str = error.to_json_string();
    assert!(json_str.contains(r#""success":false"#));
    assert!(json_str.contains(r#""error":"test error""#));
}

// ============================================================================
// Custom starts_with Operator
// ============================================================================

#[test]
fn test_starts_with_operator_basic() {
    let response = evaluate(
        r#"{"starts_with": [{"var": "email"}, "admin@"]}"#,
        r#"{"email": "admin@example.com"}"#,
    );
    assert!(response.success);
    assert_eq!(response.result, Some(json!(true)));
}

#[test]
fn test_starts_with_operator_false() {
    let response = evaluate(
        r#"{"starts_with": [{"var": "email"}, "admin@"]}"#,
        r#"{"email": "user@example.com"}"#,
    );
    assert!(response.success);
    assert_eq!(response.result, Some(json!(false)));
}

#[test]
fn test_starts_with_operator_literal() {
    let response = evaluate(r#"{"starts_with": ["/api/users", "/api/"]}"#, "{}");
    assert!(response.success);
    assert_eq!(response.result, Some(json!(true)));
}

#[test]
fn test_starts_with_operator_empty_prefix() {
    let response = evaluate(r#"{"starts_with": ["hello", ""]}"#, "{}");
    assert!(response.success);
    assert_eq!(response.result, Some(json!(true)));
}

#[test]
fn test_starts_with_operator_case_sensitive() {
    let response = evaluate(r#"{"starts_with": ["/API/users", "/api/"]}"#, "{}");
    assert!(response.success);
    assert_eq!(response.result, Some(json!(false)));
}

// ============================================================================
// Custom ends_with Operator
// ============================================================================

#[test]
fn test_ends_with_operator_basic() {
    let response = evaluate(
        r#"{"ends_with": [{"var": "filename"}, ".pdf"]}"#,
        r#"{"filename": "document.pdf"}"#,
    );
    assert!(response.success);
    assert_eq!(response.result, Some(json!(true)));
}

#[test]
fn test_ends_with_operator_false() {
    let response = evaluate(
        r#"{"ends_with": [{"var": "filename"}, ".pdf"]}"#,
        r#"{"filename": "document.docx"}"#,
    );
    assert!(response.success);
    assert_eq!(response.result, Some(json!(false)));
}

#[test]
fn test_ends_with_operator_literal() {
    let response = evaluate(r#"{"ends_with": ["https://example.com", ".com"]}"#, "{}");
    assert!(response.success);
    assert_eq!(response.result, Some(json!(true)));
}

#[test]
fn test_ends_with_operator_empty_suffix() {
    let response = evaluate(r#"{"ends_with": ["hello", ""]}"#, "{}");
    assert!(response.success);
    assert_eq!(response.result, Some(json!(true)));
}

#[test]
fn test_ends_with_operator_case_sensitive() {
    let response = evaluate(r#"{"ends_with": ["example.COM", ".com"]}"#, "{}");
    assert!(response.success);
    assert_eq!(response.result, Some(json!(false)));
}

// ============================================================================
// Custom sem_ver Operator
// ============================================================================

#[test]
fn test_sem_ver_operator_equal() {
    let response = evaluate(
        r#"{"sem_ver": [{"var": "version"}, "=", "1.2.3"]}"#,
        r#"{"version": "1.2.3"}"#,
    );
    assert!(response.success);
    assert_eq!(response.result, Some(json!(true)));
}

#[test]
fn test_sem_ver_operator_not_equal() {
    let response = evaluate(
        r#"{"sem_ver": [{"var": "version"}, "!=", "1.2.3"]}"#,
        r#"{"version": "1.2.4"}"#,
    );
    assert!(response.success);
    assert_eq!(response.result, Some(json!(true)));
}

#[test]
fn test_sem_ver_operator_less_than() {
    let response = evaluate(
        r#"{"sem_ver": [{"var": "version"}, "<", "2.0.0"]}"#,
        r#"{"version": "1.5.0"}"#,
    );
    assert!(response.success);
    assert_eq!(response.result, Some(json!(true)));
}

#[test]
fn test_sem_ver_operator_less_than_or_equal() {
    let response = evaluate(
        r#"{"sem_ver": [{"var": "version"}, "<=", "2.0.0"]}"#,
        r#"{"version": "2.0.0"}"#,
    );
    assert!(response.success);
    assert_eq!(response.result, Some(json!(true)));
}

#[test]
fn test_sem_ver_operator_greater_than() {
    let response = evaluate(
        r#"{"sem_ver": [{"var": "version"}, ">", "1.0.0"]}"#,
        r#"{"version": "2.0.0"}"#,
    );
    assert!(response.success);
    assert_eq!(response.result, Some(json!(true)));
}

#[test]
fn test_sem_ver_operator_greater_than_or_equal() {
    let response = evaluate(
        r#"{"sem_ver": [{"var": "version"}, ">=", "2.0.0"]}"#,
        r#"{"version": "2.0.0"}"#,
    );
    assert!(response.success);
    assert_eq!(response.result, Some(json!(true)));
}

#[test]
fn test_sem_ver_operator_caret_range() {
    // ^1.2.3 means >=1.2.3 <2.0.0
    let response = evaluate(
        r#"{"sem_ver": [{"var": "version"}, "^", "1.2.3"]}"#,
        r#"{"version": "1.9.0"}"#,
    );
    assert!(response.success);
    assert_eq!(response.result, Some(json!(true)));

    // Should not match 2.0.0
    let response = evaluate(
        r#"{"sem_ver": [{"var": "version"}, "^", "1.2.3"]}"#,
        r#"{"version": "2.0.0"}"#,
    );
    assert!(response.success);
    assert_eq!(response.result, Some(json!(false)));
}

#[test]
fn test_sem_ver_operator_tilde_range() {
    // ~1.2.3 means >=1.2.3 <1.3.0
    let response = evaluate(
        r#"{"sem_ver": [{"var": "version"}, "~", "1.2.3"]}"#,
        r#"{"version": "1.2.9"}"#,
    );
    assert!(response.success);
    assert_eq!(response.result, Some(json!(true)));

    // Should not match 1.3.0
    let response = evaluate(
        r#"{"sem_ver": [{"var": "version"}, "~", "1.2.3"]}"#,
        r#"{"version": "1.3.0"}"#,
    );
    assert!(response.success);
    assert_eq!(response.result, Some(json!(false)));
}

#[test]
fn test_sem_ver_operator_with_prerelease() {
    // Pre-release versions are less than release versions
    let response = evaluate(r#"{"sem_ver": ["1.0.0-alpha", "<", "1.0.0"]}"#, "{}");
    assert!(response.success);
    assert_eq!(response.result, Some(json!(true)));
}

#[test]
fn test_sem_ver_operator_literal() {
    let response = evaluate(r#"{"sem_ver": ["2.0.0", ">=", "1.0.0"]}"#, "{}");
    assert!(response.success);
    assert_eq!(response.result, Some(json!(true)));
}

#[test]
fn test_sem_ver_operator_invalid_version() {
    let response = evaluate(
        r#"{"sem_ver": [{"var": "version"}, "=", "1.2.3"]}"#,
        r#"{"version": "not.a.version"}"#,
    );
    assert!(!response.success);
    assert!(response.error.is_some());
}

#[test]
fn test_sem_ver_operator_missing_parts() {
    // Missing patch should be treated as 0
    let response = evaluate(r#"{"sem_ver": ["1.2", "=", "1.2.0"]}"#, "{}");
    assert!(response.success);
    assert_eq!(response.result, Some(json!(true)));
}

// ============================================================================
// Complex Targeting Rules (combining operators)
// ============================================================================

#[test]
fn test_sem_ver_targeting_rule() {
    // A rule that uses sem_ver for version-based targeting
    let response = evaluate(
        r#"{"sem_ver": [{"var": "app.version"}, ">=", "2.0.0"]}"#,
        r#"{"app": {"version": "2.1.0"}}"#,
    );
    assert!(response.success);
    assert_eq!(response.result, Some(json!(true)));
}

#[test]
fn test_starts_with_targeting_rule() {
    // A rule that uses starts_with for email-based targeting
    let response = evaluate(
        r#"{"starts_with": [{"var": "user.email"}, "beta@"]}"#,
        r#"{"user": {"email": "beta@example.com"}}"#,
    );
    assert!(response.success);
    assert_eq!(response.result, Some(json!(true)));
}

#[test]
fn test_ends_with_targeting_rule() {
    // A rule that uses ends_with for domain-based targeting
    let response = evaluate(
        r#"{"ends_with": [{"var": "user.email"}, "@company.com"]}"#,
        r#"{"user": {"email": "john@company.com"}}"#,
    );
    assert!(response.success);
    assert_eq!(response.result, Some(json!(true)));
}
