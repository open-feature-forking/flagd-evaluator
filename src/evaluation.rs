//! Evaluation result types and logic for feature flag evaluation.
//!
//! This module provides the data structures and functions for evaluating
//! feature flags according to the flagd provider specification.

use crate::model::FeatureFlag;
use crate::operators::create_evaluator;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashMap;

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
    Fallback,
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
    ///
    /// This is a merged representation of flag-set metadata (from the root level of the
    /// configuration) and flag-level metadata (from the specific flag definition), with
    /// flag-level metadata taking priority over flag-set metadata when keys conflict.
    ///
    /// Metadata is returned on a "best effort" basis:
    /// - For successful evaluations: both flag-set and flag metadata are merged
    /// - For disabled flags: both flag-set and flag metadata are merged  
    /// - For missing flags (FLAG_NOT_FOUND): only flag-set metadata is returned
    /// - For error cases: metadata is omitted
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

    /// Creates a fallback result.
    ///
    /// This is used when no defaultVariant is set, signaling the client
    /// to use its code-defined default value. The FALLBACK reason provides
    /// more semantic information than ERROR, though consumers may map this
    /// to FLAG_NOT_FOUND for compatibility.
    pub fn fallback(flag_key: &str) -> Self {
        Self {
            value: Value::Null,
            variant: None,
            reason: ResolutionReason::Fallback,
            error_code: Some(ErrorCode::FlagNotFound),
            error_message: Some(format!(
                "Flag '{}' has no default variant defined, will use code default",
                flag_key
            )),
            flag_metadata: None,
        }
    }

    /// Sets the flag metadata for this result.
    pub fn with_metadata(mut self, metadata: std::collections::HashMap<String, Value>) -> Self {
        self.flag_metadata = Some(metadata);
        self
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
/// - `$flagd.flagKey`: The key of the flag being evaluated
/// - `$flagd.timestamp`: Unix timestamp (in seconds) of the time of evaluation
/// - `targetingKey`: A key used for consistent hashing (extracted from context or empty)
/// - All custom fields from the original context
///
/// The `$flagd` properties are stored as a nested object to support dot notation access
/// in JSON Logic (e.g., `{"var": "$flagd.timestamp"}`).
///
/// # Arguments
/// * `flag_key` - The key of the flag being evaluated
/// * `context` - The original evaluation context
///
/// # Returns
/// An enriched context with $flagd properties and targetingKey added
fn enrich_context(flag_key: &str, context: &Value) -> Value {
    let mut enriched = if let Some(obj) = context.as_object() {
        obj.clone()
    } else {
        Map::new()
    };

    // Get current Unix timestamp (seconds since epoch)
    // Try to use the host-provided time function if available, otherwise default to 0.
    // Defaulting to 0 signals to targeting rules that time is unavailable.
    let timestamp = crate::get_current_time();

    // Create $flagd object with nested properties
    let mut flagd_props = Map::new();
    flagd_props.insert("flagKey".to_string(), Value::String(flag_key.to_string()));
    // Store timestamp as u64 to avoid overflow issues. JSON can represent large numbers.
    flagd_props.insert("timestamp".to_string(), Value::Number(timestamp.into()));

    // Add $flagd object to context
    enriched.insert("$flagd".to_string(), Value::Object(flagd_props));

    // Ensure targetingKey exists (use existing or empty string)
    if !enriched.contains_key("targetingKey") {
        enriched.insert("targetingKey".to_string(), Value::String(String::new()));
    }

    Value::Object(enriched)
}

/// Merges flag-set metadata with flag-level metadata.
///
/// According to the flagd provider specification, flag metadata takes priority over
/// flag-set metadata. This function creates a merged metadata map with flag-set
/// metadata as the base and flag metadata overriding any duplicate keys.
///
/// # Arguments
/// * `flag_set_metadata` - The metadata from the flag configuration root
/// * `flag_metadata` - The metadata from the specific flag
///
/// # Returns
/// A merged HashMap with flag metadata taking priority, or None if both are empty
fn merge_metadata(
    flag_set_metadata: &std::collections::HashMap<String, Value>,
    flag_metadata: &std::collections::HashMap<String, Value>,
) -> Option<std::collections::HashMap<String, Value>> {
    // Filter out internal fields (those starting with $) from flag-set metadata
    let filtered_flag_set: HashMap<String, Value> = flag_set_metadata
        .iter()
        .filter(|(key, _)| !key.starts_with('$'))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    // If both are empty after filtering, return None
    if filtered_flag_set.is_empty() && flag_metadata.is_empty() {
        return None;
    }

    // Start with filtered flag-set metadata as the base
    let mut merged = filtered_flag_set;

    // Override with flag-level metadata (flag metadata takes priority)
    for (key, value) in flag_metadata {
        merged.insert(key.clone(), value.clone());
    }

    Some(merged)
}

/// Evaluates a feature flag against a context.
///
/// The flag's key should be set in the flag object (from storage).
/// If the key is not set, an error is returned.
///
/// # Arguments
/// * `flag` - The feature flag to evaluate (must have key set)
/// * `context` - The evaluation context (JSON object)
/// * `flag_set_metadata` - Optional flag-set level metadata to merge with flag metadata
///
/// # Returns
/// An EvaluationResult containing the resolved value, variant, reason, and merged metadata
pub fn evaluate_flag(
    flag: &FeatureFlag,
    context: &Value,
    flag_set_metadata: &std::collections::HashMap<String, Value>,
) -> EvaluationResult {
    // Get the flag key from the flag object
    let flag_key = match &flag.key {
        Some(key) => key.as_str(),
        None => {
            return EvaluationResult::error(ErrorCode::General, "Flag key not set in flag object")
        }
    };

    // Merge metadata (flag metadata takes priority over flag-set metadata)
    let merged_metadata = merge_metadata(flag_set_metadata, &flag.metadata);

    // Check if flag is disabled
    // Return Disabled reason with FLAG_NOT_FOUND error code to signal the client
    // to use its code-defined default value. The Disabled reason provides better
    // semantic information for future use, while FLAG_NOT_FOUND maintains compatibility.
    if flag.state == "DISABLED" {
        return EvaluationResult {
            value: Value::Null,
            variant: None,
            reason: ResolutionReason::Disabled,
            error_code: Some(ErrorCode::FlagNotFound),
            error_message: Some(format!("flag: {} is disabled", flag_key)),
            flag_metadata: merged_metadata,
        };
    }

    // Check if there's no targeting rule or if it's an empty object "{}"
    // According to Java implementation (InProcessResolver.java:200-203),
    // an empty targeting string "{}" should be treated as STATIC (no targeting)
    let is_empty_targeting = match &flag.targeting {
        None => true,
        Some(Value::Object(map)) if map.is_empty() => true,
        _ => false,
    };

    if is_empty_targeting {
        return match flag.default_variant.as_ref() {
            None => EvaluationResult::fallback(flag_key),
            Some(ref value) if value.is_empty() => EvaluationResult::fallback(flag_key),
            Some(default_variant) => match flag.variants.get(default_variant) {
                Some(value) => {
                    let result =
                        EvaluationResult::static_result(value.clone(), default_variant.clone());
                    with_metadata(merged_metadata, result)
                }
                None => {
                    return EvaluationResult::error(
                        ErrorCode::General,
                        format!(
                            "Default variant '{}' not found in flag variants",
                            default_variant
                        ),
                    );
                }
            },
        };
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
            // Check if targeting returned null - this means use default variant
            // This matches the Java implementation behavior
            if result.is_null() {
                return match flag.default_variant.as_ref() {
                    None => EvaluationResult::fallback(flag_key),
                    Some(ref value) if value.is_empty() => EvaluationResult::fallback(flag_key),
                    Some(default_variant) => match flag.variants.get(default_variant) {
                        Some(value) => {
                            let result = EvaluationResult::default_result(
                                value.clone(),
                                default_variant.clone(),
                            );
                            with_metadata(merged_metadata, result)
                        }
                        None => EvaluationResult::error(
                            ErrorCode::General,
                            format!(
                                "Default variant '{}' not found in flag variants",
                                default_variant
                            ),
                        ),
                    },
                };
            }

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

            // Check for empty variant name - if both resolvedVariant and defaultVariant are empty,
            // return FALLBACK to signal "use code default" (Java implementation lines 223-229)
            if variant_name.is_empty() {
                return match flag.default_variant.as_ref() {
                    None => {
                        // Both are empty - return FALLBACK (use code default)
                        EvaluationResult::fallback(flag_key)
                    }
                    Some(default_variant) if default_variant.is_empty() => {
                        // Default variant is also empty - return FALLBACK (use code default)
                        EvaluationResult::fallback(flag_key)
                    }
                    Some(_) => {
                        // Resolved variant is empty but default is not - this is an error
                        EvaluationResult::error(
                            ErrorCode::General,
                            format!(
                                "Targeting rule returned empty variant name for flag '{}'",
                                flag_key
                            ),
                        )
                    }
                };
            }

            // Look up the variant value
            match flag.variants.get(&variant_name) {
                Some(value) => {
                    let result = EvaluationResult::targeting_match(value.clone(), variant_name);
                    with_metadata(merged_metadata, result)
                }
                None => {
                    // Variant name returned by targeting doesn't exist in variants map
                    // This is an error condition according to flagd spec
                    EvaluationResult::error(
                        ErrorCode::General,
                        format!(
                            "Targeting rule returned variant '{}' which is not defined in flag variants",
                            variant_name
                        ),
                    )
                }
            }
        }
        Err(e) => {
            EvaluationResult::error(ErrorCode::ParseError, format!("Evaluation error: {}", e))
        }
    }
}

