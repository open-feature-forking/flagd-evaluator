//! Instance-based flag evaluator for Rust library usage.
//!
//! This module provides a `FlagEvaluator` struct that manages flag state
//! and validation mode per-instance, allowing multiple independent evaluators
//! in the same process without global state issues.

use crate::model::{FeatureFlag, ParsingResult, UpdateStateResponse};
use crate::operators::create_evaluator;
use crate::types::{ErrorCode, EvaluationResult, ResolutionReason};
use crate::validation::validate_flags_config;
use datalogic_rs::DataLogic;
use serde_json::{Map, Value as JsonValue, Value};
use std::collections::{HashMap, HashSet};

/// Validation mode determines how validation errors are handled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationMode {
    /// Reject invalid flag configurations (default, strict mode)
    Strict,
    /// Accept invalid flag configurations with warnings (permissive mode)
    Permissive,
}

/// Instance-based flag evaluator.
///
/// This struct holds flag configuration and validation mode, allowing
/// multiple independent evaluators without global state conflicts.
///
/// # Example
///
/// ```
/// use flagd_evaluator::FlagEvaluator;
/// use flagd_evaluator::ValidationMode;
///
/// let mut evaluator = FlagEvaluator::new(ValidationMode::Permissive);
///
/// let config = r#"{
///     "flags": {
///         "myFlag": {
///             "state": "ENABLED",
///             "defaultVariant": "on",
///             "variants": {
///                 "on": true,
///                 "off": false
///             }
///         }
///     }
/// }"#;
///
/// evaluator.update_state(config).unwrap();
/// ```
pub struct FlagEvaluator {
    state: Option<ParsingResult>,
    validation_mode: ValidationMode,
    /// The DataLogic engine with custom operators (created once, reused for all evaluations)
    logic: DataLogic,
}

impl std::fmt::Debug for FlagEvaluator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FlagEvaluator")
            .field("state", &self.state)
            .field("validation_mode", &self.validation_mode)
            .field("logic", &"<DataLogic>")
            .finish()
    }
}

impl FlagEvaluator {
    /// Creates a new flag evaluator with the specified validation mode.
    ///
    /// # Arguments
    ///
    /// * `validation_mode` - The validation mode to use for this evaluator
    pub fn new(validation_mode: ValidationMode) -> Self {
        Self {
            state: None,
            validation_mode,
            logic: create_evaluator(),
        }
    }

    /// Gets a reference to the DataLogic engine.
    /// This allows reusing the engine without recreation overhead.
    pub fn logic(&self) -> &DataLogic {
        &self.logic
    }

    /// Updates the flag state with a new configuration.
    ///
    /// This validates and parses the provided JSON configuration, then stores it.
    /// The behavior when validation fails depends on the evaluator's validation mode.
    ///
    /// # Arguments
    ///
    /// * `json_config` - JSON string containing the flag configuration
    ///
    /// # Returns
    ///
    /// * `Ok(UpdateStateResponse)` - If successful, with changed flag keys
    /// * `Err(String)` - If there was an error
    pub fn update_state(&mut self, json_config: &str) -> Result<UpdateStateResponse, String> {
        // Validate the configuration
        let validation_result = validate_flags_config(json_config);

        match self.validation_mode {
            ValidationMode::Strict => {
                if let Err(validation_error) = validation_result {
                    return Ok(UpdateStateResponse {
                        success: false,
                        error: Some(validation_error.to_json_string()),
                        changed_flags: None,
                    });
                }
            }
            ValidationMode::Permissive => {
                if let Err(validation_error) = validation_result {
                    eprintln!(
                        "Warning: Configuration has validation errors: {}",
                        validation_error.to_json_string()
                    );
                }
            }
        }

        // Parse the configuration
        let new_parsing_result = match ParsingResult::parse(json_config) {
            Ok(result) => result,
            Err(e) => {
                return Ok(UpdateStateResponse {
                    success: false,
                    error: Some(e),
                    changed_flags: None,
                });
            }
        };

        // Detect changed flags
        let changed_flags = self.detect_changed_flags(&new_parsing_result);

        // Store the new state
        self.state = Some(new_parsing_result);

        Ok(UpdateStateResponse {
            success: true,
            error: None,
            changed_flags: Some(changed_flags),
        })
    }

    /// Gets a reference to the current flag state.
    pub fn get_state(&self) -> Option<&ParsingResult> {
        self.state.as_ref()
    }

    /// Gets the validation mode for this evaluator.
    pub fn validation_mode(&self) -> ValidationMode {
        self.validation_mode
    }

    /// Sets the validation mode for this evaluator.
    ///
    /// This affects how subsequent `update_state` calls will handle validation errors.
    pub fn set_validation_mode(&mut self, mode: ValidationMode) {
        self.validation_mode = mode;
    }

