//! Internal flag storage for managing feature flag state.
//!
//! This module provides a simple in-memory flag store that can be updated
//! with JSON configurations in the standard flagd format. It also supports
//! JSON Schema validation with configurable behavior.

use crate::model::ParsingResult;
use crate::validation::validate_flags_config;
use std::cell::RefCell;

/// Validation mode determines how validation errors are handled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationMode {
    /// Store flags only if validation succeeds (default, strict mode)
    Strict,
    /// Store flags even if validation fails (permissive mode)
    Permissive,
}

thread_local! {
    /// Thread-local storage for feature flags.
    ///
    /// In WASM environments, there's a single thread, so we use RefCell for
    /// interior mutability without the overhead of multi-threading primitives.
    static FLAG_STORE: RefCell<Option<ParsingResult>> = const { RefCell::new(None) };
    
    /// Thread-local storage for validation mode configuration.
    ///
    /// Controls whether flags should be stored when validation fails.
    static VALIDATION_MODE: RefCell<ValidationMode> = const { RefCell::new(ValidationMode::Strict) };
}

/// Sets the validation mode for flag state updates.
///
/// # Arguments
///
/// * `mode` - The validation mode to use
///
/// # Example
///
/// ```
/// use flagd_evaluator::storage::{set_validation_mode, ValidationMode};
///
/// // Use permissive mode to store flags even with validation errors
/// set_validation_mode(ValidationMode::Permissive);
/// ```
pub fn set_validation_mode(mode: ValidationMode) {
    VALIDATION_MODE.with(|m| {
        *m.borrow_mut() = mode;
    });
}

/// Gets the current validation mode.
pub fn get_validation_mode() -> ValidationMode {
    VALIDATION_MODE.with(|m| *m.borrow())
}

/// Updates the internal flag state with a new configuration.
///
/// This function validates the provided JSON configuration against the flagd schema,
/// then parses and stores it. The behavior when validation fails depends on the
/// current validation mode:
///
/// - `ValidationMode::Strict` (default): Flags are only stored if validation succeeds
/// - `ValidationMode::Permissive`: Flags are stored even if validation fails
///
/// # Arguments
///
/// * `json_config` - JSON string containing the flag configuration
///
/// # Returns
///
/// * `Ok(())` - If the configuration was successfully parsed and stored (validation may have warnings in permissive mode)
/// * `Err(String)` - If there was an error (JSON parsing error in strict mode, or validation error in strict mode)
///
/// The error string contains a JSON object with validation details:
/// ```json
/// {
///   "valid": false,
///   "errors": [
///     {
///       "path": "/flags/myFlag",
///       "message": "Missing required field: state"
///     }
///   ]
/// }
/// ```
///
/// # Example
///
/// ```
/// use flagd_evaluator::storage::update_flag_state;
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
/// update_flag_state(config).unwrap();
/// ```
pub fn update_flag_state(json_config: &str) -> Result<(), String> {
    let validation_mode = get_validation_mode();
    
    // Validate the configuration against the schema
    let validation_result = validate_flags_config(json_config);
    
    match validation_mode {
        ValidationMode::Strict => {
            // In strict mode, fail if validation fails
            if let Err(validation_error) = validation_result {
                return Err(validation_error.to_json_string());
            }
        }
        ValidationMode::Permissive => {
            // In permissive mode, log but continue
            if let Err(validation_error) = validation_result {
                // We continue processing even with validation errors
                // The caller can check the logs or handle this differently
                eprintln!("Warning: Configuration has validation errors but will be stored due to permissive mode: {}", validation_error.to_json_string());
            }
        }
    }
    
    // Parse the configuration
    let parsing_result = ParsingResult::parse(json_config)?;

    // Store the parsed flags, replacing any existing state
    FLAG_STORE.with(|store| {
        *store.borrow_mut() = Some(parsing_result);
    });

    Ok(())
}

/// Retrieves a copy of the current flag state.
///
/// # Returns
///
/// * `Some(ParsingResult)` - If the flag store has been initialized
/// * `None` - If no configuration has been loaded yet
pub fn get_flag_state() -> Option<ParsingResult> {
    FLAG_STORE.with(|store| store.borrow().as_ref().cloned())
}