fn with_metadata(
    merged_metadata: Option<HashMap<String, Value>>,
    result: EvaluationResult,
) -> EvaluationResult {
    match merged_metadata {
        Some(metadata) => result.with_metadata(metadata),
        None => result,
    }
}

/// Evaluates a boolean feature flag with type checking.
///
/// This function evaluates the flag and ensures the result is a boolean value.
/// If the value is not a boolean, it returns a TYPE_MISMATCH error.
///
/// # Arguments
/// * `flag` - The feature flag to evaluate
/// * `context` - The evaluation context (JSON object)
/// * `flag_set_metadata` - Flag-set level metadata to merge with flag metadata
///
/// # Returns
/// An EvaluationResult with a boolean value or TYPE_MISMATCH error
pub fn evaluate_bool_flag(
    flag: &FeatureFlag,
    context: &Value,
    flag_set_metadata: &std::collections::HashMap<String, Value>,
) -> EvaluationResult {
    let result = evaluate_flag(flag, context, flag_set_metadata);

    // If there's already an error or special status, return it as-is
    if result.reason == ResolutionReason::Error
        || result.reason == ResolutionReason::FlagNotFound
        || result.reason == ResolutionReason::Fallback
        || result.reason == ResolutionReason::Disabled
    {
        return result;
    }

    // Check if the value is a boolean
    if result.value.is_boolean() {
        result
    } else {
        EvaluationResult::error(
            ErrorCode::TypeMismatch,
            format!(
                "Flag value has incorrect type. Expected boolean, got {}",
                type_name(&result.value)
            ),
        )
    }
}