    /// Clears the flag state.
    pub fn clear_state(&mut self) {
        self.state = None;
    }

    // =========================================================================
    // Core evaluation methods
    // =========================================================================

    /// Evaluates a feature flag against a context.
    ///
    /// This is the main evaluation method that handles all flag types.
    ///
    /// # Arguments
    /// * `flag_key` - The key of the flag to evaluate
    /// * `context` - The evaluation context (JSON object)
    ///
    /// # Returns
    /// An EvaluationResult containing the resolved value, variant, reason, and metadata
    pub fn evaluate_flag(&self, flag_key: &str, context: &JsonValue) -> EvaluationResult {
        self.evaluate_with_type_check(flag_key, context, None)
    }

    /// Evaluates a boolean flag with type checking.
    pub fn evaluate_bool(&self, flag_key: &str, context: &JsonValue) -> EvaluationResult {
        self.evaluate_with_type_check(flag_key, context, Some(ExpectedType::Boolean))
    }

    /// Evaluates a string flag with type checking.
    pub fn evaluate_string(&self, flag_key: &str, context: &JsonValue) -> EvaluationResult {
        self.evaluate_with_type_check(flag_key, context, Some(ExpectedType::String))
    }

    /// Evaluates an integer flag with type checking.
    pub fn evaluate_int(&self, flag_key: &str, context: &JsonValue) -> EvaluationResult {
        self.evaluate_with_type_check(flag_key, context, Some(ExpectedType::Integer))
    }

    /// Evaluates a float flag with type checking.
    pub fn evaluate_float(&self, flag_key: &str, context: &JsonValue) -> EvaluationResult {
        self.evaluate_with_type_check(flag_key, context, Some(ExpectedType::Float))
    }

    /// Evaluates an object flag with type checking.
    pub fn evaluate_object(&self, flag_key: &str, context: &JsonValue) -> EvaluationResult {
        self.evaluate_with_type_check(flag_key, context, Some(ExpectedType::Object))
    }

    // =========================================================================
    // Internal evaluation logic
    // =========================================================================

    /// Internal method that handles evaluation with optional type checking.
    fn evaluate_with_type_check(
        &self,
        flag_key: &str,
        context: &JsonValue,
        expected_type: Option<ExpectedType>,
    ) -> EvaluationResult {
        // Get flag and metadata from state - avoid cloning the flag!
        let state = match &self.state {
            Some(s) => s,
            None => {
                return EvaluationResult::error(ErrorCode::General, "No flag configuration loaded");
            }
        };

        // Look up flag by reference (no clone!)
        let flag = match state.flags.get(flag_key) {
            Some(f) => f,
            None => {
                // Flag not found - return flag-set metadata per spec (best effort)
                let flag_set_metadata =
                    Self::merge_metadata(&state.flag_set_metadata, &HashMap::new());
                return EvaluationResult {
                    value: JsonValue::Null,
                    variant: None,
                    reason: ResolutionReason::FlagNotFound,
                    error_code: Some(ErrorCode::FlagNotFound),
                    error_message: Some(format!("Flag '{}' not found in configuration", flag_key)),
                    flag_metadata: flag_set_metadata,
                };
            }
        };

        // Perform the evaluation with reference to flag
        let result = self.evaluate_flag_internal(flag, flag_key, context, &state.flag_set_metadata);

        // Apply type checking if requested
        match expected_type {
            Some(expected) => self.apply_type_check(result, expected),
            None => result,
        }
    }

