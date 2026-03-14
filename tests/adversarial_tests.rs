//! Adversarial tests verifying that the evaluator fails safely and deterministically
//! under oversized inputs, deeply nested JSON, and deep `$ref` chains.
//!
//! Tests for WASM-boundary size limits (`MAX_CONFIG_BYTES`, `MAX_CONTEXT_BYTES`) live in
//! `src/lib.rs` (the `adversarial_wasm_tests` module) because they call internal functions
//! that bypass the packed-u64 pointer mechanism, which is incompatible with 64-bit native
//! heap addresses used in integration tests.
//!
//! These tests implement acceptance criteria from:
//! <https://github.com/open-feature/flagd-evaluator/issues/23>

use flagd_evaluator::{limits, FlagEvaluator, ValidationMode};

/// Verifies that a config with JSON nesting deeper than `MAX_JSON_DEPTH` (128) is rejected
/// by `FlagEvaluator::update_state` with a deterministic error.
///
/// The outer `{"flags": ...}` contributes 1 nesting level, so we use
/// `MAX_JSON_DEPTH` inner brackets to reach a total depth of `MAX_JSON_DEPTH + 1`.
#[test]
fn test_deeply_nested_json_config_rejected() {
    // MAX_JSON_DEPTH inner brackets + 1 for the outer `{` = MAX_JSON_DEPTH + 1 total
    let inner_brackets = limits::MAX_JSON_DEPTH;
    let inner_open = "[".repeat(inner_brackets);
    let inner_close = "]".repeat(inner_brackets);
    let config = format!(r#"{{"flags": {inner_open}{inner_close}}}"#);

    let mut evaluator = FlagEvaluator::new(ValidationMode::Permissive);
    let response = evaluator
        .update_state(&config)
        .expect("update_state should not panic");

    assert!(
        !response.success,
        "Expected failure for deeply nested JSON, but got success=true"
    );
    let error = response.error.expect("Expected an error message");
    assert!(
        error.contains("depth") || error.contains("nesting"),
        "Error should mention depth/nesting: {error}"
    );
}

/// Verifies that a config at exactly `MAX_JSON_DEPTH` nesting levels is not rejected
/// by the depth limit (schema or parse errors are acceptable; only depth errors are not).
///
/// The outer `{"flags": ...}` contributes 1 level, so we use `MAX_JSON_DEPTH - 1`
/// inner brackets to reach exactly `MAX_JSON_DEPTH` total.
#[test]
fn test_json_at_depth_limit_accepted() {
    // (MAX_JSON_DEPTH - 1) inner brackets + 1 for outer `{` = MAX_JSON_DEPTH total
    let inner_brackets = limits::MAX_JSON_DEPTH - 1;
    let inner_open = "[".repeat(inner_brackets);
    let inner_close = "]".repeat(inner_brackets);
    let config = format!(r#"{{"flags": {inner_open}{inner_close}}}"#);

    let mut evaluator = FlagEvaluator::new(ValidationMode::Permissive);
    let response = evaluator
        .update_state(&config)
        .expect("update_state should not panic");

    // Schema/parse errors are expected for this unusual structure — only depth errors are forbidden
    if !response.success {
        let error = response.error.as_deref().unwrap_or("");
        assert!(
            !error.contains("depth") && !error.contains("nesting"),
            "Should not fail with a depth error at exactly the limit; got: {error}"
        );
    }
}

// ============================================================================
// $ref depth limit tests
// ============================================================================

/// Builds a flag config containing a chain of `$ref`s that is `depth` levels deep.
///
/// The structure is: flag targeting → $ref eval_0 → $ref eval_1 → ... → $ref eval_{depth-1}
fn build_ref_chain_config(depth: usize) -> String {
    let mut evaluators = serde_json::Map::new();
    for i in 0..depth {
        let body = if i + 1 < depth {
            serde_json::json!({ "$ref": format!("eval_{}", i + 1) })
        } else {
            serde_json::json!({ "==": [{ "var": "email" }, "user@example.com"] })
        };
        evaluators.insert(format!("eval_{i}"), body);
    }

    serde_json::json!({
        "$evaluators": evaluators,
        "flags": {
            "refFlag": {
                "state": "ENABLED",
                "defaultVariant": "off",
                "variants": { "on": true, "off": false },
                "targeting": { "$ref": "eval_0" }
            }
        }
    })
    .to_string()
}

/// Verifies that a `$ref` chain deeper than `MAX_REF_DEPTH` (64) is rejected
/// with a deterministic error.
#[test]
fn test_deep_ref_chain_rejected() {
    let depth = limits::MAX_REF_DEPTH + 2; // 66 hops — well over the limit
    let config = build_ref_chain_config(depth);

    let mut evaluator = FlagEvaluator::new(ValidationMode::Permissive);
    let response = evaluator
        .update_state(&config)
        .expect("update_state should not panic");

    assert!(
        !response.success,
        "Expected failure for $ref chain of depth {depth}, but got success=true"
    );
    let error = response.error.expect("Expected an error message");
    assert!(
        error.contains("depth") || error.contains("$ref"),
        "Error should mention depth/$ref: {error}"
    );
}

/// Verifies that a `$ref` chain at exactly `MAX_REF_DEPTH` hops is accepted.
#[test]
fn test_ref_chain_at_limit_accepted() {
    let depth = limits::MAX_REF_DEPTH; // exactly at the limit
    let config = build_ref_chain_config(depth);

    let mut evaluator = FlagEvaluator::new(ValidationMode::Permissive);
    let response = evaluator
        .update_state(&config)
        .expect("update_state should not panic");

    // May fail for schema reasons, but must not fail with a depth error
    if !response.success {
        let error = response.error.as_deref().unwrap_or("");
        assert!(
            !error.contains("depth limit"),
            "Should not fail with a depth error at exactly the limit; got: {error}"
        );
    }
}

/// Verifies that an existing circular `$ref` is still detected and rejected.
#[test]
fn test_circular_ref_still_rejected() {
    let config = r#"{
        "$evaluators": {
            "evalA": { "$ref": "evalB" },
            "evalB": { "$ref": "evalA" }
        },
        "flags": {
            "circularFlag": {
                "state": "ENABLED",
                "defaultVariant": "off",
                "variants": { "on": true, "off": false },
                "targeting": { "$ref": "evalA" }
            }
        }
    }"#;

    let mut evaluator = FlagEvaluator::new(ValidationMode::Permissive);
    let response = evaluator
        .update_state(config)
        .expect("update_state should not panic");

    assert!(
        !response.success,
        "Expected failure for circular $ref, but got success=true"
    );
}