/// Evaluates a string feature flag with type checking.
///
/// This function evaluates the flag and ensures the result is a string value.
/// If the value is not a string, it returns a TYPE_MISMATCH error.
///
/// # Arguments
/// * `flag` - The feature flag to evaluate
/// * `context` - The evaluation context (JSON object)
/// * `flag_set_metadata` - Flag-set level metadata to merge with flag metadata
///
/// # Returns
/// An EvaluationResult with a string value or TYPE_MISMATCH error
pub fn evaluate_string_flag(
    flag: &FeatureFlag,
    context: &Value,
    flag_set_metadata: &std::collections::HashMap<String, Value>,
) -> EvaluationResult {
    let result = evaluate_flag(flag, context, flag_set_metadata);

    // If there's already an error or special status, return it as-is
    if result.reason == ResolutionReason::Error
        || result.reason == ResolutionReason::FlagNotFound
        || result.reason == ResolutionReason::Fallback
        || result.reason == ResolutionReason::Disabled
    {
        return result;
    }

    // Check if the value is a string
    if result.value.is_string() {
        result
    } else {
        EvaluationResult::error(
            ErrorCode::TypeMismatch,
            format!(
                "Flag value has incorrect type. Expected string, got {}",
                type_name(&result.value)
            ),
        )
    }
}

/// Evaluates an integer feature flag with type checking.
///
/// This function evaluates the flag and ensures the result is an integer value.
/// If the value is not an integer (i64), it returns a TYPE_MISMATCH error.
/// Note: Floating point numbers are coerced to integers (matching Java behavior).
///
/// # Arguments
/// * `flag` - The feature flag to evaluate
/// * `context` - The evaluation context (JSON object)
/// * `flag_set_metadata` - Flag-set level metadata to merge with flag metadata
///
/// # Returns
/// An EvaluationResult with an integer value or TYPE_MISMATCH error
pub fn evaluate_int_flag(
    flag: &FeatureFlag,
    context: &Value,
    flag_set_metadata: &std::collections::HashMap<String, Value>,
) -> EvaluationResult {
    let mut result = evaluate_flag(flag, context, flag_set_metadata);

    // If there's already an error or special status, return it as-is
    if result.reason == ResolutionReason::Error
        || result.reason == ResolutionReason::FlagNotFound
        || result.reason == ResolutionReason::Fallback
        || result.reason == ResolutionReason::Disabled
    {
        return result;
    }

    // Type coercion: if this is a double and we are trying to resolve an integer, convert
    // This matches the Java implementation behavior (InProcessResolver.java:239-241)
    if result.value.is_f64() {
        if let Some(f) = result.value.as_f64() {
            result.value = Value::Number(serde_json::Number::from(f as i64));
            return result;
        }
    }

    // Check if the value is an integer (i64)
    // Note: JSON numbers can be i64 or f64, we need to ensure it's an integer
    if result.value.is_i64() || result.value.is_u64() {
        result
    } else {
        EvaluationResult::error(
            ErrorCode::TypeMismatch,
            format!(
                "Flag value has incorrect type. Expected integer, got {}",
                type_name(&result.value)
            ),
        )
    }
}

/// Evaluates a float feature flag with type checking.
///
/// This function evaluates the flag and ensures the result is a numeric value.
/// Both integers and floating point numbers are accepted as valid float values.
/// Integers are automatically coerced to doubles (matching Java behavior).
///
/// # Arguments
/// * `flag` - The feature flag to evaluate
/// * `context` - The evaluation context (JSON object)
/// * `flag_set_metadata` - Flag-set level metadata to merge with flag metadata
///
/// # Returns
/// An EvaluationResult with a float value or TYPE_MISMATCH error
pub fn evaluate_float_flag(
    flag: &FeatureFlag,
    context: &Value,
    flag_set_metadata: &std::collections::HashMap<String, Value>,
) -> EvaluationResult {
    let mut result = evaluate_flag(flag, context, flag_set_metadata);

    // If there's already an error or special status, return it as-is
    if result.reason == ResolutionReason::Error
        || result.reason == ResolutionReason::FlagNotFound
        || result.reason == ResolutionReason::Fallback
        || result.reason == ResolutionReason::Disabled
    {
        return result;
    }

    // Type coercion: if this is an integer and we are trying to resolve a double, convert
    // This matches the Java implementation behavior (InProcessResolver.java:236-238)
    if result.value.is_i64() || result.value.is_u64() {
        if let Some(i) = result.value.as_i64() {
            // Convert integer to float
            if let Some(num) = serde_json::Number::from_f64(i as f64) {
                result.value = Value::Number(num);
            }
        } else if let Some(u) = result.value.as_u64() {
            // Convert unsigned integer to float
            if let Some(num) = serde_json::Number::from_f64(u as f64) {
                result.value = Value::Number(num);
            }
        }
        return result;
    }

    // Check if the value is a number (integer or float)
    if result.value.is_number() {
        result
    } else {
        EvaluationResult::error(
            ErrorCode::TypeMismatch,
            format!(
                "Flag value has incorrect type. Expected float, got {}",
                type_name(&result.value)
            ),
        )
    }
}

