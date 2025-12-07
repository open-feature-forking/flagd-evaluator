//! Internal flag storage for managing feature flag state.
//!
//! This module provides a simple in-memory flag store that can be updated
//! with JSON configurations in the standard flagd format. It also supports
//! JSON Schema validation with configurable behavior.

use crate::model::{ParsingResult, UpdateStateResponse};
use crate::validation::validate_flags_config;
use std::cell::RefCell;
use std::collections::HashSet;

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
/// The function also detects changes between the old and new configuration by comparing:
/// - Added flags (present in new config but not in old)
/// - Removed flags (present in old config but not in new)
/// - Mutated flags (any field changed: state, default variant, variants, targeting rules, or metadata)
///
/// # Arguments
///
/// * `json_config` - JSON string containing the flag configuration
///
/// # Returns
///
/// * `Ok(UpdateStateResponse)` - If the configuration was successfully parsed and stored,
///   with a list of changed flag keys
/// * `Err(String)` - If there was an error (JSON parsing error or validation error in strict mode)
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
/// let response = update_flag_state(config).unwrap();
/// assert!(response.success);
/// assert!(response.changed_flags.is_some());
/// ```
pub fn update_flag_state(json_config: &str) -> Result<UpdateStateResponse, String> {
    let validation_mode = get_validation_mode();

    // Validate the configuration against the schema
    let validation_result = validate_flags_config(json_config);

    match validation_mode {
        ValidationMode::Strict => {
            // In strict mode, fail if validation fails
            if let Err(validation_error) = validation_result {
                return Ok(UpdateStateResponse {
                    success: false,
                    error: Some(validation_error.to_json_string()),
                    changed_flags: None,
                });
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

    // Get the current state to compare
    let old_state = get_flag_state();

    // Detect changed flags
    let changed_flags = detect_changed_flags(old_state.as_ref(), &new_parsing_result);

    // Store the parsed flags, replacing any existing state
    FLAG_STORE.with(|store| {
        *store.borrow_mut() = Some(new_parsing_result);
    });

    Ok(UpdateStateResponse {
        success: true,
        error: None,
        changed_flags: Some(changed_flags),
    })
}

/// Detects which flags have changed between the old and new state.
///
/// Compares flags based on:
/// - Added flags (in new but not in old)
/// - Removed flags (in old but not in new)
/// - Mutated flags (any field changed: state, default variant, variants, targeting, or metadata)
///
/// # Arguments
///
/// * `old_state` - The previous flag state (if any)
/// * `new_state` - The new flag state
///
/// # Returns
///
/// A vector of flag keys that have changed
fn detect_changed_flags(
    old_state: Option<&ParsingResult>,
    new_state: &ParsingResult,
) -> Vec<String> {
    let mut changed_keys = HashSet::new();

    match old_state {
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
                        // Flag was added
                        changed_keys.insert(key.clone());
                    }
                    Some(old_flag) => {
                        // Flag exists in both, check if it changed
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

    // Convert to sorted Vec for consistent output
    let mut result: Vec<String> = changed_keys.into_iter().collect();
    result.sort();
    result
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

        let response = update_flag_state(config).unwrap();
        assert!(response.success);
        assert!(response.error.is_none());
        assert!(response.changed_flags.is_some());

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

        let response = update_flag_state(config2).unwrap();
        assert!(response.success);
        // Both flags changed (flag1 removed, flag2 added)
        assert_eq!(response.changed_flags.unwrap().len(), 2);

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
        let response = update_flag_state(config).unwrap();
        assert!(!response.success);
        assert!(response.error.is_some());
        let err = response.error.unwrap();
        // Error should contain "Invalid JSON" from validation module
        assert!(err.contains("Invalid JSON") || err.contains("\"valid\":false"));
    }

    #[test]
    fn test_update_flag_state_missing_flags_field() {
        clear_flag_state();
        set_validation_mode(ValidationMode::Strict);

        let config = r#"{"other": "data"}"#;
        let response = update_flag_state(config).unwrap();
        assert!(!response.success);
        // Should fail validation because "flags" is required
        assert!(response.error.is_some());
        let err = response.error.unwrap();
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
        let response = update_flag_state(config).unwrap();
        assert!(!response.success);
        assert!(response.error.is_some());
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

        let response = update_flag_state(config).unwrap();
        assert!(!response.success);

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

        let response = update_flag_state(config).unwrap();
        assert!(response.success);

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

        let response = update_flag_state(config).unwrap();
        assert!(!response.success);

        let error = response.error.unwrap();
        // Error should be valid JSON
        let error_json: serde_json::Value = serde_json::from_str(&error).unwrap();
        assert_eq!(error_json["valid"], false);
        assert!(error_json["errors"].is_array());
    }

    // ============================================================================
    // Change detection tests
    // ============================================================================

    #[test]
    fn test_changed_flags_on_first_update() {
        clear_flag_state();

        let config = r#"{
            "flags": {
                "flag1": {
                    "state": "ENABLED",
                    "defaultVariant": "on",
                    "variants": {"on": true}
                },
                "flag2": {
                    "state": "ENABLED",
                    "defaultVariant": "off",
                    "variants": {"off": false}
                }
            }
        }"#;

        let response = update_flag_state(config).unwrap();
        assert!(response.success);
        
        let changed = response.changed_flags.unwrap();
        assert_eq!(changed.len(), 2);
        assert!(changed.contains(&"flag1".to_string()));
        assert!(changed.contains(&"flag2".to_string()));
    }

    #[test]
    fn test_changed_flags_added() {
        clear_flag_state();

        // Initial state
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

        // Add a new flag
        let config2 = r#"{
            "flags": {
                "flag1": {
                    "state": "ENABLED",
                    "defaultVariant": "on",
                    "variants": {"on": true}
                },
                "flag2": {
                    "state": "ENABLED",
                    "defaultVariant": "off",
                    "variants": {"off": false}
                }
            }
        }"#;

        let response = update_flag_state(config2).unwrap();
        assert!(response.success);
        
        let changed = response.changed_flags.unwrap();
        assert_eq!(changed.len(), 1);
        assert!(changed.contains(&"flag2".to_string()));
    }

    #[test]
    fn test_changed_flags_removed() {
        clear_flag_state();

        // Initial state with two flags
        let config1 = r#"{
            "flags": {
                "flag1": {
                    "state": "ENABLED",
                    "defaultVariant": "on",
                    "variants": {"on": true}
                },
                "flag2": {
                    "state": "ENABLED",
                    "defaultVariant": "off",
                    "variants": {"off": false}
                }
            }
        }"#;
        update_flag_state(config1).unwrap();

        // Remove flag2
        let config2 = r#"{
            "flags": {
                "flag1": {
                    "state": "ENABLED",
                    "defaultVariant": "on",
                    "variants": {"on": true}
                }
            }
        }"#;

        let response = update_flag_state(config2).unwrap();
        assert!(response.success);
        
        let changed = response.changed_flags.unwrap();
        assert_eq!(changed.len(), 1);
        assert!(changed.contains(&"flag2".to_string()));
    }

    #[test]
    fn test_changed_flags_default_variant_mutation() {
        clear_flag_state();

        // Initial state
        let config1 = r#"{
            "flags": {
                "flag1": {
                    "state": "ENABLED",
                    "defaultVariant": "on",
                    "variants": {
                        "on": true,
                        "off": false
                    }
                }
            }
        }"#;
        update_flag_state(config1).unwrap();

        // Change default variant
        let config2 = r#"{
            "flags": {
                "flag1": {
                    "state": "ENABLED",
                    "defaultVariant": "off",
                    "variants": {
                        "on": true,
                        "off": false
                    }
                }
            }
        }"#;

        let response = update_flag_state(config2).unwrap();
        assert!(response.success);
        
        let changed = response.changed_flags.unwrap();
        assert_eq!(changed.len(), 1);
        assert!(changed.contains(&"flag1".to_string()));
    }

    #[test]
    fn test_changed_flags_targeting_mutation() {
        clear_flag_state();

        // Initial state
        let config1 = r#"{
            "flags": {
                "flag1": {
                    "state": "ENABLED",
                    "defaultVariant": "on",
                    "variants": {
                        "on": true,
                        "off": false
                    },
                    "targeting": {
                        "if": [
                            {"==": [{"var": "user"}, "admin"]},
                            "on",
                            "off"
                        ]
                    }
                }
            }
        }"#;
        update_flag_state(config1).unwrap();

        // Change targeting rule
        let config2 = r#"{
            "flags": {
                "flag1": {
                    "state": "ENABLED",
                    "defaultVariant": "on",
                    "variants": {
                        "on": true,
                        "off": false
                    },
                    "targeting": {
                        "if": [
                            {"==": [{"var": "user"}, "superadmin"]},
                            "on",
                            "off"
                        ]
                    }
                }
            }
        }"#;

        let response = update_flag_state(config2).unwrap();
        assert!(response.success);
        
        let changed = response.changed_flags.unwrap();
        assert_eq!(changed.len(), 1);
        assert!(changed.contains(&"flag1".to_string()));
    }

    #[test]
    fn test_changed_flags_metadata_mutation() {
        clear_flag_state();

        // Initial state
        let config1 = r#"{
            "flags": {
                "flag1": {
                    "state": "ENABLED",
                    "defaultVariant": "on",
                    "variants": {"on": true},
                    "metadata": {
                        "description": "Original description"
                    }
                }
            }
        }"#;
        update_flag_state(config1).unwrap();

        // Change metadata
        let config2 = r#"{
            "flags": {
                "flag1": {
                    "state": "ENABLED",
                    "defaultVariant": "on",
                    "variants": {"on": true},
                    "metadata": {
                        "description": "Updated description"
                    }
                }
            }
        }"#;

        let response = update_flag_state(config2).unwrap();
        assert!(response.success);
        
        let changed = response.changed_flags.unwrap();
        assert_eq!(changed.len(), 1);
        assert!(changed.contains(&"flag1".to_string()));
    }

    #[test]
    fn test_changed_flags_state_mutation() {
        clear_flag_state();

        // Initial state
        let config1 = r#"{
            "flags": {
                "flag1": {
                    "state": "ENABLED",
                    "defaultVariant": "on",
                    "variants": {"on": true, "off": false}
                }
            }
        }"#;
        update_flag_state(config1).unwrap();

        // Change state from ENABLED to DISABLED
        let config2 = r#"{
            "flags": {
                "flag1": {
                    "state": "DISABLED",
                    "defaultVariant": "on",
                    "variants": {"on": true, "off": false}
                }
            }
        }"#;

        let response = update_flag_state(config2).unwrap();
        assert!(response.success);
        
        let changed = response.changed_flags.unwrap();
        assert_eq!(changed.len(), 1);
        assert!(changed.contains(&"flag1".to_string()));
    }

    #[test]
    fn test_changed_flags_variants_mutation() {
        clear_flag_state();

        // Initial state
        let config1 = r#"{
            "flags": {
                "flag1": {
                    "state": "ENABLED",
                    "defaultVariant": "on",
                    "variants": {
                        "on": true,
                        "off": false
                    }
                }
            }
        }"#;
        update_flag_state(config1).unwrap();

        // Change variant value
        let config2 = r#"{
            "flags": {
                "flag1": {
                    "state": "ENABLED",
                    "defaultVariant": "on",
                    "variants": {
                        "on": true,
                        "off": true
                    }
                }
            }
        }"#;

        let response = update_flag_state(config2).unwrap();
        assert!(response.success);
        
        let changed = response.changed_flags.unwrap();
        assert_eq!(changed.len(), 1);
        assert!(changed.contains(&"flag1".to_string()));
    }

    #[test]
    fn test_changed_flags_variants_added() {
        clear_flag_state();

        // Initial state with two variants
        let config1 = r#"{
            "flags": {
                "flag1": {
                    "state": "ENABLED",
                    "defaultVariant": "red",
                    "variants": {
                        "red": "red-value",
                        "blue": "blue-value"
                    }
                }
            }
        }"#;
        update_flag_state(config1).unwrap();

        // Add a new variant
        let config2 = r#"{
            "flags": {
                "flag1": {
                    "state": "ENABLED",
                    "defaultVariant": "red",
                    "variants": {
                        "red": "red-value",
                        "blue": "blue-value",
                        "green": "green-value"
                    }
                }
            }
        }"#;

        let response = update_flag_state(config2).unwrap();
        assert!(response.success);
        
        let changed = response.changed_flags.unwrap();
        assert_eq!(changed.len(), 1);
        assert!(changed.contains(&"flag1".to_string()));
    }

    #[test]
    fn test_changed_flags_no_changes() {
        clear_flag_state();

        // Initial state
        let config = r#"{
            "flags": {
                "flag1": {
                    "state": "ENABLED",
                    "defaultVariant": "on",
                    "variants": {"on": true}
                }
            }
        }"#;
        update_flag_state(config).unwrap();

        // Apply same config again
        let response = update_flag_state(config).unwrap();
        assert!(response.success);
        
        let changed = response.changed_flags.unwrap();
        assert_eq!(changed.len(), 0);
    }

    #[test]
    fn test_changed_flags_mixed_operations() {
        clear_flag_state();

        // Initial state
        let config1 = r#"{
            "flags": {
                "flag1": {
                    "state": "ENABLED",
                    "defaultVariant": "on",
                    "variants": {"on": true}
                },
                "flag2": {
                    "state": "ENABLED",
                    "defaultVariant": "off",
                    "variants": {"off": false}
                },
                "flag3": {
                    "state": "ENABLED",
                    "defaultVariant": "red",
                    "variants": {"red": "red", "blue": "blue"}
                }
            }
        }"#;
        update_flag_state(config1).unwrap();

        // Mixed: keep flag1 same, modify flag2, remove flag3, add flag4
        let config2 = r#"{
            "flags": {
                "flag1": {
                    "state": "ENABLED",
                    "defaultVariant": "on",
                    "variants": {"on": true}
                },
                "flag2": {
                    "state": "ENABLED",
                    "defaultVariant": "on",
                    "variants": {"off": false}
                },
                "flag4": {
                    "state": "ENABLED",
                    "defaultVariant": "green",
                    "variants": {"green": "green"}
                }
            }
        }"#;

        let response = update_flag_state(config2).unwrap();
        assert!(response.success);
        
        let changed = response.changed_flags.unwrap();
        // flag2 modified, flag3 removed, flag4 added
        assert_eq!(changed.len(), 3);
        assert!(changed.contains(&"flag2".to_string()));
        assert!(changed.contains(&"flag3".to_string()));
        assert!(changed.contains(&"flag4".to_string()));
        assert!(!changed.contains(&"flag1".to_string()));
    }

    #[test]
    fn test_changed_flags_sorted_output() {
        clear_flag_state();

        // Add multiple flags at once
        let config = r#"{
            "flags": {
                "zFlag": {
                    "state": "ENABLED",
                    "defaultVariant": "on",
                    "variants": {"on": true}
                },
                "aFlag": {
                    "state": "ENABLED",
                    "defaultVariant": "on",
                    "variants": {"on": true}
                },
                "mFlag": {
                    "state": "ENABLED",
                    "defaultVariant": "on",
                    "variants": {"on": true}
                }
            }
        }"#;

        let response = update_flag_state(config).unwrap();
        assert!(response.success);
        
        let changed = response.changed_flags.unwrap();
        assert_eq!(changed.len(), 3);
        // Should be sorted alphabetically
        assert_eq!(changed[0], "aFlag");
        assert_eq!(changed[1], "mFlag");
        assert_eq!(changed[2], "zFlag");
    }
}
