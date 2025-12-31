//! Evaluation result types for feature flag evaluation.
//!
//! This module provides the data structures for representing evaluation results
//! according to the flagd provider specification.

use serde::{Deserialize, Serialize};
use serde_json::Value;
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
    pub flag_metadata: Option<HashMap<String, Value>>,
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
    pub fn with_metadata(mut self, metadata: HashMap<String, Value>) -> Self {
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_static_result() {
        let result = EvaluationResult::static_result(json!(true), "on".to_string());
        assert_eq!(result.value, json!(true));
        assert_eq!(result.variant, Some("on".to_string()));
        assert_eq!(result.reason, ResolutionReason::Static);
        assert!(result.error_code.is_none());
    }

    #[test]
    fn test_default_result() {
        let result = EvaluationResult::default_result(json!(false), "off".to_string());
        assert_eq!(result.value, json!(false));
        assert_eq!(result.variant, Some("off".to_string()));
        assert_eq!(result.reason, ResolutionReason::Default);
    }

    #[test]
    fn test_targeting_match() {
        let result = EvaluationResult::targeting_match(json!("value"), "variant".to_string());
        assert_eq!(result.value, json!("value"));
        assert_eq!(result.variant, Some("variant".to_string()));
        assert_eq!(result.reason, ResolutionReason::TargetingMatch);
    }

    #[test]
    fn test_disabled() {
        let result = EvaluationResult::disabled(json!(null), "default".to_string());
        assert_eq!(result.value, json!(null));
        assert_eq!(result.reason, ResolutionReason::Disabled);
    }

    #[test]
    fn test_error() {
        let result = EvaluationResult::error(ErrorCode::ParseError, "Invalid rule");
        assert_eq!(result.value, Value::Null);
        assert_eq!(result.reason, ResolutionReason::Error);
        assert_eq!(result.error_code, Some(ErrorCode::ParseError));
        assert_eq!(result.error_message, Some("Invalid rule".to_string()));
    }

    #[test]
    fn test_flag_not_found() {
        let result = EvaluationResult::flag_not_found("missing-flag");
        assert_eq!(result.reason, ResolutionReason::FlagNotFound);
        assert_eq!(result.error_code, Some(ErrorCode::FlagNotFound));
        assert!(result.error_message.unwrap().contains("missing-flag"));
    }

    #[test]
    fn test_fallback() {
        let result = EvaluationResult::fallback("my-flag");
        assert_eq!(result.reason, ResolutionReason::Fallback);
        assert_eq!(result.error_code, Some(ErrorCode::FlagNotFound));
        assert!(result.error_message.unwrap().contains("my-flag"));
    }

    #[test]
    fn test_with_metadata() {
        let mut metadata = HashMap::new();
        metadata.insert("key".to_string(), json!("value"));

        let result =
            EvaluationResult::static_result(json!(true), "on".to_string()).with_metadata(metadata);
        assert!(result.flag_metadata.is_some());
        assert_eq!(
            result.flag_metadata.unwrap().get("key"),
            Some(&json!("value"))
        );
    }

    #[test]
    fn test_to_json_string() {
        let result = EvaluationResult::static_result(json!(42), "variant".to_string());
        let json_str = result.to_json_string();
        assert!(json_str.contains("\"value\":42"));
        assert!(json_str.contains("\"variant\":\"variant\""));
        assert!(json_str.contains("\"reason\":\"STATIC\""));
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
            (EvaluationResult::fallback("test"), "FALLBACK"),
        ];

        for (result, expected_reason) in test_cases {
            let json_str = result.to_json_string();
            let parsed: Value = serde_json::from_str(&json_str).unwrap();
            assert_eq!(parsed["reason"], expected_reason);
        }
    }
}