/// Evaluates an object/struct feature flag with type checking.
///
/// This function evaluates the flag and ensures the result is an object value.
/// If the value is not an object, it returns a TYPE_MISMATCH error.
///
/// # Arguments
/// * `flag` - The feature flag to evaluate
/// * `context` - The evaluation context (JSON object)
/// * `flag_set_metadata` - Flag-set level metadata to merge with flag metadata
///
/// # Returns
/// An EvaluationResult with an object value or TYPE_MISMATCH error
pub fn evaluate_object_flag(
    flag: &FeatureFlag,
    context: &Value,
    flag_set_metadata: &std::collections::HashMap<String, Value>,
) -> EvaluationResult {
    let result = evaluate_flag(flag, context, flag_set_metadata);

    // If there's already an error or special status, return it as-is
    if result.reason == ResolutionReason::Error
        || result.reason == ResolutionReason::FlagNotFound
        || result.reason == ResolutionReason::Fallback
        || result.reason == ResolutionReason::Disabled
    {
        return result;
    }

    // Check if the value is an object
    if result.value.is_object() {
        result
    } else {
        EvaluationResult::error(
            ErrorCode::TypeMismatch,
            format!(
                "Flag value has incorrect type. Expected object, got {}",
                type_name(&result.value)
            ),
        )
    }
}

