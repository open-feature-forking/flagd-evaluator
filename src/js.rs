//! JavaScript/WASM bindings using wasm-bindgen
//!
//! This module provides JavaScript-friendly bindings for the flagd-evaluator library.
//! It handles string conversion and JSON serialization automatically via wasm-bindgen
//! and serde-wasm-bindgen, returning native JavaScript objects instead of JSON strings.
//!
//! ## Usage from JavaScript
//!
//! ```javascript
//! import init, { evaluate, FlagdEvaluator } from './pkg/flagd_evaluator.js';
//!
//! await init();
//!
//! // Simple function API - returns a JavaScript object directly
//! const result = evaluate('{"==": [1, 1]}', '{}');
//! console.log(result.success); // true
//! console.log(result.result);  // true
//!
//! // Or using the class API
//! const evaluator = new FlagdEvaluator();
//! const result = evaluator.evaluate('{"==": [1, 1]}', '{}');
//! console.log(result.success); // true
//! ```

use wasm_bindgen::prelude::*;

/// Evaluates a JSON Logic rule against the provided data.
///
/// This is the main entry point for JavaScript/WASM usage. It accepts JSON strings
/// for both the rule and data, evaluates the rule, and returns a JavaScript object
/// with the evaluation result.
///
/// # Arguments
/// * `rule_json` - A JSON string representing the rule to evaluate
/// * `data_json` - A JSON string representing the data to evaluate against
///
/// # Returns
/// A JavaScript object with the following structure:
/// ```javascript
/// {
///   success: true|false,
///   result: <value>|null,
///   error: null|"error message"
/// }
/// ```
///
/// # Example
/// ```javascript
/// const result = evaluate('{"==": [1, 1]}', '{}');
/// console.log(result.success); // true
/// console.log(result.result);  // true
/// ```
#[wasm_bindgen]
pub fn evaluate(rule_json: &str, data_json: &str) -> JsValue {
    let logic = crate::create_evaluator();

    let response = match logic.evaluate_json(rule_json, data_json) {
        Ok(result) => crate::EvaluationResponse::success(result),
        Err(e) => crate::EvaluationResponse::error(format!("{}", e)),
    };

    serde_wasm_bindgen::to_value(&response).unwrap_or_else(|e| {
        // Fallback in case serialization fails
        let error_response =
            crate::EvaluationResponse::error(format!("Serialization failed: {}", e));
        serde_wasm_bindgen::to_value(&error_response).unwrap_or(JsValue::NULL)
    })
}

/// A stateful evaluator for JSON Logic rules.
///
/// This provides a class-based API for JavaScript consumers who prefer
/// object-oriented interfaces.
///
/// # Example
/// ```javascript
/// const evaluator = new FlagdEvaluator();
/// const result = evaluator.evaluate('{"var": "name"}', '{"name": "Alice"}');
/// console.log(result.success); // true
/// console.log(result.result);  // "Alice"
/// ```
#[wasm_bindgen]
pub struct FlagdEvaluator;

#[wasm_bindgen]
impl FlagdEvaluator {
    /// Creates a new FlagdEvaluator instance.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        FlagdEvaluator
    }

    /// Evaluates a JSON Logic rule against the provided data.
    ///
    /// # Arguments
    /// * `rule` - A JSON string representing the rule to evaluate
    /// * `data` - A JSON string representing the data to evaluate against
    ///
    /// # Returns
    /// A JavaScript object containing the evaluation response
    #[wasm_bindgen]
    pub fn evaluate(&self, rule: &str, data: &str) -> JsValue {
        evaluate(rule, data)
    }
}

impl Default for FlagdEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_evaluate_basic() {
        // In tests, we can't easily test JsValue, so we test the underlying logic
        let logic = crate::create_evaluator();
        let result = logic.evaluate_json(r#"{"==": [1, 1]}"#, "{}");
        assert!(result.is_ok());
    }

    #[test]
    fn test_evaluator_class() {
        let logic = crate::create_evaluator();
        let result = logic.evaluate_json(r#"{"==": [1, 1]}"#, "{}");
        assert!(result.is_ok());
    }
}
