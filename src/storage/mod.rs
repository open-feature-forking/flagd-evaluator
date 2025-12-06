//! Internal flag storage for managing feature flag state.
//!
//! This module provides a simple in-memory flag store that can be updated
//! with JSON configurations in the standard flagd format.

use crate::model::ParsingResult;
use std::cell::RefCell;

thread_local! {
    /// Thread-local storage for feature flags.
    ///
    /// In WASM environments, there's a single thread, so we use RefCell for
    /// interior mutability without the overhead of multi-threading primitives.
    static FLAG_STORE: RefCell<Option<ParsingResult>> = const { RefCell::new(None) };
}

/// Updates the internal flag state with a new configuration.
///
/// This function parses the provided JSON configuration in the standard flagd
/// format and replaces the entire internal state. Any previously stored flags
/// are discarded.
///
/// # Arguments
///
/// * `json_config` - JSON string containing the flag configuration
///
/// # Returns
///
/// * `Ok(())` - If the configuration was successfully parsed and stored
/// * `Err(String)` - If there was an error parsing the configuration
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
    FLAG_STORE.with(|store| {
        store.borrow().as_ref().map(|result| ParsingResult {
            flags: result.flags.clone(),
            flag_set_metadata: result.flag_set_metadata.clone(),
        })
    })
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
        let config = "not valid json";
        let result = update_flag_state(config);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Failed to parse JSON"));
    }

    #[test]
    fn test_update_flag_state_missing_flags_field() {
        let config = r#"{"other": "data"}"#;
        let result = update_flag_state(config);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing 'flags' field"));
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
}
