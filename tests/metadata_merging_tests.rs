//! Tests for metadata merging in flag evaluation responses.
//!
//! According to the flagd provider specification, evaluation responses should merge
//! flag-set metadata and flag-level metadata, with flag metadata taking priority.
//! Metadata should be returned on a "best effort" basis for disabled, missing, and
//! erroneous flags.

use flagd_evaluator::evaluation::evaluate_flag;
use flagd_evaluator::storage::{clear_flag_state, update_flag_state};
use serde_json::json;

#[test]
fn test_metadata_merging_flag_priority() {
    // Test that flag metadata takes priority over flag-set metadata
    clear_flag_state();

    let config = r#"{
        "metadata": {
            "version": "1.0",
            "env": "production",
            "owner": "flagset"
        },
        "flags": {
            "testFlag": {
                "state": "ENABLED",
                "defaultVariant": "on",
                "variants": {
                    "on": true,
                    "off": false
                },
                "metadata": {
                    "owner": "flag-owner",
                    "description": "Test flag"
                }
            }
        }
    }"#;

    update_flag_state(config).unwrap();
    let state = flagd_evaluator::storage::get_flag_state().unwrap();
    let flag = state.flags.get("testFlag").unwrap();

    let context = json!({});
    let result = evaluate_flag(flag, &context, &state.flag_set_metadata);

    // Verify metadata is merged with flag metadata taking priority
    assert!(result.flag_metadata.is_some());
    let metadata = result.flag_metadata.unwrap();

    // Flag metadata should override flag-set metadata
    assert_eq!(metadata.get("owner").unwrap(), "flag-owner");

    // Flag metadata should be included
    assert_eq!(metadata.get("description").unwrap(), "Test flag");

    // Flag-set metadata should be included where not overridden
    assert_eq!(metadata.get("version").unwrap(), "1.0");
    assert_eq!(metadata.get("env").unwrap(), "production");
}

#[test]
fn test_metadata_only_flag_set() {
    // Test when only flag-set metadata exists
    clear_flag_state();

    let config = r#"{
        "metadata": {
            "version": "2.0",
            "team": "platform"
        },
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

    update_flag_state(config).unwrap();
    let state = flagd_evaluator::storage::get_flag_state().unwrap();
    let flag = state.flags.get("testFlag").unwrap();

    let context = json!({});
    let result = evaluate_flag(flag, &context, &state.flag_set_metadata);

    // Verify flag-set metadata is included
    assert!(result.flag_metadata.is_some());
    let metadata = result.flag_metadata.unwrap();
    assert_eq!(metadata.get("version").unwrap(), "2.0");
    assert_eq!(metadata.get("team").unwrap(), "platform");
}

#[test]
fn test_metadata_only_flag_level() {
    // Test when only flag-level metadata exists
    clear_flag_state();

    let config = r#"{
        "flags": {
            "testFlag": {
                "state": "ENABLED",
                "defaultVariant": "on",
                "variants": {
                    "on": true,
                    "off": false
                },
                "metadata": {
                    "deprecated": false,
                    "contact": "team@example.com"
                }
            }
        }
    }"#;

    update_flag_state(config).unwrap();
    let state = flagd_evaluator::storage::get_flag_state().unwrap();
    let flag = state.flags.get("testFlag").unwrap();

    let context = json!({});
    let result = evaluate_flag(flag, &context, &state.flag_set_metadata);

    // Verify flag-level metadata is included
    assert!(result.flag_metadata.is_some());
    let metadata = result.flag_metadata.unwrap();
    assert_eq!(metadata.get("deprecated").unwrap(), false);
    assert_eq!(metadata.get("contact").unwrap(), "team@example.com");
}

#[test]
fn test_metadata_with_disabled_flag() {
    // Test metadata is returned for disabled flags
    clear_flag_state();

    let config = r#"{
        "metadata": {
            "version": "1.0"
        },
        "flags": {
            "disabledFlag": {
                "state": "DISABLED",
                "defaultVariant": "off",
                "variants": {
                    "on": true,
                    "off": false
                },
                "metadata": {
                    "reason": "deprecated"
                }
            }
        }
    }"#;

    update_flag_state(config).unwrap();
    let state = flagd_evaluator::storage::get_flag_state().unwrap();
    let flag = state.flags.get("disabledFlag").unwrap();

    let context = json!({});
    let result = evaluate_flag(flag, &context, &state.flag_set_metadata);

    // Verify disabled flag returns metadata
    assert_eq!(
        result.reason,
        flagd_evaluator::evaluation::ResolutionReason::Disabled
    );
    assert!(result.flag_metadata.is_some());
    let metadata = result.flag_metadata.unwrap();
    assert_eq!(metadata.get("reason").unwrap(), "deprecated");
    assert_eq!(metadata.get("version").unwrap(), "1.0");
}

#[test]
fn test_metadata_with_targeting_match() {
    // Test metadata is returned with targeting match
    clear_flag_state();

    let config = r#"{
        "metadata": {
            "project": "feature-flags"
        },
        "flags": {
            "targetedFlag": {
                "state": "ENABLED",
                "defaultVariant": "off",
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
                },
                "metadata": {
                    "category": "access-control"
                }
            }
        }
    }"#;

    update_flag_state(config).unwrap();
    let state = flagd_evaluator::storage::get_flag_state().unwrap();
    let flag = state.flags.get("targetedFlag").unwrap();

    let context = json!({"user": "admin"});
    let result = evaluate_flag(flag, &context, &state.flag_set_metadata);

    // Verify targeting match includes merged metadata
    assert_eq!(
        result.reason,
        flagd_evaluator::evaluation::ResolutionReason::TargetingMatch
    );
    assert!(result.flag_metadata.is_some());
    let metadata = result.flag_metadata.unwrap();
    assert_eq!(metadata.get("category").unwrap(), "access-control");
    assert_eq!(metadata.get("project").unwrap(), "feature-flags");
}

#[test]
fn test_no_metadata_when_empty() {
    // Test that no metadata field is present when both are empty
    clear_flag_state();

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

    update_flag_state(config).unwrap();
    let state = flagd_evaluator::storage::get_flag_state().unwrap();
    let flag = state.flags.get("testFlag").unwrap();

    let context = json!({});
    let result = evaluate_flag(flag, &context, &state.flag_set_metadata);

    // Verify no metadata field when both are empty
    assert!(result.flag_metadata.is_none());
}