    /// Core flag evaluation logic.
    fn evaluate_flag_internal(
        &self,
        flag: &FeatureFlag,
        flag_key: &str,
        context: &JsonValue,
        flag_set_metadata: &HashMap<String, JsonValue>,
    ) -> EvaluationResult {
        // Check if flag is disabled - still return metadata per spec
        if flag.state == "DISABLED" {
            let merged_metadata = Self::merge_metadata(flag_set_metadata, &flag.metadata);
            return EvaluationResult {
                value: JsonValue::Null,
                variant: None,
                reason: ResolutionReason::Disabled,
                error_code: Some(ErrorCode::FlagNotFound),
                error_message: Some(format!("flag: {} is disabled", flag_key)),
                flag_metadata: merged_metadata,
            };
        }

        // Check if there's no targeting rule or if it's an empty object "{}"
        let is_empty_targeting = match &flag.targeting {
            None => true,
            Some(JsonValue::Object(map)) if map.is_empty() => true,
            _ => false,
        };

        if is_empty_targeting {
            return match flag.default_variant.as_ref() {
                None => EvaluationResult::fallback(flag_key),
                Some(value) if value.is_empty() => EvaluationResult::fallback(flag_key),
                Some(default_variant) => match flag.variants.get(default_variant) {
                    Some(value) => {
                        let result =
                            EvaluationResult::static_result(value.clone(), default_variant.clone());
                        // Lazy metadata: only merge if there's actually metadata
                        Self::with_lazy_metadata(flag_set_metadata, &flag.metadata, result)
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

        // Enrich the context with flagKey and targetingKey
        let enriched_context = Self::enrich_context(flag_key, context);

        // Evaluate targeting using the instance's DataLogic engine
        let eval_result = if let Some(ref compiled) = flag.compiled_targeting {
            // Fast path: use pre-compiled targeting with evaluate_owned (no JSON serialization)
            self.logic.evaluate_owned(compiled, enriched_context)
        } else {
            // Fallback: compile at runtime (for flags created without pre-compilation)
            let targeting = flag.targeting.as_ref().unwrap();
            let rule_str = targeting.to_string();
            let context_str = enriched_context.to_string();
            self.logic.evaluate_json(&rule_str, &context_str)
        };

        match eval_result {
            Ok(result) => {
                // Check if targeting returned null - this means use default variant
                if result.is_null() {
                    return match flag.default_variant.as_ref() {
                        None => EvaluationResult::fallback(flag_key),
                        Some(value) if value.is_empty() => EvaluationResult::fallback(flag_key),
                        Some(default_variant) => match flag.variants.get(default_variant) {
                            Some(value) => {
                                let result = EvaluationResult::default_result(
                                    value.clone(),
                                    default_variant.clone(),
                                );
                                Self::with_lazy_metadata(flag_set_metadata, &flag.metadata, result)
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
                // Optimization: avoid clone if result is already a String
                let variant_name = match result {
                    JsonValue::String(s) => s,
                    other => match other.as_str() {
                        Some(s) => s.to_string(),
                        None => other.to_string().trim_matches('"').to_string(),
                    },
                };

                // Check for empty variant name
                if variant_name.is_empty() {
                    return match flag.default_variant.as_ref() {
                        None => EvaluationResult::fallback(flag_key),
                        Some(default_variant) if default_variant.is_empty() => {
                            EvaluationResult::fallback(flag_key)
                        }
                        Some(_) => EvaluationResult::error(
                            ErrorCode::General,
                            format!(
                                "Targeting rule returned empty variant name for flag '{}'",
                                flag_key
                            ),
                        ),
                    };
                }

                // Look up the variant value
                match flag.variants.get(&variant_name) {
                    Some(value) => {
                        let result = EvaluationResult::targeting_match(value.clone(), variant_name);
                        Self::with_lazy_metadata(flag_set_metadata, &flag.metadata, result)
                    }
                    None => EvaluationResult::error(
                        ErrorCode::General,
                        format!(
                            "Targeting rule returned variant '{}' which is not defined in flag variants",
                            variant_name
                        ),
                    ),
                }
            }
            Err(e) => {
                EvaluationResult::error(ErrorCode::ParseError, format!("Evaluation error: {}", e))
            }
        }
    }

    /// Applies type checking to an evaluation result.
    fn apply_type_check(
        &self,
        mut result: EvaluationResult,
        expected: ExpectedType,
    ) -> EvaluationResult {
        // If there's already an error or special status, return it as-is
        if result.reason == ResolutionReason::Error
            || result.reason == ResolutionReason::FlagNotFound
            || result.reason == ResolutionReason::Fallback
            || result.reason == ResolutionReason::Disabled
        {
            return result;
        }

        match expected {
            ExpectedType::Boolean => {
                if result.value.is_boolean() {
                    result
                } else {
                    EvaluationResult::error(
                        ErrorCode::TypeMismatch,
                        format!(
                            "Flag value has incorrect type. Expected boolean, got {}",
                            Self::type_name(&result.value)
                        ),
                    )
                }
            }
            ExpectedType::String => {
                if result.value.is_string() {
                    result
                } else {
                    EvaluationResult::error(
                        ErrorCode::TypeMismatch,
                        format!(
                            "Flag value has incorrect type. Expected string, got {}",
                            Self::type_name(&result.value)
                        ),
                    )
                }
            }
            ExpectedType::Integer => {
                // Type coercion: float to integer (Java-compatible behavior)
                if result.value.is_f64() {
                    if let Some(f) = result.value.as_f64() {
                        result.value = JsonValue::Number(serde_json::Number::from(f as i64));
                        return result;
                    }
                }
                if result.value.is_i64() || result.value.is_u64() {
                    result
                } else {
                    EvaluationResult::error(
                        ErrorCode::TypeMismatch,
                        format!(
                            "Flag value has incorrect type. Expected integer, got {}",
                            Self::type_name(&result.value)
                        ),
                    )
                }
            }
            ExpectedType::Float => {
                // Type coercion: integer to float (Java-compatible behavior)
                if result.value.is_i64() || result.value.is_u64() {
                    if let Some(i) = result.value.as_i64() {
                        if let Some(num) = serde_json::Number::from_f64(i as f64) {
                            result.value = JsonValue::Number(num);
                        }
                    } else if let Some(u) = result.value.as_u64() {
                        if let Some(num) = serde_json::Number::from_f64(u as f64) {
                            result.value = JsonValue::Number(num);
                        }
                    }
                    return result;
                }
                if result.value.is_number() {
                    result
                } else {
                    EvaluationResult::error(
                        ErrorCode::TypeMismatch,
                        format!(
                            "Flag value has incorrect type. Expected float, got {}",
                            Self::type_name(&result.value)
                        ),
                    )
                }
            }
            ExpectedType::Object => {
                if result.value.is_object() {
                    result
                } else {
                    EvaluationResult::error(
                        ErrorCode::TypeMismatch,
                        format!(
                            "Flag value has incorrect type. Expected object, got {}",
                            Self::type_name(&result.value)
                        ),
                    )
                }
            }
        }
    }

    // =========================================================================
    // Helper methods
    // =========================================================================

    /// Detects which flags have changed between the current and new state.
    fn detect_changed_flags(&self, new_state: &ParsingResult) -> Vec<String> {
        let mut changed_keys = HashSet::new();

        match &self.state {
            None => {
                // No previous state, all flags are new
                for key in new_state.flags.keys() {
                    changed_keys.insert(key.clone());
                }
            }
            Some(old) => {
                // Check for added and mutated flags
                for (key, new_flag) in &new_state.flags {
                    match old.flags.get(key) {
                        None => {
                            changed_keys.insert(key.clone());
                        }
                        Some(old_flag) => {
                            if new_flag.is_different_from(old_flag) {
                                changed_keys.insert(key.clone());
                            }
                        }
                    }
                }

                // Check for removed flags
                for key in old.flags.keys() {
                    if !new_state.flags.contains_key(key) {
                        changed_keys.insert(key.clone());
                    }
                }
            }
        }

        let mut result: Vec<String> = changed_keys.into_iter().collect();
        result.sort();
        result
    }

    /// Enriches the evaluation context with standard flagd fields.
    fn enrich_context(flag_key: &str, context: &Value) -> Value {
        let mut enriched = if let Some(obj) = context.as_object() {
            obj.clone()
        } else {
            Map::new()
        };

        // Get current Unix timestamp (seconds since epoch)
        let timestamp = crate::get_current_time();

        // Create $flagd object with nested properties
        let mut flagd_props = Map::new();
        flagd_props.insert("flagKey".to_string(), Value::String(flag_key.to_string()));
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
    fn merge_metadata(
        flag_set_metadata: &HashMap<String, JsonValue>,
        flag_metadata: &HashMap<String, JsonValue>,
    ) -> Option<HashMap<String, JsonValue>> {
        // Filter out internal fields (those starting with $) from flag-set metadata
        let filtered_flag_set: HashMap<String, JsonValue> = flag_set_metadata
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

    /// Lazy metadata attachment - only merges metadata if there's actually metadata to merge.
    /// This avoids the cost of creating HashMaps when both sources are empty.
    fn with_lazy_metadata(
        flag_set_metadata: &HashMap<String, JsonValue>,
        flag_metadata: &HashMap<String, JsonValue>,
        result: EvaluationResult,
    ) -> EvaluationResult {
        // Fast path: if both are empty, skip merging entirely
        if flag_set_metadata.is_empty() && flag_metadata.is_empty() {
            return result;
        }

        // Only merge if there's actual metadata
        match Self::merge_metadata(flag_set_metadata, flag_metadata) {
            Some(metadata) => result.with_metadata(metadata),
            None => result,
        }
    }

    /// Helper function to get a human-readable type name from a JSON value.
    fn type_name(value: &JsonValue) -> &'static str {
        match value {
            JsonValue::Null => "null",
            JsonValue::Bool(_) => "boolean",
            JsonValue::Number(n) => {
                if n.is_i64() || n.is_u64() {
                    "integer"
                } else {
                    "float"
                }
            }
            JsonValue::String(_) => "string",
            JsonValue::Array(_) => "array",
            JsonValue::Object(_) => "object",
        }
    }
}

impl Default for FlagEvaluator {
    fn default() -> Self {
        Self::new(ValidationMode::Strict)
    }
}

/// Expected type for type-checked evaluation.
#[derive(Debug, Clone, Copy)]
enum ExpectedType {
    Boolean,
    String,
    Integer,
    Float,
    Object,
}
