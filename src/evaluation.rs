//! Evaluation result types and logic for feature flag evaluation.
//!
//! This module provides the data structures and functions for evaluating
//! feature flags according to the flagd provider specification.

use crate::model::FeatureFlag;
use crate::operators::create_evaluator;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The reason for the evaluation result.
///
/// These reasons match the flagd provider specification for evaluation results.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ResolutionReason {
    /// The resolved value is statically configured (no targeting rules).
    Static,
    /// The resolved value is the result of targeting rule evaluation.
    TargetingMatch,
    /// The flag is disabled, returning the default variant.
    Disabled,
    /// An error occurred during evaluation.
    Error,
    /// The flag was not found.
    FlagNotFound,
}

/// Error codes matching the flagd provider specification.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    /// The flag key was not found in the configuration.
    FlagNotFound,
    /// Error parsing or evaluating the targeting rule.
    ParseError,
    /// The evaluated type does not match the expected type.
    TypeMismatch,
    /// Generic evaluation error.
    General,
}

/// The result of a feature flag evaluation.
///
/// This structure matches the flagd provider specification for evaluation results.
/// See: https://flagd.dev/reference/specifications/providers/#evaluation-results
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvaluationResult {
    /// The resolved value of the flag.
    pub value: Value,

    /// The variant name that was selected (if applicable).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,

    /// The reason for the resolution.
    pub reason: ResolutionReason,

    /// Error code if an error occurred.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<ErrorCode>,

    /// Error message if an error occurred.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

impl EvaluationResult {
    /// Creates a successful static evaluation result.
    ///
    /// Used when no targeting rules are evaluated and the default variant is used.
    pub fn static_result(value: Value, variant: String) -> Self {
        Self {
            value,
            variant: Some(variant),
            reason: ResolutionReason::Static,
            error_code: None,
            error_message: None,
        }
    }

    /// Creates a successful targeting match evaluation result.
    ///
    /// Used when targeting rules are evaluated and match.
    pub fn targeting_match(value: Value, variant: String) -> Self {
        Self {
            value,
            variant: Some(variant),
            reason: ResolutionReason::TargetingMatch,
            error_code: None,
            error_message: None,
        }
    }

    /// Creates a disabled flag evaluation result.
    pub fn disabled(value: Value, variant: String) -> Self {
        Self {
            value,
            variant: Some(variant),
            reason: ResolutionReason::Disabled,
            error_code: None,
            error_message: None,
        }
    }

    /// Creates an error evaluation result.
    pub fn error(error_code: ErrorCode, error_message: impl Into<String>) -> Self {
        Self {
            value: Value::Null,
            variant: None,
            reason: ResolutionReason::Error,
            error_code: Some(error_code),
            error_message: Some(error_message.into()),
        }
    }

    /// Creates a flag not found error result.
    pub fn flag_not_found(flag_key: &str) -> Self {
        Self::error(
            ErrorCode::FlagNotFound,
            format!("Flag '{}' not found in configuration", flag_key),
        )
    }

    /// Serializes the result to a JSON string.
    pub fn to_json_string(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|e| {
            format!(
                r#"{{"value":null,"reason":"ERROR","errorCode":"GENERAL","errorMessage":"Serialization failed: {}"}}"#,
                e
            )
        })
    }
}

