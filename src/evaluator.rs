//! Instance-based flag evaluator for Rust library usage.
//!
//! This module provides a `FlagEvaluator` struct that manages flag state
//! and validation mode per-instance, allowing multiple independent evaluators
//! in the same process without global state issues.

use crate::evaluation::EvaluationResult;
use crate::model::{ParsingResult, UpdateStateResponse};
use crate::validation::validate_flags_config;
use serde_json::Value as JsonValue;
use std::collections::HashSet;

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
/// use flagd_evaluator::storage::ValidationMode;
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
#[derive(Debug)]
pub struct FlagEvaluator {
    state: Option<ParsingResult>,
    validation_mode: ValidationMode,
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
        }
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

    /// Evaluates a boolean flag.
    pub fn evaluate_bool(&self, flag_key: &str, context: &JsonValue) -> EvaluationResult {
        match &self.state {
            Some(state) => {
                let flag = state
                    .flags
                    .get(flag_key)
                    .cloned()
                    .unwrap_or_else(|| self.empty_flag(flag_key));
                crate::evaluation::evaluate_bool_flag(&flag, context, &state.flag_set_metadata)
            }
            None => EvaluationResult {
                value: JsonValue::Bool(false),
                variant: None,
                reason: crate::evaluation::ResolutionReason::Error,
                error_code: Some(crate::evaluation::ErrorCode::General),
                error_message: Some("No flag configuration loaded".to_string()),
                flag_metadata: None,
            },
        }
    }

    /// Evaluates a string flag.
    pub fn evaluate_string(&self, flag_key: &str, context: &JsonValue) -> EvaluationResult {
        match &self.state {
            Some(state) => {
                let flag = state
                    .flags
                    .get(flag_key)
                    .cloned()
                    .unwrap_or_else(|| self.empty_flag(flag_key));
                crate::evaluation::evaluate_string_flag(&flag, context, &state.flag_set_metadata)
            }
            None => EvaluationResult {
                value: JsonValue::String(String::new()),
                variant: None,
                reason: crate::evaluation::ResolutionReason::Error,
                error_code: Some(crate::evaluation::ErrorCode::General),
                error_message: Some("No flag configuration loaded".to_string()),
                flag_metadata: None,
            },
        }
    }

    /// Evaluates an integer flag.
    pub fn evaluate_int(&self, flag_key: &str, context: &JsonValue) -> EvaluationResult {
        match &self.state {
            Some(state) => {
                let flag = state
                    .flags
                    .get(flag_key)
                    .cloned()
                    .unwrap_or_else(|| self.empty_flag(flag_key));
                crate::evaluation::evaluate_int_flag(&flag, context, &state.flag_set_metadata)
            }
            None => EvaluationResult {
                value: JsonValue::Number(0.into()),
                variant: None,
                reason: crate::evaluation::ResolutionReason::Error,
                error_code: Some(crate::evaluation::ErrorCode::General),
                error_message: Some("No flag configuration loaded".to_string()),
                flag_metadata: None,
            },
        }
    }

    /// Evaluates a float flag.
    pub fn evaluate_float(&self, flag_key: &str, context: &JsonValue) -> EvaluationResult {
        match &self.state {
            Some(state) => {
                let flag = state
                    .flags
                    .get(flag_key)
                    .cloned()
                    .unwrap_or_else(|| self.empty_flag(flag_key));
                crate::evaluation::evaluate_float_flag(&flag, context, &state.flag_set_metadata)
            }
            None => EvaluationResult {
                value: JsonValue::Number(serde_json::Number::from_f64(0.0).unwrap()),
                variant: None,
                reason: crate::evaluation::ResolutionReason::Error,
                error_code: Some(crate::evaluation::ErrorCode::General),
                error_message: Some("No flag configuration loaded".to_string()),
                flag_metadata: None,
            },
        }
    }

    /// Evaluates a generic flag (for objects/structs).
    pub fn evaluate_flag(&self, flag_key: &str, context: &JsonValue) -> EvaluationResult {
        match &self.state {
            Some(state) => {
                let flag = state
                    .flags
                    .get(flag_key)
                    .cloned()
                    .unwrap_or_else(|| self.empty_flag(flag_key));
                crate::evaluation::evaluate_flag(&flag, context, &state.flag_set_metadata)
            }
            None => EvaluationResult {
                value: JsonValue::Object(serde_json::Map::new()),
                variant: None,
                reason: crate::evaluation::ResolutionReason::Error,
                error_code: Some(crate::evaluation::ErrorCode::General),
                error_message: Some("No flag configuration loaded".to_string()),
                flag_metadata: None,
            },
        }
    }

    /// Evaluates an object flag with type checking.
    pub fn evaluate_object(&self, flag_key: &str, context: &JsonValue) -> EvaluationResult {
        match &self.state {
            Some(state) => {
                let flag = state
                    .flags
                    .get(flag_key)
                    .cloned()
                    .unwrap_or_else(|| self.empty_flag(flag_key));
                crate::evaluation::evaluate_object_flag(&flag, context, &state.flag_set_metadata)
            }
            None => EvaluationResult {
                value: JsonValue::Object(serde_json::Map::new()),
                variant: None,
                reason: crate::evaluation::ResolutionReason::Error,
                error_code: Some(crate::evaluation::ErrorCode::General),
                error_message: Some("No flag configuration loaded".to_string()),
                flag_metadata: None,
            },
        }
    }

    /// Helper to create an empty flag for missing flags.
    fn empty_flag(&self, key: &str) -> crate::model::FeatureFlag {
        crate::model::FeatureFlag {
            key: Some(key.to_string()),
            state: "FLAG_NOT_FOUND".to_string(),
            default_variant: None,
            variants: Default::default(),
            targeting: None,
            metadata: Default::default(),
        }
    }

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
}

impl Default for FlagEvaluator {
    fn default() -> Self {
        Self::new(ValidationMode::Strict)
    }
}
