//! State management for feature flag evaluation.
//!
//! This module provides thread-safe global state management for storing
//! and retrieving feature flag configurations.

use crate::model::ParsingResult;
use std::cell::RefCell;

thread_local! {
    /// Thread-local storage for the feature flag state.
    /// This allows each thread to have its own independent copy of the flag configuration.
    static FLAG_STATE: RefCell<Option<ParsingResult>> = const { RefCell::new(None) };
}

/// Updates the global flag state with a new configuration.
///
/// # Arguments
/// * `parsing_result` - The parsed flag configuration to store
pub fn update_flag_state(parsing_result: ParsingResult) {
    FLAG_STATE.with(|state| {
        *state.borrow_mut() = Some(parsing_result);
    });
}

/// Retrieves a feature flag from the global state by key.
///
/// # Arguments
/// * `flag_key` - The key of the flag to retrieve
///
/// # Returns
/// * `Some(FeatureFlag)` if the flag exists
/// * `None` if the flag does not exist or state is not initialized
pub fn get_flag(flag_key: &str) -> Option<crate::model::FeatureFlag> {
    FLAG_STATE.with(|state| {
        state
            .borrow()
            .as_ref()
            .and_then(|parsing_result| parsing_result.flags.get(flag_key).cloned())
    })
}

/// Clears the global flag state.
///
/// This is primarily useful for testing.
pub fn clear_flag_state() {
    FLAG_STATE.with(|state| {
        *state.borrow_mut() = None;
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::FeatureFlag;
    use std::collections::HashMap;

    #[test]
    fn test_update_and_get_flag() {
        clear_flag_state();

        let mut flags = HashMap::new();
        flags.insert(
            "test-flag".to_string(),
            FeatureFlag {
                state: "ENABLED".to_string(),
                default_variant: "on".to_string(),
                variants: HashMap::new(),
                targeting: None,
                metadata: HashMap::new(),
            },
        );

        let parsing_result = ParsingResult {
            flags,
            flag_set_metadata: HashMap::new(),
        };

        update_flag_state(parsing_result);

        let flag = get_flag("test-flag");
        assert!(flag.is_some());
        assert_eq!(flag.unwrap().state, "ENABLED");
    }

    #[test]
    fn test_get_nonexistent_flag() {
        clear_flag_state();

        let flag = get_flag("nonexistent");
        assert!(flag.is_none());
    }

    #[test]
    fn test_clear_flag_state() {
        let mut flags = HashMap::new();
        flags.insert(
            "test-flag".to_string(),
            FeatureFlag {
                state: "ENABLED".to_string(),
                default_variant: "on".to_string(),
                variants: HashMap::new(),
                targeting: None,
                metadata: HashMap::new(),
            },
        );

        let parsing_result = ParsingResult {
            flags,
            flag_set_metadata: HashMap::new(),
        };

        update_flag_state(parsing_result);
        assert!(get_flag("test-flag").is_some());

        clear_flag_state();
        assert!(get_flag("test-flag").is_none());
    }
}