/// Clears the internal flag state.
///
/// This function removes all stored flags, resetting the store to an
/// uninitialized state.
pub fn clear_flag_state() {
    FLAG_STORE.with(|store| {
        *store.borrow_mut() = None;
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_update_flag_state_success() {
        let config = r#"{
            "flags": {
                "testFlag": {
                    "state": "ENABLED",
                    "defaultVariant": "on",
                    "variants": {
                        "on": true,
                        "off": false
                    }
                }
            }
        }"#;

        let result = update_flag_state(config);
        assert!(result.is_ok());

        let state = get_flag_state();
        assert!(state.is_some());
        let state = state.unwrap();
        assert_eq!(state.flags.len(), 1);
        assert!(state.flags.contains_key("testFlag"));
    }

    #[test]
    fn test_update_flag_state_replaces_existing() {
        // First configuration
        let config1 = r#"{
            "flags": {
                "flag1": {
                    "state": "ENABLED",
                    "defaultVariant": "on",
                    "variants": {"on": true}
                }
            }
        }"#;

        update_flag_state(config1).unwrap();
        let state = get_flag_state().unwrap();
        assert_eq!(state.flags.len(), 1);
        assert!(state.flags.contains_key("flag1"));

        // Second configuration should replace the first
        let config2 = r#"{
            "flags": {
                "flag2": {
                    "state": "ENABLED",
                    "defaultVariant": "off",
                    "variants": {"off": false}
                }
            }
        }"#;

        update_flag_state(config2).unwrap();
        let state = get_flag_state().unwrap();
        assert_eq!(state.flags.len(), 1);
        assert!(!state.flags.contains_key("flag1"));
        assert!(state.flags.contains_key("flag2"));
    }

    #[test]
    fn test_update_flag_state_invalid_json() {
        clear_flag_state();
        set_validation_mode(ValidationMode::Strict);
        
        let config = "not valid json";
        let result = update_flag_state(config);
        assert!(result.is_err());
        let err = result.unwrap_err();
        // Error should contain "Invalid JSON" from validation module
        assert!(err.contains("Invalid JSON") || err.contains("\"valid\":false"));
    }

    #[test]
    fn test_update_flag_state_missing_flags_field() {
        clear_flag_state();
        set_validation_mode(ValidationMode::Strict);
        
        let config = r#"{"other": "data"}"#;
        let result = update_flag_state(config);
        assert!(result.is_err());
        // Should fail validation because "flags" is required
        let err = result.unwrap_err();
        assert!(err.contains("\"valid\":false"));
    }

    #[test]
    fn test_update_flag_state_invalid_flag_structure() {
        let config = r#"{
            "flags": {
                "badFlag": {
                    "state": "ENABLED"
                }
            }
        }"#;
        let result = update_flag_state(config);
        assert!(result.is_err());
    }

    #[test]
    fn test_update_flag_state_with_metadata() {
        let config = r#"{
            "$schema": "https://flagd.dev/schema/v0/flags.json",
            "flags": {
                "myFlag": {
                    "state": "ENABLED",
                    "defaultVariant": "on",
                    "variants": {"on": true}
                }
            }
        }"#;

        update_flag_state(config).unwrap();
        let state = get_flag_state().unwrap();
        assert!(state.flag_set_metadata.contains_key("$schema"));
    }

    #[test]
    fn test_clear_flag_state() {
        let config = r#"{
            "flags": {
                "testFlag": {
                    "state": "ENABLED",
                    "defaultVariant": "on",
                    "variants": {"on": true}
                }
            }
        }"#;

        update_flag_state(config).unwrap();
        assert!(get_flag_state().is_some());

        clear_flag_state();
        assert!(get_flag_state().is_none());
    }

    #[test]
    fn test_get_flag_state_before_initialization() {
        clear_flag_state();
        assert!(get_flag_state().is_none());
    }

    #[test]
    fn test_update_flag_state_empty_flags() {
        let config = r#"{"flags": {}}"#;
        update_flag_state(config).unwrap();
        let state = get_flag_state().unwrap();
        assert_eq!(state.flags.len(), 0);
    }

    #[test]
    fn test_update_flag_state_multiple_flags() {
        clear_flag_state();
        set_validation_mode(ValidationMode::Strict);
        
        let config = r#"{
            "flags": {
                "flag1": {
                    "state": "ENABLED",
                    "defaultVariant": "on",
                    "variants": {"on": true}
                },
                "flag2": {
                    "state": "DISABLED",
                    "defaultVariant": "off",
                    "variants": {"off": false}
                },
                "flag3": {
                    "state": "ENABLED",
                    "defaultVariant": "red",
                    "variants": {
                        "red": "red",
                        "blue": "blue"
                    }
                }
            }
        }"#;

        update_flag_state(config).unwrap();
        let state = get_flag_state().unwrap();
        assert_eq!(state.flags.len(), 3);
        assert!(state.flags.contains_key("flag1"));
        assert!(state.flags.contains_key("flag2"));
        assert!(state.flags.contains_key("flag3"));
    }

    #[test]
    fn test_validation_mode_strict_rejects_invalid() {
        clear_flag_state();
        set_validation_mode(ValidationMode::Strict);

        // Invalid config - missing required fields
        let config = r#"{
            "flags": {
                "badFlag": {
                    "state": "ENABLED"
                }
            }
        }"#;

        let result = update_flag_state(config);
        assert!(result.is_err());
        
        // State should not be updated
        assert!(get_flag_state().is_none());
    }

    #[test]
    fn test_validation_mode_permissive_accepts_invalid() {
        clear_flag_state();
        set_validation_mode(ValidationMode::Permissive);

        // Invalid config according to schema, but valid JSON that can be parsed
        // Note: ParsingResult::parse will still fail if required fields are missing
        // So we need a config that fails schema validation but parses correctly
        let config = r#"{
            "flags": {
                "flag1": {
                    "state": "ENABLED",
                    "defaultVariant": "on",
                    "variants": {"on": true, "off": false}
                }
            },
            "invalidField": "should not be here"
        }"#;

        // In permissive mode, this should succeed (or at least not fail on schema validation)
        let result = update_flag_state(config);
        // This might still succeed because the parser is more lenient
        assert!(result.is_ok());

        // Reset to strict mode for other tests
        set_validation_mode(ValidationMode::Strict);
    }

    #[test]
    fn test_validation_mode_switch() {
        assert_eq!(get_validation_mode(), ValidationMode::Strict);

        set_validation_mode(ValidationMode::Permissive);
        assert_eq!(get_validation_mode(), ValidationMode::Permissive);

        set_validation_mode(ValidationMode::Strict);
        assert_eq!(get_validation_mode(), ValidationMode::Strict);
    }

    #[test]
    fn test_validation_mode_strict_with_valid_config() {
        clear_flag_state();
        set_validation_mode(ValidationMode::Strict);

        let config = r#"{
            "flags": {
                "validFlag": {
                    "state": "ENABLED",
                    "defaultVariant": "on",
                    "variants": {"on": true, "off": false}
                }
            }
        }"#;

        let result = update_flag_state(config);
        assert!(result.is_ok());

        let state = get_flag_state().unwrap();
        assert_eq!(state.flags.len(), 1);
        assert!(state.flags.contains_key("validFlag"));
    }

    #[test]
    fn test_validation_error_format() {
        clear_flag_state();
        set_validation_mode(ValidationMode::Strict);

        // Invalid state value
        let config = r#"{
            "flags": {
                "badFlag": {
                    "state": "INVALID",
                    "defaultVariant": "on",
                    "variants": {"on": true}
                }
            }
        }"#;

        let result = update_flag_state(config);
        assert!(result.is_err());
        
        let error = result.unwrap_err();
        // Error should be valid JSON
        let error_json: serde_json::Value = serde_json::from_str(&error).unwrap();
        assert_eq!(error_json["valid"], false);
        assert!(error_json["errors"].is_array());
    }
}
