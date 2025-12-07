//! Evaluation result types and logic for feature flag evaluation.
//!
//! This module provides the data structures and functions for evaluating
//! feature flags according to the flagd provider specification.

use crate::model::FeatureFlag;
use crate::operators::create_evaluator;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// The reason for the evaluation result.
///
/// These reasons match the flagd provider specification for evaluation results.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ResolutionReason {
    /// The resolved value is statically configured (no targeting rules exist).
    Static,
    /// The resolved value uses the default variant because targeting didn't match.
    Default,
    /// The resolved value is the result of a successful targeting rule match.
    TargetingMatch,
    /// The flag is disabled, returning the default variant.
    Disabled,
    /// An error occurred during evaluation.
    Error,
    /// The flag was not found in the configuration.
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

    /// Optional metadata associated with the flag.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flag_metadata: Option<std::collections::HashMap<String, Value>>,
}

impl EvaluationResult {
    /// Creates a successful static evaluation result.
    ///
    /// Used when no targeting rules exist and the default variant is used.
    pub fn static_result(value: Value, variant: String) -> Self {
        Self {
            value,
            variant: Some(variant),
            reason: ResolutionReason::Static,
            error_code: None,
            error_message: None,
            flag_metadata: None,
        }
    }

    /// Creates a successful default evaluation result.
    ///
    /// Used when targeting rules exist but didn't match, falling back to default variant.
    pub fn default_result(value: Value, variant: String) -> Self {
        Self {
            value,
            variant: Some(variant),
            reason: ResolutionReason::Default,
            error_code: None,
            error_message: None,
            flag_metadata: None,
        }
    }

    /// Creates a successful targeting match evaluation result.
    ///
    /// Used when targeting rules are evaluated and successfully match.
    pub fn targeting_match(value: Value, variant: String) -> Self {
        Self {
            value,
            variant: Some(variant),
            reason: ResolutionReason::TargetingMatch,
            error_code: None,
            error_message: None,
            flag_metadata: None,
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
            flag_metadata: None,
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
            flag_metadata: None,
        }
    }

    /// Creates a flag not found result.
    pub fn flag_not_found(flag_key: &str) -> Self {
        Self {
            value: Value::Null,
            variant: None,
            reason: ResolutionReason::FlagNotFound,
            error_code: Some(ErrorCode::FlagNotFound),
            error_message: Some(format!("Flag '{}' not found in configuration", flag_key)),
            flag_metadata: None,
        }
    }

    /// Sets the flag metadata for this result.
    pub fn with_metadata(mut self, metadata: std::collections::HashMap<String, Value>) -> Self {
        self.flag_metadata = Some(metadata);
        self
    }