/// Helper function to get a human-readable type name from a JSON value.
///
/// Note: In JSON, integers like 10 are typically stored as i64, while numbers
/// with decimal points like 10.0 or 3.14 are stored as f64. This function
/// distinguishes between these to provide accurate error messages.
fn type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(n) => {
            if n.is_i64() || n.is_u64() {
                "integer"
            } else {
                "float"
            }
        }
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
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
            default_variant: Option::from("off".to_string()),
            variants,
            targeting,
            metadata: HashMap::new(),
        }
    }

    // Helper to get empty flag-set metadata for tests
    fn empty_flag_set_metadata() -> HashMap<String, Value> {
        HashMap::new()
    }

    #[test]
    fn test_static_result() {
        let flag = create_test_flag(None);
        let context = json!({});

        let result = evaluate_flag(&flag, &context, &empty_flag_set_metadata());
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

        let result = evaluate_flag(&flag, &context, &empty_flag_set_metadata());
        // Disabled flags return Disabled reason with FLAG_NOT_FOUND error code
        // Value is null to signal "use code default"
        assert_eq!(result.value, Value::Null);
        assert_eq!(result.variant, None);
        assert_eq!(result.reason, ResolutionReason::Disabled);
        assert_eq!(result.error_code, Some(ErrorCode::FlagNotFound));
        assert!(result.error_message.is_some());
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

        let result = evaluate_flag(&flag, &context, &empty_flag_set_metadata());
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

        let result = evaluate_flag(&flag, &context, &empty_flag_set_metadata());
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
            default_variant: Option::from("string_variant".to_string()),
            variants,
            targeting: Some(targeting),
            metadata: HashMap::new(),
        };

        // Test string variant
        let result = evaluate_flag(
            &flag,
            &json!({"variant_name": "string_variant"}),
            &empty_flag_set_metadata(),
        );
        assert_eq!(result.value, json!("hello"));

        // Test int variant
        let result = evaluate_flag(
            &flag,
            &json!({"variant_name": "int_variant"}),
            &empty_flag_set_metadata(),
        );
        assert_eq!(result.value, json!(42));

        // Test float variant
        let result = evaluate_flag(
            &flag,
            &json!({"variant_name": "float_variant"}),
            &empty_flag_set_metadata(),
        );
        assert_eq!(result.value, json!(3.14));

        // Test bool variant
        let result = evaluate_flag(
            &flag,
            &json!({"variant_name": "bool_variant"}),
            &empty_flag_set_metadata(),
        );
        assert_eq!(result.value, json!(true));

        // Test object variant
        let result = evaluate_flag(
            &flag,
            &json!({"variant_name": "object_variant"}),
            &empty_flag_set_metadata(),
        );
        assert_eq!(result.value, json!({"key": "value"}));
    }

    #[test]
    fn test_context_enrichment_with_flag_key() {
        let targeting = json!({
            "var": "$flagd.flagKey"
        });
        let flag = create_test_flag(Some(targeting));
        let context = json!({});

        let result = evaluate_flag(&flag, &context, &empty_flag_set_metadata());
        // The targeting rule returns the $flagd.flagKey variant name ("test_flag")
        // Since "test_flag" is not a valid variant, it should return an error (Java-compatible behavior)
        assert_eq!(result.reason, ResolutionReason::Error);
        assert_eq!(result.error_code, Some(ErrorCode::General));
        assert!(result.error_message.is_some());
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

        let result = evaluate_flag(&flag, &context, &empty_flag_set_metadata());
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

        let result = evaluate_flag(&flag, &context, &empty_flag_set_metadata());
        assert_eq!(result.value, json!(true));
        assert_eq!(result.variant, Some("on".to_string()));
    }

    #[test]
    fn test_context_enrichment_with_timestamp() {
        // Test that $flagd.timestamp is injected and is a valid unix timestamp
        let targeting = json!({
            "if": [
                {">": [{"var": "$flagd.timestamp"}, 0]},
                "on",
                "off"
            ]
        });
        let flag = create_test_flag(Some(targeting));
        let context = json!({});

        let result = evaluate_flag(&flag, &context, &empty_flag_set_metadata());
        // Should be "on" because timestamp should be > 0 (unless system time is before 1970)
        assert_eq!(result.value, json!(true));
        assert_eq!(result.variant, Some("on".to_string()));
        assert_eq!(result.reason, ResolutionReason::TargetingMatch);
    }

    #[test]
    fn test_context_enrichment_timestamp_is_numeric() {
        // Test that $flagd.timestamp is a number, not a string
        let targeting = json!({
            "var": "$flagd.timestamp"
        });

        let mut variants = HashMap::new();
        // Use numeric variants to verify timestamp is returned as a number
        variants.insert("timestamp".to_string(), json!(0));

        let flag = FeatureFlag {
            key: Some("test_flag".to_string()),
            state: "ENABLED".to_string(),
            default_variant: Option::from("timestamp".to_string()),
            variants,
            targeting: Some(targeting),
            metadata: HashMap::new(),
        };

        let context = json!({});
        let result = evaluate_flag(&flag, &context, &empty_flag_set_metadata());

        // The targeting returns a numeric timestamp which gets converted to a string
        // but won't match "timestamp" variant name, so it should return an error (Java-compatible behavior)
        assert_eq!(result.reason, ResolutionReason::Error);
        assert_eq!(result.error_code, Some(ErrorCode::General));
    }

    #[test]
    fn test_context_enrichment_all_flagd_properties() {
        // Test that both $flagd.flagKey and $flagd.timestamp are present
        let targeting = json!({
            "if": [
                {
                    "and": [
                        {"==": [{"var": "$flagd.flagKey"}, "test_flag"]},
                        {">": [{"var": "$flagd.timestamp"}, 0]}
                    ]
                },
                "success",
                "failure"
            ]
        });

        let mut variants = HashMap::new();
        variants.insert("success".to_string(), json!("both-present"));
        variants.insert("failure".to_string(), json!("missing-properties"));

        let flag = FeatureFlag {
            key: Some("test_flag".to_string()),
            state: "ENABLED".to_string(),
            default_variant: Option::from("failure".to_string()),
            variants,
            targeting: Some(targeting),
            metadata: HashMap::new(),
        };

        let context = json!({});
        let result = evaluate_flag(&flag, &context, &empty_flag_set_metadata());

        // Both conditions should be true, returning "success" variant
        assert_eq!(result.variant, Some("success".to_string()));
        assert_eq!(result.value, json!("both-present"));
        assert_eq!(result.reason, ResolutionReason::TargetingMatch);
    }

    #[test]
    fn test_flag_without_key_returns_error() {
        let mut flag = create_test_flag(None);
        flag.key = None; // Remove the key
        let context = json!({});

        let result = evaluate_flag(&flag, &context, &empty_flag_set_metadata());
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
            default_variant: Option::from("on".to_string()),
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
        let result = evaluate_flag(&flag, &context, &empty_flag_set_metadata());

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
            default_variant: Option::from("off".to_string()),
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
        let result = evaluate_flag(&flag, &context, &empty_flag_set_metadata());

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

        let result = evaluate_flag(&flag, &context, &empty_flag_set_metadata());
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

        let result = evaluate_flag(&flag, &json!({}), &empty_flag_set_metadata());
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

        let result = evaluate_flag(&flag, &json!({}), &empty_flag_set_metadata());

        // Disabled flags return Disabled reason with FLAG_NOT_FOUND error code
        assert_eq!(result.reason, ResolutionReason::Disabled);
        assert_eq!(result.error_code, Some(ErrorCode::FlagNotFound));
        // Metadata should still be included
        assert!(result.flag_metadata.is_some());
        assert_eq!(
            result.flag_metadata.as_ref().unwrap().get("reason"),
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

    // ============================================================================
    // Type-specific evaluation tests
    // ============================================================================

    #[test]
    fn test_evaluate_bool_flag_success() {
        let mut variants = HashMap::new();
        variants.insert("on".to_string(), json!(true));
        variants.insert("off".to_string(), json!(false));

        let flag = FeatureFlag {
            key: Some("bool_flag".to_string()),
            state: "ENABLED".to_string(),
            default_variant: Option::from("on".to_string()),
            variants,
            targeting: None,
            metadata: HashMap::new(),
        };

        let result = evaluate_bool_flag(&flag, &json!({}), &empty_flag_set_metadata());
        assert_eq!(result.value, json!(true));
        assert_eq!(result.reason, ResolutionReason::Static);
        assert!(result.error_code.is_none());
    }

    #[test]
    fn test_evaluate_bool_flag_type_mismatch() {
        let mut variants = HashMap::new();
        variants.insert("on".to_string(), json!("not_a_bool"));
        variants.insert("off".to_string(), json!(false));

        let flag = FeatureFlag {
            key: Some("bool_flag".to_string()),
            state: "ENABLED".to_string(),
            default_variant: Option::from("on".to_string()),
            variants,
            targeting: None,
            metadata: HashMap::new(),
        };

        let result = evaluate_bool_flag(&flag, &json!({}), &empty_flag_set_metadata());
        assert_eq!(result.reason, ResolutionReason::Error);
        assert_eq!(result.error_code, Some(ErrorCode::TypeMismatch));
        assert!(result
            .error_message
            .unwrap()
            .contains("Expected boolean, got string"));
    }

    #[test]
    fn test_evaluate_string_flag_success() {
        let mut variants = HashMap::new();
        variants.insert("red".to_string(), json!("crimson"));
        variants.insert("blue".to_string(), json!("azure"));

        let flag = FeatureFlag {
            key: Some("string_flag".to_string()),
            state: "ENABLED".to_string(),
            default_variant: Option::from("red".to_string()),
            variants,
            targeting: None,
            metadata: HashMap::new(),
        };

        let result = evaluate_string_flag(&flag, &json!({}), &empty_flag_set_metadata());
        assert_eq!(result.value, json!("crimson"));
        assert_eq!(result.reason, ResolutionReason::Static);
        assert!(result.error_code.is_none());
    }

    #[test]
    fn test_evaluate_string_flag_type_mismatch() {
        let mut variants = HashMap::new();
        variants.insert("red".to_string(), json!(123));
        variants.insert("blue".to_string(), json!("azure"));

        let flag = FeatureFlag {
            key: Some("string_flag".to_string()),
            state: "ENABLED".to_string(),
            default_variant: Option::from("red".to_string()),
            variants,
            targeting: None,
            metadata: HashMap::new(),
        };

        let result = evaluate_string_flag(&flag, &json!({}), &empty_flag_set_metadata());
        assert_eq!(result.reason, ResolutionReason::Error);
        assert_eq!(result.error_code, Some(ErrorCode::TypeMismatch));
        assert!(result
            .error_message
            .unwrap()
            .contains("Expected string, got integer"));
    }

    #[test]
    fn test_evaluate_int_flag_success() {
        let mut variants = HashMap::new();
        variants.insert("small".to_string(), json!(10));
        variants.insert("large".to_string(), json!(100));

        let flag = FeatureFlag {
            key: Some("int_flag".to_string()),
            state: "ENABLED".to_string(),
            default_variant: Option::from("small".to_string()),
            variants,
            targeting: None,
            metadata: HashMap::new(),
        };

        let result = evaluate_int_flag(&flag, &json!({}), &empty_flag_set_metadata());
        assert_eq!(result.value, json!(10));
        assert_eq!(result.reason, ResolutionReason::Static);
        assert!(result.error_code.is_none());
    }

    #[test]
    fn test_evaluate_int_flag_type_mismatch_float() {
        let mut variants = HashMap::new();
        variants.insert("small".to_string(), json!(3.14));
        variants.insert("large".to_string(), json!(100));

        let flag = FeatureFlag {
            key: Some("int_flag".to_string()),
            state: "ENABLED".to_string(),
            default_variant: Option::from("small".to_string()),
            variants,
            targeting: None,
            metadata: HashMap::new(),
        };

        let result = evaluate_int_flag(&flag, &json!({}), &empty_flag_set_metadata());
        // Float is coerced to integer (Java-compatible behavior)
        // 3.14 becomes 3
        assert_eq!(result.value, json!(3));
        assert_eq!(result.reason, ResolutionReason::Static);
        assert!(result.error_code.is_none());
    }

    #[test]
    fn test_evaluate_int_flag_type_mismatch_string() {
        let mut variants = HashMap::new();
        variants.insert("small".to_string(), json!("10"));
        variants.insert("large".to_string(), json!(100));

        let flag = FeatureFlag {
            key: Some("int_flag".to_string()),
            state: "ENABLED".to_string(),
            default_variant: Option::from("small".to_string()),
            variants,
            targeting: None,
            metadata: HashMap::new(),
        };

        let result = evaluate_int_flag(&flag, &json!({}), &empty_flag_set_metadata());
        assert_eq!(result.reason, ResolutionReason::Error);
        assert_eq!(result.error_code, Some(ErrorCode::TypeMismatch));
        assert!(result
            .error_message
            .unwrap()
            .contains("Expected integer, got string"));
    }

    #[test]
    fn test_evaluate_float_flag_success_with_float() {
        let mut variants = HashMap::new();
        variants.insert("low".to_string(), json!(1.5));
        variants.insert("high".to_string(), json!(9.99));

        let flag = FeatureFlag {
            key: Some("float_flag".to_string()),
            state: "ENABLED".to_string(),
            default_variant: Option::from("low".to_string()),
            variants,
            targeting: None,
            metadata: HashMap::new(),
        };

        let result = evaluate_float_flag(&flag, &json!({}), &empty_flag_set_metadata());
        assert_eq!(result.value, json!(1.5));
        assert_eq!(result.reason, ResolutionReason::Static);
        assert!(result.error_code.is_none());
    }

    #[test]
    fn test_evaluate_float_flag_success_with_int() {
        // Float evaluation should accept integers and coerce them to floats
        let mut variants = HashMap::new();
        variants.insert("low".to_string(), json!(1));
        variants.insert("high".to_string(), json!(10));

        let flag = FeatureFlag {
            key: Some("float_flag".to_string()),
            state: "ENABLED".to_string(),
            default_variant: Option::from("low".to_string()),
            variants,
            targeting: None,
            metadata: HashMap::new(),
        };

        let result = evaluate_float_flag(&flag, &json!({}), &empty_flag_set_metadata());
        // Integer is coerced to float (Java-compatible behavior)
        assert_eq!(result.value, json!(1.0));
        assert_eq!(result.reason, ResolutionReason::Static);
        assert!(result.error_code.is_none());
    }

    #[test]
    fn test_evaluate_float_flag_type_mismatch() {
        let mut variants = HashMap::new();
        variants.insert("low".to_string(), json!("not_a_number"));
        variants.insert("high".to_string(), json!(9.99));

        let flag = FeatureFlag {
            key: Some("float_flag".to_string()),
            state: "ENABLED".to_string(),
            default_variant: Option::from("low".to_string()),
            variants,
            targeting: None,
            metadata: HashMap::new(),
        };

        let result = evaluate_float_flag(&flag, &json!({}), &empty_flag_set_metadata());
        assert_eq!(result.reason, ResolutionReason::Error);
        assert_eq!(result.error_code, Some(ErrorCode::TypeMismatch));
        assert!(result
            .error_message
            .unwrap()
            .contains("Expected float, got string"));
    }

    #[test]
    fn test_evaluate_object_flag_success() {
        let mut variants = HashMap::new();
        variants.insert("config1".to_string(), json!({"timeout": 30, "retries": 3}));
        variants.insert("config2".to_string(), json!({"timeout": 60, "retries": 5}));

        let flag = FeatureFlag {
            key: Some("object_flag".to_string()),
            state: "ENABLED".to_string(),
            default_variant: Option::from("config1".to_string()),
            variants,
            targeting: None,
            metadata: HashMap::new(),
        };

        let result = evaluate_object_flag(&flag, &json!({}), &empty_flag_set_metadata());
        assert_eq!(result.value, json!({"timeout": 30, "retries": 3}));
        assert_eq!(result.reason, ResolutionReason::Static);
        assert!(result.error_code.is_none());
    }

    #[test]
    fn test_evaluate_object_flag_type_mismatch() {
        let mut variants = HashMap::new();
        variants.insert("config1".to_string(), json!("not_an_object"));
        variants.insert("config2".to_string(), json!({"timeout": 60, "retries": 5}));

        let flag = FeatureFlag {
            key: Some("object_flag".to_string()),
            state: "ENABLED".to_string(),
            default_variant: Option::from("config1".to_string()),
            variants,
            targeting: None,
            metadata: HashMap::new(),
        };

        let result = evaluate_object_flag(&flag, &json!({}), &empty_flag_set_metadata());
        assert_eq!(result.reason, ResolutionReason::Error);
        assert_eq!(result.error_code, Some(ErrorCode::TypeMismatch));
        assert!(result
            .error_message
            .unwrap()
            .contains("Expected object, got string"));
    }

    #[test]
    fn test_evaluate_bool_flag_preserves_error() {
        // When the flag itself has an error (e.g., not found), type checking should not be applied
        let mut flag = create_test_flag(None);
        flag.key = None; // This will cause an error

        let result = evaluate_bool_flag(&flag, &json!({}), &empty_flag_set_metadata());
        assert_eq!(result.reason, ResolutionReason::Error);
        assert_eq!(result.error_code, Some(ErrorCode::General));
        // Error should be about the flag key, not about type mismatch
        assert!(result.error_message.unwrap().contains("Flag key not set"));
    }

    #[test]
    fn test_evaluate_string_flag_with_targeting() {
        let mut variants = HashMap::new();
        variants.insert("admin".to_string(), json!("admin_message"));
        variants.insert("user".to_string(), json!("user_message"));

        let targeting = json!({
            "if": [
                {"==": [{"var": "role"}, "admin"]},
                "admin",
                "user"
            ]
        });

        let flag = FeatureFlag {
            key: Some("message_flag".to_string()),
            state: "ENABLED".to_string(),
            default_variant: Option::from("user".to_string()),
            variants,
            targeting: Some(targeting),
            metadata: HashMap::new(),
        };

        let result =
            evaluate_string_flag(&flag, &json!({"role": "admin"}), &empty_flag_set_metadata());
        assert_eq!(result.value, json!("admin_message"));
        assert_eq!(result.reason, ResolutionReason::TargetingMatch);
        assert!(result.error_code.is_none());
    }

    #[test]
    fn test_evaluate_int_flag_with_disabled_state() {
        let mut variants = HashMap::new();
        variants.insert("small".to_string(), json!(10));
        variants.insert("large".to_string(), json!(100));

        let flag = FeatureFlag {
            key: Some("int_flag".to_string()),
            state: "DISABLED".to_string(),
            default_variant: Option::from("small".to_string()),
            variants,
            targeting: None,
            metadata: HashMap::new(),
        };

        let result = evaluate_int_flag(&flag, &json!({}), &empty_flag_set_metadata());
        // Disabled flags return Disabled reason with FLAG_NOT_FOUND error code
        assert_eq!(result.value, Value::Null);
        assert_eq!(result.reason, ResolutionReason::Disabled);
        assert_eq!(result.error_code, Some(ErrorCode::FlagNotFound));
    }

    #[test]
    fn test_type_name_helper() {
        assert_eq!(type_name(&json!(null)), "null");
        assert_eq!(type_name(&json!(true)), "boolean");
        assert_eq!(type_name(&json!(false)), "boolean");
        assert_eq!(type_name(&json!(42)), "integer");
        assert_eq!(type_name(&json!(3.14)), "float");
        assert_eq!(type_name(&json!("hello")), "string");
        assert_eq!(type_name(&json!([1, 2, 3])), "array");
        assert_eq!(type_name(&json!({"key": "value"})), "object");
    }

    #[test]
    fn test_evaluate_object_flag_with_array_type_mismatch() {
        let mut variants = HashMap::new();
        variants.insert("config1".to_string(), json!([1, 2, 3]));
        variants.insert("config2".to_string(), json!({"timeout": 60, "retries": 5}));

        let flag = FeatureFlag {
            key: Some("object_flag".to_string()),
            state: "ENABLED".to_string(),
            default_variant: Option::from("config1".to_string()),
            variants,
            targeting: None,
            metadata: HashMap::new(),
        };

        let result = evaluate_object_flag(&flag, &json!({}), &empty_flag_set_metadata());
        assert_eq!(result.reason, ResolutionReason::Error);
        assert_eq!(result.error_code, Some(ErrorCode::TypeMismatch));
        assert!(result
            .error_message
            .unwrap()
            .contains("Expected object, got array"));
    }

    #[test]
    fn test_all_typed_evaluators_with_complex_targeting() {
        // Test that all type-specific evaluators work correctly with targeting rules

        // Boolean flag with targeting
        let bool_targeting = json!({
            "if": [{">=": [{"var": "score"}, 80]}, "on", "off"]
        });
        let mut bool_variants = HashMap::new();
        bool_variants.insert("on".to_string(), json!(true));
        bool_variants.insert("off".to_string(), json!(false));
        let bool_flag = FeatureFlag {
            key: Some("bool_flag".to_string()),
            state: "ENABLED".to_string(),
            default_variant: Option::from("off".to_string()),
            variants: bool_variants,
            targeting: Some(bool_targeting),
            metadata: HashMap::new(),
        };
        let bool_result = evaluate_bool_flag(
            &bool_flag,
            &json!({"score": 90}),
            &empty_flag_set_metadata(),
        );
        assert_eq!(bool_result.value, json!(true));
        assert_eq!(bool_result.reason, ResolutionReason::TargetingMatch);

        // String flag with targeting
        let string_targeting = json!({
            "if": [{"==": [{"var": "tier"}, "premium"]}, "gold", "silver"]
        });
        let mut string_variants = HashMap::new();
        string_variants.insert("gold".to_string(), json!("gold_tier"));
        string_variants.insert("silver".to_string(), json!("silver_tier"));
        let string_flag = FeatureFlag {
            key: Some("string_flag".to_string()),
            state: "ENABLED".to_string(),
            default_variant: Option::from("silver".to_string()),
            variants: string_variants,
            targeting: Some(string_targeting),
            metadata: HashMap::new(),
        };
        let string_result = evaluate_string_flag(
            &string_flag,
            &json!({"tier": "premium"}),
            &empty_flag_set_metadata(),
        );
        assert_eq!(string_result.value, json!("gold_tier"));
        assert_eq!(string_result.reason, ResolutionReason::TargetingMatch);

        // Integer flag with targeting
        let int_targeting = json!({
            "if": [{"<": [{"var": "age"}, 18]}, "minor", "adult"]
        });
        let mut int_variants = HashMap::new();
        int_variants.insert("minor".to_string(), json!(10));
        int_variants.insert("adult".to_string(), json!(100));
        let int_flag = FeatureFlag {
            key: Some("int_flag".to_string()),
            state: "ENABLED".to_string(),
            default_variant: Option::from("adult".to_string()),
            variants: int_variants,
            targeting: Some(int_targeting),
            metadata: HashMap::new(),
        };
        let int_result =
            evaluate_int_flag(&int_flag, &json!({"age": 15}), &empty_flag_set_metadata());
        assert_eq!(int_result.value, json!(10));
        assert_eq!(int_result.reason, ResolutionReason::TargetingMatch);
    }
}