/// Evaluates a feature flag against a context.
///
/// # Arguments
/// * `flag` - The feature flag to evaluate
/// * `context` - The evaluation context (JSON object)
///
/// # Returns
/// An EvaluationResult containing the resolved value, variant, and reason
pub fn evaluate_flag(flag: &FeatureFlag, context: &Value) -> EvaluationResult {
    // Check if flag is disabled
    if flag.state == "DISABLED" {
        // Return the default variant value
        if let Some(value) = flag.variants.get(&flag.default_variant) {
            return EvaluationResult::disabled(value.clone(), flag.default_variant.clone());
        } else {
            return EvaluationResult::error(
                ErrorCode::General,
                format!(
                    "Default variant '{}' not found in flag variants",
                    flag.default_variant
                ),
            );
        }
    }

    // If there's no targeting rule, return the default variant
    if flag.targeting.is_none() {
        if let Some(value) = flag.variants.get(&flag.default_variant) {
            return EvaluationResult::static_result(value.clone(), flag.default_variant.clone());
        } else {
            return EvaluationResult::error(
                ErrorCode::General,
                format!(
                    "Default variant '{}' not found in flag variants",
                    flag.default_variant
                ),
            );
        }
    }

    // Evaluate targeting rule
    let targeting = flag.targeting.as_ref().unwrap();
    let logic = create_evaluator();

    // Convert targeting rule and context to JSON strings for evaluation
    let rule_str = targeting.to_string();
    let context_str = context.to_string();

    match logic.evaluate_json(&rule_str, &context_str) {
        Ok(result) => {
            // The result should be a variant name (string)
            let variant_name = match &result {
                Value::String(s) => s.clone(),
                _ => {
                    // If the result is not a string, try to convert it to string
                    match result.as_str() {
                        Some(s) => s.to_string(),
                        None => result.to_string().trim_matches('"').to_string(),
                    }
                }
            };

            // Look up the variant value
            if let Some(value) = flag.variants.get(&variant_name) {
                EvaluationResult::targeting_match(value.clone(), variant_name)
            } else {
                // Variant not found, use default
                if let Some(value) = flag.variants.get(&flag.default_variant) {
                    EvaluationResult::targeting_match(value.clone(), flag.default_variant.clone())
                } else {
                    EvaluationResult::error(
                        ErrorCode::General,
                        format!("Variant '{}' not found in flag variants", variant_name),
                    )
                }
            }
        }
        Err(e) => {
            EvaluationResult::error(ErrorCode::ParseError, format!("Evaluation error: {}", e))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashMap;

    fn create_test_flag(targeting: Option<Value>) -> FeatureFlag {
        let mut variants = HashMap::new();
        variants.insert("on".to_string(), json!(true));
        variants.insert("off".to_string(), json!(false));

        FeatureFlag {
            state: "ENABLED".to_string(),
            default_variant: "off".to_string(),
            variants,
            targeting,
            metadata: HashMap::new(),
        }
    }

    #[test]
    fn test_static_result() {
        let flag = create_test_flag(None);
        let context = json!({});

        let result = evaluate_flag(&flag, &context);
        assert_eq!(result.value, json!(false));
        assert_eq!(result.variant, Some("off".to_string()));
        assert_eq!(result.reason, ResolutionReason::Static);
        assert!(result.error_code.is_none());
    }

    #[test]
    fn test_disabled_flag() {
        let mut flag = create_test_flag(None);
        flag.state = "DISABLED".to_string();
        let context = json!({});

        let result = evaluate_flag(&flag, &context);
        assert_eq!(result.value, json!(false));
        assert_eq!(result.variant, Some("off".to_string()));
        assert_eq!(result.reason, ResolutionReason::Disabled);
    }

    #[test]
    fn test_targeting_match() {
        let targeting = json!({
            "if": [
                {"==": [{"var": "user"}, "admin"]},
                "on",
                "off"
            ]
        });
        let flag = create_test_flag(Some(targeting));
        let context = json!({"user": "admin"});

        let result = evaluate_flag(&flag, &context);
        assert_eq!(result.value, json!(true));
        assert_eq!(result.variant, Some("on".to_string()));
        assert_eq!(result.reason, ResolutionReason::TargetingMatch);
    }

    #[test]
    fn test_targeting_no_match() {
        let targeting = json!({
            "if": [
                {"==": [{"var": "user"}, "admin"]},
                "on",
                "off"
            ]
        });
        let flag = create_test_flag(Some(targeting));
        let context = json!({"user": "guest"});

        let result = evaluate_flag(&flag, &context);
        assert_eq!(result.value, json!(false));
        assert_eq!(result.variant, Some("off".to_string()));
        assert_eq!(result.reason, ResolutionReason::TargetingMatch);
    }

    #[test]
    fn test_flag_not_found_result() {
        let result = EvaluationResult::flag_not_found("missing-flag");
        assert_eq!(result.reason, ResolutionReason::Error);
        assert_eq!(result.error_code, Some(ErrorCode::FlagNotFound));
        assert!(result.error_message.is_some());
    }

    #[test]
    fn test_result_serialization() {
        let result = EvaluationResult::static_result(json!(42), "variant1".to_string());
        let json_str = result.to_json_string();
        assert!(json_str.contains("\"value\":42"));
        assert!(json_str.contains("\"variant\":\"variant1\""));
        assert!(json_str.contains("\"reason\":\"STATIC\""));
    }

    #[test]
    fn test_different_value_types() {
        let mut variants = HashMap::new();
        variants.insert("string_variant".to_string(), json!("hello"));
        variants.insert("int_variant".to_string(), json!(42));
        variants.insert("float_variant".to_string(), json!(3.14));
        variants.insert("bool_variant".to_string(), json!(true));
        variants.insert("object_variant".to_string(), json!({"key": "value"}));

        let targeting = json!({
            "var": "variant_name"
        });

        let flag = FeatureFlag {
            state: "ENABLED".to_string(),
            default_variant: "string_variant".to_string(),
            variants,
            targeting: Some(targeting),
            metadata: HashMap::new(),
        };

        // Test string variant
        let result = evaluate_flag(&flag, &json!({"variant_name": "string_variant"}));
        assert_eq!(result.value, json!("hello"));

        // Test int variant
        let result = evaluate_flag(&flag, &json!({"variant_name": "int_variant"}));
        assert_eq!(result.value, json!(42));

        // Test float variant
        let result = evaluate_flag(&flag, &json!({"variant_name": "float_variant"}));
        assert_eq!(result.value, json!(3.14));

        // Test bool variant
        let result = evaluate_flag(&flag, &json!({"variant_name": "bool_variant"}));
        assert_eq!(result.value, json!(true));

        // Test object variant
        let result = evaluate_flag(&flag, &json!({"variant_name": "object_variant"}));
        assert_eq!(result.value, json!({"key": "value"}));
    }
}