    /// Attaches metadata to the result if the metadata is not empty.
    fn with_metadata_if_present(self, metadata: &std::collections::HashMap<String, Value>) -> Self {
        if metadata.is_empty() {
            self
        } else {
            self.with_metadata(metadata.clone())
        }
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

/// Enriches the evaluation context with standard flagd fields.
///
/// According to the flagd specification, the evaluation context should include:
/// - `flagKey`: The key of the flag being evaluated
/// - `targetingKey`: A key used for consistent hashing (extracted from context or empty)
/// - All custom fields from the original context
///
/// Note: `timestamp` is not included in this WASM implementation as it requires
/// system time which may not be available in all WASM runtimes.
///
/// # Arguments
/// * `flag_key` - The key of the flag being evaluated
/// * `context` - The original evaluation context
///
/// # Returns
/// An enriched context with flagKey and targetingKey added
fn enrich_context(flag_key: &str, context: &Value) -> Value {
    let mut enriched = if let Some(obj) = context.as_object() {
        obj.clone()
    } else {
        Map::new()
    };

    // Add flagKey
    enriched.insert("flagKey".to_string(), Value::String(flag_key.to_string()));

    // Ensure targetingKey exists (use existing or empty string)
    if !enriched.contains_key("targetingKey") {
        enriched.insert("targetingKey".to_string(), Value::String(String::new()));
    }

    Value::Object(enriched)
}

/// Evaluates a feature flag against a context.
///
/// The flag's key should be set in the flag object (from storage).
/// If the key is not set, an error is returned.
///
/// # Arguments
/// * `flag` - The feature flag to evaluate (must have key set)
/// * `context` - The evaluation context (JSON object)
///
/// # Returns
/// An EvaluationResult containing the resolved value, variant, and reason
pub fn evaluate_flag(flag: &FeatureFlag, context: &Value) -> EvaluationResult {
    // Get the flag key from the flag object
    let flag_key = match &flag.key {
        Some(key) => key.as_str(),
        None => {
            return EvaluationResult::error(ErrorCode::General, "Flag key not set in flag object")
        }
    };
    // Check if flag is disabled
    if flag.state == "DISABLED" {
        // Return the default variant value
        if let Some(value) = flag.variants.get(&flag.default_variant) {
            return EvaluationResult::disabled(value.clone(), flag.default_variant.clone())
                .with_metadata_if_present(&flag.metadata);
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
            return EvaluationResult::static_result(value.clone(), flag.default_variant.clone())
                .with_metadata_if_present(&flag.metadata);
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

    // Enrich the context with flagKey and targetingKey
    let enriched_context = enrich_context(flag_key, context);

    // Evaluate targeting rule
    let targeting = flag.targeting.as_ref().unwrap();
    let logic = create_evaluator();

    // Convert targeting rule and enriched context to JSON strings for evaluation
    let rule_str = targeting.to_string();
    let context_str = enriched_context.to_string();

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
                    .with_metadata_if_present(&flag.metadata)
            } else {
                // Variant not found in targeting result, use default
                if let Some(value) = flag.variants.get(&flag.default_variant) {
                    EvaluationResult::default_result(value.clone(), flag.default_variant.clone())
                        .with_metadata_if_present(&flag.metadata)
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
            key: Some("test_flag".to_string()),
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
        assert_eq!(result.reason, ResolutionReason::FlagNotFound);
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
            key: Some("test_flag".to_string()),
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

    #[test]
    fn test_context_enrichment_with_flag_key() {
        let targeting = json!({
            "var": "flagKey"
        });
        let flag = create_test_flag(Some(targeting));
        let context = json!({});

        let result = evaluate_flag(&flag, &context);
        // The targeting rule returns the flagKey variant name, which should be looked up
        // Since "test_flag" is not a valid variant, it should fall back to default
        assert_eq!(result.variant, Some("off".to_string()));
    }

    #[test]
    fn test_context_enrichment_with_targeting_key() {
        let targeting = json!({
            "if": [
                {"==": [{"var": "targetingKey"}, "user-123"]},
                "on",
                "off"
            ]
        });
        let flag = create_test_flag(Some(targeting));
        let context = json!({"targetingKey": "user-123"});

        let result = evaluate_flag(&flag, &context);
        assert_eq!(result.value, json!(true));
        assert_eq!(result.variant, Some("on".to_string()));
    }

    #[test]
    fn test_context_enrichment_default_targeting_key() {
        // When no targetingKey is provided, it should be set to empty string
        let targeting = json!({
            "if": [
                {"==": [{"var": "targetingKey"}, ""]},
                "on",
                "off"
            ]
        });
        let flag = create_test_flag(Some(targeting));
        let context = json!({});

        let result = evaluate_flag(&flag, &context);
        assert_eq!(result.value, json!(true));
        assert_eq!(result.variant, Some("on".to_string()));
    }

    #[test]
    fn test_flag_without_key_returns_error() {
        let mut flag = create_test_flag(None);
        flag.key = None; // Remove the key
        let context = json!({});

        let result = evaluate_flag(&flag, &context);
        assert_eq!(result.reason, ResolutionReason::Error);
        assert_eq!(result.error_code, Some(ErrorCode::General));
        assert!(result.error_message.is_some());
    }

    #[test]
    fn test_flag_metadata_preserved_in_result() {
        let mut metadata = HashMap::new();
        metadata.insert("description".to_string(), json!("Test flag"));
        metadata.insert("team".to_string(), json!("platform"));

        let flag = FeatureFlag {
            key: Some("test_flag".to_string()),
            state: "ENABLED".to_string(),
            default_variant: "on".to_string(),
            variants: {
                let mut v = HashMap::new();
                v.insert("on".to_string(), json!(true));
                v.insert("off".to_string(), json!(false));
                v
            },
            targeting: None,
            metadata: metadata.clone(),
        };

        let context = json!({});
        let result = evaluate_flag(&flag, &context);

        assert_eq!(result.reason, ResolutionReason::Static);
        assert!(result.flag_metadata.is_some());
        let result_metadata = result.flag_metadata.unwrap();
        assert_eq!(
            result_metadata.get("description"),
            Some(&json!("Test flag"))
        );
        assert_eq!(result_metadata.get("team"), Some(&json!("platform")));
    }

    #[test]
    fn test_flag_metadata_with_targeting() {
        let mut metadata = HashMap::new();
        metadata.insert("version".to_string(), json!(2));

        let targeting = json!({
            "if": [
                {"==": [{"var": "user"}, "admin"]},
                "on",
                "off"
            ]
        });

        let flag = FeatureFlag {
            key: Some("test_flag".to_string()),
            state: "ENABLED".to_string(),
            default_variant: "off".to_string(),
            variants: {
                let mut v = HashMap::new();
                v.insert("on".to_string(), json!(true));
                v.insert("off".to_string(), json!(false));
                v
            },
            targeting: Some(targeting),
            metadata: metadata.clone(),
        };

        let context = json!({"user": "admin"});
        let result = evaluate_flag(&flag, &context);

        assert_eq!(result.reason, ResolutionReason::TargetingMatch);
        assert_eq!(result.value, json!(true));
        assert!(result.flag_metadata.is_some());
        assert_eq!(
            result.flag_metadata.unwrap().get("version"),
            Some(&json!(2))
        );
    }

    #[test]
    fn test_flag_metadata_not_included_when_empty() {
        let flag = create_test_flag(None);
        let context = json!({});

        let result = evaluate_flag(&flag, &context);
        assert!(result.flag_metadata.is_none());
    }

    #[test]
    fn test_error_result_structure() {
        let result = EvaluationResult::error(ErrorCode::ParseError, "Invalid targeting rule");

        assert_eq!(result.reason, ResolutionReason::Error);
        assert_eq!(result.error_code, Some(ErrorCode::ParseError));
        assert_eq!(
            result.error_message,
            Some("Invalid targeting rule".to_string())
        );
        assert_eq!(result.value, Value::Null);
        assert!(result.variant.is_none());
        assert!(result.flag_metadata.is_none());
    }

    #[test]
    fn test_all_error_codes_serialize() {
        let error_codes = vec![
            ErrorCode::FlagNotFound,
            ErrorCode::ParseError,
            ErrorCode::TypeMismatch,
            ErrorCode::General,
        ];

        for error_code in error_codes {
            let result = EvaluationResult::error(error_code.clone(), "test error");
            let json_str = result.to_json_string();
            let parsed: Value = serde_json::from_str(&json_str).unwrap();

            assert_eq!(parsed["reason"], "ERROR");
            assert!(parsed["errorCode"].is_string());
            assert_eq!(parsed["errorMessage"], "test error");
            assert_eq!(parsed["value"], Value::Null);
        }
    }

    #[test]
    fn test_all_resolution_reasons_serialize() {
        let test_cases = vec![
            (
                EvaluationResult::static_result(json!(true), "on".to_string()),
                "STATIC",
            ),
            (
                EvaluationResult::default_result(json!(false), "off".to_string()),
                "DEFAULT",
            ),
            (
                EvaluationResult::targeting_match(json!(false), "off".to_string()),
                "TARGETING_MATCH",
            ),
            (
                EvaluationResult::disabled(json!(null), "default".to_string()),
                "DISABLED",
            ),
            (
                EvaluationResult::error(ErrorCode::General, "error"),
                "ERROR",
            ),
            (EvaluationResult::flag_not_found("test"), "FLAG_NOT_FOUND"),
        ];

        for (result, expected_reason) in test_cases {
            let json_str = result.to_json_string();
            let parsed: Value = serde_json::from_str(&json_str).unwrap();
            assert_eq!(parsed["reason"], expected_reason);
        }
    }

    #[test]
    fn test_result_with_metadata_serialization() {
        let mut metadata = HashMap::new();
        metadata.insert("key1".to_string(), json!("value1"));
        metadata.insert("key2".to_string(), json!(42));

        let result = EvaluationResult::static_result(json!("test"), "variant".to_string())
            .with_metadata(metadata);

        let json_str = result.to_json_string();
        let parsed: Value = serde_json::from_str(&json_str).unwrap();

        assert_eq!(parsed["value"], "test");
        assert_eq!(parsed["variant"], "variant");
        assert_eq!(parsed["reason"], "STATIC");
        assert!(parsed["flagMetadata"].is_object());
        assert_eq!(parsed["flagMetadata"]["key1"], "value1");
        assert_eq!(parsed["flagMetadata"]["key2"], 42);
    }

    #[test]
    fn test_type_mismatch_error() {
        let result =
            EvaluationResult::error(ErrorCode::TypeMismatch, "Expected boolean but got string");

        assert_eq!(result.reason, ResolutionReason::Error);
        assert_eq!(result.error_code, Some(ErrorCode::TypeMismatch));
        assert!(result
            .error_message
            .unwrap()
            .contains("Expected boolean but got string"));
    }

    #[test]
    fn test_parse_error_from_invalid_targeting() {
        let mut flag = create_test_flag(Some(json!({"invalid_operator": [1, 2, 3]})));
        flag.key = Some("test".to_string());

        let result = evaluate_flag(&flag, &json!({}));
        assert_eq!(result.reason, ResolutionReason::Error);
        assert_eq!(result.error_code, Some(ErrorCode::ParseError));
        assert!(result.error_message.is_some());
    }

    #[test]
    fn test_disabled_flag_with_metadata() {
        let mut metadata = HashMap::new();
        metadata.insert("reason".to_string(), json!("deprecated"));

        let mut flag = create_test_flag(None);
        flag.state = "DISABLED".to_string();
        flag.metadata = metadata;

        let result = evaluate_flag(&flag, &json!({}));

        assert_eq!(result.reason, ResolutionReason::Disabled);
        assert!(result.flag_metadata.is_some());
        assert_eq!(
            result.flag_metadata.unwrap().get("reason"),
            Some(&json!("deprecated"))
        );
    }

    #[test]
    fn test_json_serialization_format() {
        // Test that the JSON output matches the expected format
        let mut metadata = HashMap::new();
        metadata.insert("test".to_string(), json!("value"));

        let result = EvaluationResult::static_result(json!(42), "variant1".to_string())
            .with_metadata(metadata);

        let json_str = result.to_json_string();
        let parsed: Value = serde_json::from_str(&json_str).unwrap();

        // Verify all fields are present and correctly formatted
        assert_eq!(parsed["value"], 42);
        assert_eq!(parsed["variant"], "variant1");
        assert_eq!(parsed["reason"], "STATIC");
        assert!(parsed["flagMetadata"].is_object());
        assert_eq!(parsed["flagMetadata"]["test"], "value");
        // errorCode and errorMessage should not be present for success
        assert!(parsed.get("errorCode").is_none() || parsed["errorCode"].is_null());
        assert!(parsed.get("errorMessage").is_none() || parsed["errorMessage"].is_null());
    }

    #[test]
    fn test_error_json_serialization_format() {
        let result = EvaluationResult::error(ErrorCode::FlagNotFound, "Flag not found");

        let json_str = result.to_json_string();
        let parsed: Value = serde_json::from_str(&json_str).unwrap();

        // Verify error fields
        assert_eq!(parsed["reason"], "ERROR");
        assert_eq!(parsed["errorCode"], "FLAG_NOT_FOUND");
        assert_eq!(parsed["errorMessage"], "Flag not found");
        assert!(parsed["value"].is_null());
        // variant should not be present for errors
        assert!(parsed.get("variant").is_none() || parsed["variant"].is_null());
        assert!(parsed.get("flagMetadata").is_none() || parsed["flagMetadata"].is_null());
    }
}
